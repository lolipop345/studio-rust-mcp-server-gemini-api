use crate::error::{Report, Result};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::{extract::State, Json};
use color_eyre::eyre::{eyre, Error, OptionExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::oneshot::Receiver;
use tokio::sync::{mpsc, watch, Mutex};
use tokio::time::Duration;
use uuid::Uuid;

pub const STUDIO_PLUGIN_PORT: u16 = 44755;
const LONG_POLL_DURATION: Duration = Duration::from_secs(15);
const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1alpha/models";

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ToolArguments {
    pub args: ToolArgumentValues,
    pub id: Option<Uuid>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct RunCommandResponse {
    pub success: bool,
    pub response: String,
    pub id: Uuid,
}

pub struct AppState {
    pub process_queue: VecDeque<ToolArguments>,
    pub output_map: HashMap<Uuid, mpsc::UnboundedSender<Result<String>>>,
    pub waiter: watch::Receiver<()>,
    pub trigger: watch::Sender<()>,
    pub api_key: Option<String>,
    pub conversation_history: Vec<GeminiContent>,
    pub chat_streams: HashMap<String, mpsc::Sender<SsePayload>>,
    pub chat_receivers: HashMap<String, mpsc::Receiver<SsePayload>>,
    pub active_generations: HashMap<String, tokio::task::AbortHandle>,
    pub plan_waiters: HashMap<String, tokio::sync::oneshot::Sender<String>>,
    pub db: crate::db::DbClient,
}

pub type PackedState = Arc<Mutex<AppState>>;

#[derive(Debug, Clone)]
pub enum SsePayload {
    ToolCall {
        name: String,
        args: serde_json::Value,
        call_index: usize,
    },
    ToolResult {
        name: String,
        result: String,
        call_index: usize,
    },
    Text {
        content: String,
    },
    Thought {
        content: String,
    },
    ThoughtSignature {
        signature: String,
    },
    Done,
    Error {
        error: String,
    },
}

impl AppState {
    pub fn new(db: crate::db::DbClient) -> Self {
        let (trigger, waiter) = watch::channel(());
        Self {
            process_queue: VecDeque::new(),
            output_map: HashMap::new(),
            waiter,
            trigger,
            api_key: None,
            conversation_history: Vec::new(),
            chat_streams: HashMap::new(),
            chat_receivers: HashMap::new(),
            active_generations: HashMap::new(),
            plan_waiters: HashMap::new(),
            db,
        }
    }
}

impl ToolArguments {
    pub fn new(args: ToolArgumentValues) -> (Self, Uuid) {
        Self { args, id: None }.with_id()
    }

    fn with_id(self) -> (Self, Uuid) {
        let id = Uuid::new_v4();
        (
            Self {
                args: self.args,
                id: Some(id),
            },
            id,
        )
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RunCode {
    pub command: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct InsertModel {
    pub query: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GetConsoleOutput {}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GetStudioMode {}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StartStopPlay {
    pub mode: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RunScriptInPlayMode {
    pub code: String,
    pub timeout: Option<u32>,
    pub mode: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProposePlan {
    pub plan_text: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum ToolArgumentValues {
    RunCode(RunCode),
    InsertModel(InsertModel),
    GetConsoleOutput(GetConsoleOutput),
    StartStopPlay(StartStopPlay),
    RunScriptInPlayMode(RunScriptInPlayMode),
    GetStudioMode(GetStudioMode),
    ProposePlan(ProposePlan),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiRequest {
    pub contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDeclaration>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "generationConfig")]
    pub generation_config: Option<GenerationConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ThinkingConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_thoughts: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeminiPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "thoughtSignature")]
    pub thought_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<InlineData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<GeminiFunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<GeminiFunctionResponse>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InlineData {
    pub mime_type: String,
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiFunctionCall {
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiFunctionResponse {
    pub name: String,
    pub response: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolDeclaration {
    #[serde(skip_serializing_if = "Option::is_none", rename = "functionDeclarations")]
    pub function_declarations: Option<Vec<FunctionDeclaration>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "googleSearch")]
    pub google_search: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "codeExecution")]
    pub code_execution: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionDeclaration {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiResponse {
    pub candidates: Option<Vec<GeminiCandidate>>,
    pub error: Option<GeminiError>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiError {
    pub message: String,
    pub code: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiCandidate {
    pub content: Option<GeminiContent>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GeminiModel {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(rename = "baseModelId", default)]
    pub base_model_id: Option<String>,
    #[serde(rename = "displayName", default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "inputTokenLimit", default)]
    pub input_token_limit: Option<u64>,
    #[serde(rename = "outputTokenLimit", default)]
    pub output_token_limit: Option<u64>,
    #[serde(rename = "supportedGenerationMethods", default)]
    pub supported_generation_methods: Vec<String>,
    #[serde(default)]
    pub thinking: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ListModelsResponse {
    pub models: Vec<GeminiModel>,
    #[serde(rename = "nextPageToken", default)]
    pub next_page_token: Option<String>,
}

pub fn build_tool_declarations() -> Vec<ToolDeclaration> {
    vec![ToolDeclaration {
        function_declarations: Some(vec![
            FunctionDeclaration {
                name: "run_code".to_string(),
                description: "Runs a command in Roblox Studio and returns the printed output. Can be used to both make changes and retrieve information.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Code to run"
                        }
                    },
                    "required": ["command"]
                })),
            },
            FunctionDeclaration {
                name: "insert_model".to_string(),
                description: "Inserts a model from the Roblox marketplace into the workspace. Returns the inserted model name.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Query to search for the model"
                        }
                    },
                    "required": ["query"]
                })),
            },
            FunctionDeclaration {
                name: "get_console_output".to_string(),
                description: "Get the console output from Roblox Studio.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {}
                })),
            },
            FunctionDeclaration {
                name: "start_stop_play".to_string(),
                description: "Start or stop play mode or run the server. Don't enter run_server mode unless you are sure no client/player is needed.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "mode": {
                            "type": "string",
                            "description": "Mode to start or stop, must be start_play, stop, or run_server. Don't use run_server unless you are sure no client/player is needed.",
                            "enum": ["start_play", "stop", "run_server"]
                        }
                    },
                    "required": ["mode"]
                })),
            },
            FunctionDeclaration {
                name: "run_script_in_play_mode".to_string(),
                description: "Run a script in play mode and automatically stop play after script finishes or timeout. Returns the output of the script. Result format: { success: boolean, value: string, error: string, logs: { level: string, message: string, ts: number }[], errors: { level: string, message: string, ts: number }[], duration: number, isTimeout: boolean }. Prefer using start_stop_play tool instead run_script_in_play_mode. Only use run_script_in_play_mode to run one time unit test code on server datamodel. After calling run_script_in_play_mode, the datamodel status will be reset to stop mode. If it returns 'StudioTestService: Previous call to start play session has not been completed', call start_stop_play tool to stop play mode first then try it again.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "code": {
                            "type": "string",
                            "description": "Code to run"
                        },
                        "timeout": {
                            "type": "integer",
                            "description": "Timeout in seconds, defaults to 100 seconds"
                        },
                        "mode": {
                            "type": "string",
                            "description": "Mode to run in, must be start_play or run_server",
                            "enum": ["start_play", "run_server"]
                        }
                    },
                    "required": ["code", "mode"]
                })),
            },
            FunctionDeclaration {
                name: "get_studio_mode".to_string(),
                description: "Get the current studio mode. Returns the studio mode. The result will be one of start_play, run_server, or stop.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {}
                })),
            },
            FunctionDeclaration {
                name: "propose_plan".to_string(),
                description: "Propose a plan, UI layout, or architecture change to the user before executing it. You MUST use this tool first when the user asks to build a new system (e.g. shop UI, spectator mode, leaderboard, etc.) or write complex cross-script logic. This opens an interactive Artifacts view for the user to review, comment on, and approve your approach before you write the code. Do NOT use this for simple questions, tweaks, or isolated bug fixes. Render the plan exactly as markdown content.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "plan_text": {
                            "type": "string",
                            "description": "The full markdown text of the proposed plan."
                        }
                    },
                    "required": ["plan_text"]
                })),
            },
        ]),
        google_search: None,
        code_execution: None,
    }]
}

pub fn build_system_instruction(custom_prompt: Option<String>) -> GeminiContent {
    let mut text = "You are a Roblox Studio assistant. You must be aware of the current studio mode before using any tools. Infer the mode from conversation context or use get_studio_mode.\n\
        Use run_code to query data from Roblox Studio place or to change it.\n\
        After calling run_script_in_play_mode, the datamodel status will be reset to stop mode.\n\
        Prefer using start_stop_play tool instead of run_script_in_play_mode. Only use run_script_in_play_mode to run one time unit test code on server datamodel.\n\
        When receiving a request, evaluate its scope. If it involves designing a new system, creating complex cross-script logic, or building full UI layouts, ALWAYS use the `propose_plan` tool FIRST to present a structured markdown plan for user approval. For simple commands, minor bug fixes, or direct isolated code edits, proceed normally without proposing a plan.".to_string();

    if let Some(custom) = custom_prompt {
        text.push_str("\n\n");
        text.push_str(&custom);
    }

    GeminiContent {
        role: None,
        parts: vec![GeminiPart {
            text: Some(text),
            thought: None,
            thought_signature: None,
            inline_data: None,
            function_call: None,
            function_response: None,
        }],
    }
}

pub async fn stream_to_gemini(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    contents: &[GeminiContent],
    tools: &[ToolDeclaration],
    system_instruction: &GeminiContent,
    generation_config: Option<GenerationConfig>,
) -> color_eyre::Result<reqwest::Response> {
    let url = format!(
        "{}/{}:streamGenerateContent?alt=sse&key={}",
        GEMINI_API_BASE, model, api_key
    );

    let request_body = GeminiRequest {
        contents: contents.to_vec(),
        tools: if tools.is_empty() { None } else { Some(tools.to_vec()) },
        system_instruction: Some(system_instruction.clone()),
        generation_config,
    };

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        let err_body = response.text().await.unwrap_or_default();
        return Err(eyre!("API Error: {}", err_body));
    }

    Ok(response)
}

pub fn convert_function_call_to_tool_args(
    fc: &GeminiFunctionCall,
) -> color_eyre::Result<ToolArgumentValues> {
    match fc.name.as_str() {
        "run_code" => {
            let command = fc
                .args
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("run_code missing 'command' argument"))?
                .to_string();
            Ok(ToolArgumentValues::RunCode(RunCode { command }))
        }
        "insert_model" => {
            let query = fc
                .args
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("insert_model missing 'query' argument"))?
                .to_string();
            Ok(ToolArgumentValues::InsertModel(InsertModel { query }))
        }
        "get_console_output" => Ok(ToolArgumentValues::GetConsoleOutput(GetConsoleOutput {})),
        "start_stop_play" => {
            let mode = fc
                .args
                .get("mode")
                .and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("start_stop_play missing 'mode' argument"))?
                .to_string();
            Ok(ToolArgumentValues::StartStopPlay(StartStopPlay { mode }))
        }
        "run_script_in_play_mode" => {
            let code = fc
                .args
                .get("code")
                .and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("run_script_in_play_mode missing 'code' argument"))?
                .to_string();
            let timeout = fc.args.get("timeout").and_then(|v| v.as_u64()).map(|v| v as u32);
            let mode = fc
                .args
                .get("mode")
                .and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("run_script_in_play_mode missing 'mode' argument"))?
                .to_string();
            Ok(ToolArgumentValues::RunScriptInPlayMode(
                RunScriptInPlayMode {
                    code,
                    timeout,
                    mode,
                },
            ))
        }
        "get_studio_mode" => Ok(ToolArgumentValues::GetStudioMode(GetStudioMode {})),
        "propose_plan" => {
             let plan_text = fc
                .args
                .get("plan_text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("propose_plan missing 'plan_text' argument"))?
                .to_string();
             Ok(ToolArgumentValues::ProposePlan(ProposePlan { plan_text }))
        }
        other => Err(eyre!("Unknown function call: {}", other)),
    }
}

pub async fn dispatch_function_call(
    state: &PackedState,
    fc: &GeminiFunctionCall,
    chat_id: &str,
) -> color_eyre::Result<String> {
    if fc.name == "propose_plan" {
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        {
            let mut app_state = state.lock().await;
            app_state.plan_waiters.insert(chat_id.to_string(), tx);
        }
        
        tracing::debug!("Waiting for user feedback on proposed plan (chat_id: {})", chat_id);
        
        let result = rx.await.unwrap_or_else(|_| "Error: plan review modal was closed or connection lost.".to_string());
        return Ok(result);
    }

    let tool_args = convert_function_call_to_tool_args(fc)?;
    let (command, id) = ToolArguments::new(tool_args);
    tracing::debug!("Dispatching function call to Roblox Studio: {:?}", command);

    let (tx, mut rx) = mpsc::unbounded_channel::<Result<String>>();

    let trigger = {
        let mut app_state = state.lock().await;
        app_state.process_queue.push_back(command);
        app_state.output_map.insert(id, tx);
        app_state.trigger.clone()
    };

    trigger.send(()).map_err(|e| eyre!("Unable to trigger send: {}", e))?;

    let result = rx
        .recv()
        .await
        .ok_or_else(|| eyre!("Couldn't receive response from Roblox Studio"))?;

    {
        let mut app_state = state.lock().await;
        app_state.output_map.remove_entry(&id);
    }

    match result {
        Ok(response) => Ok(response),
        Err(err) => Ok(format!("Error: {}", err)),
    }
}

#[derive(Deserialize)]
pub struct ChatSendRequest {
    pub message: String,
    pub file_base64: Option<Vec<String>>,
    pub file_mime_type: Option<Vec<String>>,
    pub model: Option<String>,
    pub system_instruction: Option<String>,
    pub temperature: Option<f32>,
    pub thinking_level: Option<String>,
    pub enable_google_search: Option<bool>,
    pub enable_code_execution: Option<bool>,
    pub history: Option<Vec<GeminiContent>>,
}

#[derive(Serialize)]
pub struct ChatSendResponse {
    pub chat_id: String,
}

pub async fn chat_send_handler(
    State(state): State<PackedState>,
    Json(payload): Json<ChatSendRequest>,
) -> Result<impl IntoResponse> {
    let chat_id = Uuid::new_v4().to_string();
    let (sse_tx, sse_rx) = mpsc::channel::<SsePayload>(100);

    {
        let mut app_state = state.lock().await;
        app_state.chat_streams.insert(chat_id.clone(), sse_tx.clone());
        app_state.chat_receivers.insert(chat_id.clone(), sse_rx);

        if let Some(history) = &payload.history {
            app_state.conversation_history = history.clone();
        }

        let mut parts = Vec::new();

        if let (Some(base64_list), Some(mime_list)) = (&payload.file_base64, &payload.file_mime_type) {
            for (base64, mime) in base64_list.iter().zip(mime_list.iter()) {
                parts.push(GeminiPart {
                    text: None,
                    thought: None,
                    thought_signature: None,
                    inline_data: Some(InlineData {
                        mime_type: mime.clone(),
                        data: base64.clone(),
                    }),
                    function_call: None,
                    function_response: None,
                });
            }
        }

        parts.push(GeminiPart {
            text: Some(payload.message.clone()),
            thought: None,
            thought_signature: None,
            inline_data: None,
            function_call: None,
            function_response: None,
        });

        app_state.conversation_history.push(GeminiContent {
            role: Some("user".to_string()),
            parts,
        });
    }

    let state_clone = Arc::clone(&state);
    let chat_id_clone = chat_id.clone();
    
    // Unpack request parameters right before processing
    let opts = payload;

    let handle = tokio::spawn(async move {
        process_chat(state_clone.clone(), sse_tx, chat_id_clone.clone(), opts).await;
        // Clean up the abort handle when naturally finished
        let mut app_state = state_clone.lock().await;
        app_state.active_generations.remove(&chat_id_clone);
    });

    {
        let mut app_state = state.lock().await;
        app_state.active_generations.insert(chat_id.clone(), handle.abort_handle());
    }

    Ok(Json(ChatSendResponse { chat_id }))
}

#[derive(Deserialize)]
pub struct ChatStopRequest {
    pub chat_id: String,
}

pub async fn chat_stop_handler(
    State(state): State<PackedState>,
    Json(payload): Json<ChatStopRequest>,
) -> Result<impl IntoResponse> {
    let mut app_state = state.lock().await;
    
    if let Some(handle) = app_state.active_generations.remove(&payload.chat_id) {
        handle.abort();
        tracing::info!("Aborted Gemini generation for chat_id: {}", payload.chat_id);
    }
    
    Ok(axum::http::StatusCode::OK)
}

#[derive(Deserialize)]
pub struct ChatPlanResponseRequest {
    pub chat_id: String,
    pub response: String,
}

pub async fn chat_plan_response_handler(
    State(state): State<PackedState>,
    Json(payload): Json<ChatPlanResponseRequest>,
) -> Result<impl IntoResponse> {
    let mut app_state = state.lock().await;
    
    if let Some(tx) = app_state.plan_waiters.remove(&payload.chat_id) {
        let _ = tx.send(payload.response);
        Ok(axum::http::StatusCode::OK)
    } else {
        Err(crate::error::Report::from(eyre!("No pending plan review found for chat ID: {}", payload.chat_id)))
    }
}

async fn process_chat(
    state: PackedState,
    sse_tx: mpsc::Sender<SsePayload>,
    chat_id: String,
    opts: ChatSendRequest,
) {
    let client = reqwest::Client::new();
    
    let target_model = opts.model.unwrap_or_else(|| "gemini-1.5-flash".to_string());
    // Use the model name as-is from the frontend.

    
    // Gemini 3.1 Pro Preview does not yet support mixing Function Calling with built-in tools like Google Search
    let is_gemini_3 = target_model.starts_with("gemini-3.1");

    let mut tools = build_tool_declarations();
    if !is_gemini_3 && opts.enable_google_search.unwrap_or(false) {
        tools.push(ToolDeclaration {
            function_declarations: None,
            google_search: Some(serde_json::json!({})),
            code_execution: None,
        });
    }
    if !is_gemini_3 && opts.enable_code_execution.unwrap_or(false) {
        tools.push(ToolDeclaration {
            function_declarations: None,
            google_search: None,
            code_execution: Some(serde_json::json!({})),
        });
    }
    
    let system_instruction = build_system_instruction(opts.system_instruction);
    
    let mut generation_config = GenerationConfig::default();
    let mut has_gen_config = false;
    
    if let Some(t) = opts.temperature {
        generation_config.temperature = Some(t);
        has_gen_config = true;
    }
    if is_gemini_3 {
        let mut t_config = ThinkingConfig {
            thinking_level: None,
            include_thoughts: Some(true),
        };
        if let Some(level) = opts.thinking_level {
            if level != "none" {
                t_config.thinking_level = Some(level);
            }
        }
        generation_config.thinking_config = Some(t_config);
        has_gen_config = true;
    } else if let Some(level) = opts.thinking_level {
        if level != "none" {
            generation_config.thinking_config = Some(ThinkingConfig {
                thinking_level: Some(level),
                include_thoughts: Some(true),
            });
            has_gen_config = true;
        }
    }
    let gen_config = if has_gen_config { Some(generation_config) } else { None };

    let api_key = {
        let app_state = state.lock().await;
        app_state.api_key.clone()
    };

    let api_key = match api_key {
        Some(key) => key,
        None => {
            let _ = sse_tx.send(SsePayload::Error {
                error: "API key not set. Please configure it in settings.".to_string(),
            }).await;
            let _ = sse_tx.send(SsePayload::Done).await;
            return;
        }
    };

    loop {
        let history = {
            state.lock().await.conversation_history.clone()
        };

        let response = match stream_to_gemini(
            &client,
            &api_key,
            &target_model,
            &history,
            &tools,
            &system_instruction,
            gen_config.clone(),
        ).await {
            Ok(res) => res,
            Err(e) => {
                let _ = sse_tx.send(SsePayload::Error {
                    error: format!("{}", e),
                }).await;
                let _ = sse_tx.send(SsePayload::Done).await;
                break;
            }
        };

        use futures_util::StreamExt;

        let mut has_function_calls = false;
        let mut function_calls: Vec<GeminiFunctionCall> = Vec::new();
        let mut final_text = String::new();
        let mut final_thought = String::new();
        let mut final_thought_signature = None;

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_res) = stream.next().await {
            match chunk_res {
                Ok(bytes) => {
                    let chunk_str = String::from_utf8_lossy(&bytes);
                    buffer.push_str(&chunk_str);
                    
                    // Process lines in the buffer
                    while let Some(idx) = buffer.find('\n') {
                        let line = buffer[..idx].trim().to_string();
                        buffer = buffer[idx + 1..].to_string();

                        if line.starts_with("data: ") {
                            let data = line[6..].trim();
                            if data.is_empty() { continue; }

                            let chunk: GeminiResponse = match serde_json::from_str(data) {
                                Ok(c) => c,
                                Err(e) => {
                                    // Sometimes JSON can be split across lines in non-standard SSE.
                                    // But Gemini usually sends full JSON per data line.
                                    // If it fails, it might be a partial line, so we can try to put it back or skip.
                                    tracing::error!("Failed to parse Gemini JSON: {} | Data: {}", e, data);
                                    continue;
                                }
                            };

                            if let Some(err) = chunk.error {
                                let _ = sse_tx.send(SsePayload::Error {
                                    error: format!("Gemini API error {}: {}", err.code.unwrap_or(0), err.message),
                                }).await;
                                break;
                            }

                            if let Some(candidate) = chunk.candidates.as_ref().and_then(|c| c.first()) {
                                if let Some(content) = candidate.content.as_ref() {
                                    for part in &content.parts {
                                        if let Some(ref fc) = part.function_call {
                                            has_function_calls = true;
                                            function_calls.push(fc.clone());
                                        }
                                        if let Some(ref text) = part.text {
                                            if !text.is_empty() {
                                                if part.thought == Some(true) {
                                                    final_thought.push_str(text);
                                                    let _ = sse_tx.send(SsePayload::Thought {
                                                        content: text.clone(),
                                                    }).await;
                                                } else {
                                                    final_text.push_str(text);
                                                    let _ = sse_tx.send(SsePayload::Text {
                                                        content: text.clone(),
                                                    }).await;
                                                }
                                            }
                                        }
                                        if let Some(ref sig) = part.thought_signature {
                                            final_thought_signature = Some(sig.clone());
                                            let _ = sse_tx.send(SsePayload::ThoughtSignature {
                                                signature: sig.clone(),
                                            }).await;
                                        }
                                        // Also check for thoughtSignature as a hint that this might be a finished thought chunk
                                        if part.thought_signature.is_some() && final_thought.is_empty() && final_text.is_empty() {
                                            tracing::debug!("Recorded thought signature without text");
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = sse_tx.send(SsePayload::Error {
                        error: format!("Stream decode error: {}", e),
                    }).await;
                    break;
                }
            }
        }

        let mut full_parts: Vec<GeminiPart> = Vec::new();
        if has_function_calls {
            for fc in &function_calls {
                full_parts.push(GeminiPart {
                    text: None,
                    thought: None,
                    thought_signature: final_thought_signature.clone(),
                    inline_data: None,
                    function_call: Some(fc.clone()),
                    function_response: None,
                });
            }
        } else {
            if !final_thought.is_empty() {
                full_parts.push(GeminiPart {
                    text: Some(final_thought),
                    thought: Some(true),
                    thought_signature: None,
                    inline_data: None,
                    function_call: None,
                    function_response: None,
                });
            }
            if !final_text.is_empty() {
                full_parts.push(GeminiPart {
                    text: Some(final_text),
                    thought: None,
                    thought_signature: None,
                    inline_data: None,
                    function_call: None,
                    function_response: None,
                });
            }
        }

        {
            let mut app_state = state.lock().await;
            app_state.conversation_history.push(GeminiContent {
                role: Some("model".to_string()),
                parts: full_parts,
            });
        }

        if has_function_calls {
            let mut function_response_parts: Vec<GeminiPart> = Vec::new();

            for (idx, fc) in function_calls.iter().enumerate() {
                let _ = sse_tx.send(SsePayload::ToolCall {
                    name: fc.name.clone(),
                    args: fc.args.clone(),
                    call_index: idx,
                }).await;

                let result = dispatch_function_call(&state, fc, &chat_id).await;

                let result_str = match result {
                    Ok(r) => r,
                    Err(e) => format!("Error dispatching tool: {}", e),
                };

                let _ = sse_tx.send(SsePayload::ToolResult {
                    name: fc.name.clone(),
                    result: result_str.clone(),
                    call_index: idx,
                }).await;

                function_response_parts.push(GeminiPart {
                    text: None,
                    thought: None,
                    thought_signature: None,
                    inline_data: None,
                    function_call: None,
                    function_response: Some(GeminiFunctionResponse {
                        name: fc.name.clone(),
                        response: serde_json::json!({
                            "result": result_str
                        }),
                    }),
                });
            }

            {
                let mut app_state = state.lock().await;
                app_state.conversation_history.push(GeminiContent {
                    role: Some("user".to_string()),
                    parts: function_response_parts,
                });
            }

            continue;
        }

        let _ = sse_tx.send(SsePayload::Done).await;
        break;
    }

    {
        let mut app_state = state.lock().await;
        app_state.chat_streams.remove(&chat_id);
    }
}

pub async fn chat_events_handler(
    State(state): State<PackedState>,
    axum::extract::Path(chat_id): axum::extract::Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = std::result::Result<Event, Infallible>>> {
    let (real_tx, real_rx) = mpsc::channel::<std::result::Result<Event, Infallible>>(100);

    let state_clone = Arc::clone(&state);
    let chat_id_clone = chat_id.clone();
    
    // We get the receiver from the state, if it exists
    let payload_rx = {
        let mut app_state = state.lock().await;
        app_state.chat_receivers.remove(&chat_id)
    };

    tokio::spawn(async move {
        // If the chat somehow didn't exist, just close the SSE
        let mut payload_rx = match payload_rx {
            Some(rx) => rx,
            None => return,
        };

        // Loop until dropped or Done/Error received
        while let Some(payload) = payload_rx.recv().await {
            let event = match payload {
                SsePayload::ToolCall { name, args, call_index } => {
                    Event::default()
                        .event("tool_call")
                        .data(serde_json::json!({
                            "name": name,
                            "args": args,
                            "call_index": call_index,
                        }).to_string())
                }
                SsePayload::ToolResult { name, result, call_index } => {
                    Event::default()
                        .event("tool_result")
                        .data(serde_json::json!({
                            "name": name,
                            "result": result,
                            "call_index": call_index,
                        }).to_string())
                }
                SsePayload::Text { content } => {
                    Event::default()
                        .event("text")
                        .data(serde_json::json!({
                            "content": content,
                        }).to_string())
                }
                SsePayload::Thought { content } => {
                    Event::default()
                        .event("thinking")
                        .data(serde_json::json!({
                            "content": content,
                        }).to_string())
                }
                SsePayload::ThoughtSignature { signature } => {
                    Event::default()
                        .event("thought_signature")
                        .data(serde_json::json!({
                            "signature": signature,
                        }).to_string())
                }
                SsePayload::Done => {
                    let _ = real_tx.send(Ok(Event::default().event("done").data("{}"))).await;
                    break;
                }
                SsePayload::Error { error } => {
                    Event::default()
                        .event("error_msg")
                        .data(serde_json::json!({
                            "error": error,
                        }).to_string())
                }
            };
            
            if real_tx.send(Ok(event)).await.is_err() {
                break;
            }
        }
        
        // Remove from state on disconnect or completion
        {
            let mut app_state = state_clone.lock().await;
            app_state.chat_streams.remove(&chat_id_clone);
        }
    });

    Sse::new(tokio_stream::wrappers::ReceiverStream::new(real_rx)).keep_alive(KeepAlive::default())
}

#[derive(Deserialize)]
pub struct ApiKeyRequest {
    pub api_key: String,
}

pub async fn api_key_handler(
    State(state): State<PackedState>,
    Json(payload): Json<ApiKeyRequest>,
) -> Result<impl IntoResponse> {
    let mut app_state = state.lock().await;
    app_state.api_key = Some(payload.api_key);
    Ok(Json(serde_json::json!({ "success": true })))
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub api_key_set: bool,
}

pub async fn status_handler(
    State(state): State<PackedState>,
) -> Result<impl IntoResponse> {
    let app_state = state.lock().await;
    Ok(Json(StatusResponse {
        api_key_set: app_state.api_key.is_some(),
    }))
}

pub async fn models_handler(
    State(state): State<PackedState>,
) -> Result<impl IntoResponse> {
    let api_key = {
        let app_state = state.lock().await;
        app_state.api_key.clone()
    };

    let api_key = match api_key {
        Some(key) => key,
        None => return Err(Report::from(eyre!("API key not set"))),
    };

    let client = reqwest::Client::new();
    let mut all_models: Vec<GeminiModel> = Vec::new();
    let mut page_token: Option<String> = None;
    
    loop {
        let mut url = format!("https://generativelanguage.googleapis.com/v1beta/models?key={}&pageSize=1000", api_key);
        if let Some(ref token) = page_token {
            url.push_str(&format!("&pageToken={}", token));
        }
        
        let res = client.get(&url).send().await?;
        if !res.status().is_success() {
            let txt = res.text().await?;
            return Err(Report::from(eyre!("Failed to fetch models: {}", txt)));
        }
        
        let data: ListModelsResponse = res.json().await?;
        all_models.extend(data.models);
        
        match data.next_page_token {
            Some(token) if !token.is_empty() => page_token = Some(token),
            _ => break,
        }
    }
    
    // Filter models that support generateContent
    let filtered_models: Vec<GeminiModel> = all_models
        .into_iter()
        .filter(|m| m.supported_generation_methods.iter().any(|s| s == "generateContent"))
        .filter(|m| {
            // Exclude models that don't support function calling (tools)
            let name = m.name.to_lowercase();
            let short = name.split('/').last().unwrap_or(&name);
            !short.starts_with("gemma")
                && !short.contains("-tts")
                && !short.starts_with("embedding-")
                && !short.starts_with("text-embedding-")
                && !short.contains("-aqa")
                && !short.starts_with("imagen-")
                && !short.starts_with("learnlm-")
        })
        .collect();
        
    Ok(Json(ListModelsResponse { models: filtered_models, next_page_token: None }))
}

pub async fn request_handler(State(state): State<PackedState>) -> Result<impl IntoResponse> {
    let timeout = tokio::time::timeout(LONG_POLL_DURATION, async {
        let mut waiter = { state.lock().await.waiter.clone() };
        loop {
            {
                let mut state = state.lock().await;
                if let Some(task) = state.process_queue.pop_front() {
                    return Ok::<ToolArguments, Error>(task);
                }
            }
            waiter.changed().await?
        }
    })
    .await;
    match timeout {
        Ok(result) => Ok(Json(result?).into_response()),
        _ => Ok((StatusCode::LOCKED, String::new()).into_response()),
    }
}

pub async fn response_handler(
    State(state): State<PackedState>,
    Json(payload): Json<RunCommandResponse>,
) -> Result<impl IntoResponse> {
    tracing::debug!("Received reply from studio {:?}", payload);
    let mut state = state.lock().await;
    let tx = state
        .output_map
        .remove(&payload.id)
        .ok_or_eyre("Unknown ID")?;
    let result: Result<String, Report> = if payload.success {
        Ok(payload.response)
    } else {
        Err(Report::from(eyre!(payload.response)))
    };
    Ok(tx.send(result)?)
}

pub async fn proxy_handler(
    State(state): State<PackedState>,
    Json(command): Json<ToolArguments>,
) -> Result<impl IntoResponse> {
    let id = command.id.ok_or_eyre("Got proxy command with no id")?;
    tracing::debug!("Received request to proxy {:?}", command);
    let (tx, mut rx) = mpsc::unbounded_channel();
    {
        let mut state = state.lock().await;
        state.process_queue.push_back(command);
        state.output_map.insert(id, tx);
    }
    let result = rx.recv().await.ok_or_eyre("Couldn't receive response")?;
    {
        let mut state = state.lock().await;
        state.output_map.remove_entry(&id);
    }
    let (success, response) = match result {
        Ok(s) => (true, s),
        Err(e) => (false, e.to_string()),
    };
    tracing::debug!("Sending back to dud: success={}, response={:?}", success, response);
    Ok(Json(RunCommandResponse {
        success,
        response,
        id,
    }))
}

pub async fn dud_proxy_loop(state: PackedState, exit: Receiver<()>) {
    let client = reqwest::Client::new();

    let mut waiter = { state.lock().await.waiter.clone() };
    while exit.is_empty() {
        let entry = { state.lock().await.process_queue.pop_front() };
        if let Some(entry) = entry {
            let res = client
                .post(format!("http://127.0.0.1:{STUDIO_PLUGIN_PORT}/proxy"))
                .json(&entry)
                .send()
                .await;
            if let Ok(res) = res {
                let tx = {
                    state
                        .lock()
                        .await
                        .output_map
                        .remove(&entry.id.unwrap())
                        .unwrap()
                };
                let res = res
                    .json::<RunCommandResponse>()
                    .await
                    .map(|r| r.response)
                    .map_err(Into::into);
                tx.send(res).unwrap();
            } else {
                tracing::error!("Failed to proxy: {:?}", res);
            };
        } else {
            waiter.changed().await.unwrap();
        }
    }
}

// --- History Operations ---

use crate::db::{ChatSession, ChatSessionFull};

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

pub async fn history_search_handler(
    State(state): State<PackedState>,
    axum::extract::Query(query): axum::extract::Query<SearchQuery>,
) -> Result<Json<Vec<ChatSession>>, axum::http::StatusCode> {
    let state = state.lock().await;
    match state.db.search_chats(&query.q) {
        Ok(chats) => Ok(Json(chats)),
        Err(e) => {
            tracing::error!("Failed to search chats: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn history_list_handler(
    State(state): State<PackedState>,
) -> Result<Json<Vec<ChatSession>>, axum::http::StatusCode> {
    let state = state.lock().await;
    match state.db.list_chats() {
        Ok(chats) => Ok(Json(chats)),
        Err(e) => {
            tracing::error!("Failed to list chats: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn history_load_handler(
    State(state): State<PackedState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<ChatSessionFull>, axum::http::StatusCode> {
    let state = state.lock().await;
    match state.db.load_chat(&id) {
        Ok(Some(chat)) => Ok(Json(chat)),
        Ok(None) => Err(axum::http::StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to load chat: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn history_save_handler(
    State(state): State<PackedState>,
    Json(chat): Json<ChatSessionFull>,
) -> Result<axum::http::StatusCode, axum::http::StatusCode> {
    let state = state.lock().await;
    match state.db.save_chat(&chat) {
        Ok(_) => Ok(axum::http::StatusCode::OK),
        Err(e) => {
            tracing::error!("Failed to save chat: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn history_delete_handler(
    State(state): State<PackedState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::http::StatusCode, axum::http::StatusCode> {
    let state = state.lock().await;
    match state.db.delete_chat(&id) {
        Ok(_) => Ok(axum::http::StatusCode::OK),
        Err(e) => {
            tracing::error!("Failed to delete chat: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn history_clear_all_handler(
    State(state): State<PackedState>,
) -> Result<axum::http::StatusCode, axum::http::StatusCode> {
    let state = state.lock().await;
    match state.db.clear_all() {
        Ok(_) => Ok(axum::http::StatusCode::OK),
        Err(e) => {
            tracing::error!("Failed to clear all chats: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(serde::Deserialize)]
pub struct UpdateTitleRequest {
    pub id: String,
    pub title: String,
}

pub async fn history_update_title_handler(
    State(state): State<PackedState>,
    Json(req): Json<UpdateTitleRequest>,
) -> Result<axum::http::StatusCode, axum::http::StatusCode> {
    let state = state.lock().await;
    match state.db.update_title(&req.id, &req.title) {
        Ok(_) => Ok(axum::http::StatusCode::OK),
        Err(e) => {
            tracing::error!("Failed to update title: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(serde::Deserialize)]
pub struct TogglePinRequest {
    pub id: String,
    pub pinned: bool,
}

pub async fn history_toggle_pin_handler(
    State(state): State<PackedState>,
    Json(req): Json<TogglePinRequest>,
) -> Result<axum::http::StatusCode, axum::http::StatusCode> {
    let state = state.lock().await;
    match state.db.toggle_pin(&req.id, req.pinned) {
        Ok(_) => Ok(axum::http::StatusCode::OK),
        Err(e) => {
            tracing::error!("Failed to toggle pin: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
