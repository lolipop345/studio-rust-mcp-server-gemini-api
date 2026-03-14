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
    pub current_tool: Option<String>,
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
            current_tool: None,
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
pub struct AskPlanningQuestion {
    pub question: String,
    pub options: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ReadFile {
    pub path: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ReadLines {
    pub path: String,
    #[serde(rename = "startLine")]
    pub start_line: u32,
    #[serde(rename = "endLine")]
    pub end_line: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GetHierarchy {
    #[serde(rename = "includeBaseParts")]
    pub include_base_parts: Option<bool>,
    #[serde(rename = "maxDepth")]
    pub max_depth: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GetHierarchyOf {
    pub path: String,
    #[serde(rename = "includeBaseParts")]
    pub include_base_parts: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GetProperties {
    pub path: String,
    pub properties: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FindInstances {
    pub name: Option<String>,
    #[serde(rename = "className")]
    pub class_name: Option<String>,
    pub root: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AddInstance {
    #[serde(rename = "className")]
    pub class_name: String,
    pub parent: String,
    pub properties: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RemoveInstance {
    pub path: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AddJsonInstance {
    pub json: serde_json::Value,
    pub parent: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ReplaceLinesWith {
    pub path: String,
    #[serde(rename = "startLine")]
    pub start_line: u32,
    #[serde(rename = "endLine")]
    pub end_line: u32,
    #[serde(rename = "newContent")]
    pub new_content: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ReplaceWith {
    pub path: String,
    #[serde(rename = "newSource")]
    pub new_source: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MoveInstance {
    pub path: String,
    #[serde(rename = "newParent")]
    pub new_parent: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CloneInstance {
    pub path: String,
    #[serde(rename = "newParent")]
    pub new_parent: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImportFromHttp {
    pub url: String,
    pub parent: String,
    #[serde(rename = "instanceType")]
    pub instance_type: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImportFromWally {
    pub package: String,
    pub parent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateScript {
    #[serde(rename = "scriptType")]
    pub script_type: String,
    pub name: String,
    pub source: String,
    pub parent: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DebugScript {
    pub code: String,
    pub cleanup: Option<bool>,
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
    AskPlanningQuestion(AskPlanningQuestion),
    ReadFile(ReadFile),
    ReadLines(ReadLines),
    GetHierarchy(GetHierarchy),
    GetHierarchyOf(GetHierarchyOf),
    GetProperties(GetProperties),
    FindInstances(FindInstances),
    AddInstance(AddInstance),
    RemoveInstance(RemoveInstance),
    AddJsonInstance(AddJsonInstance),
    ReplaceLinesWith(ReplaceLinesWith),
    ReplaceWith(ReplaceWith),
    MoveInstance(MoveInstance),
    CloneInstance(CloneInstance),
    ImportFromHttp(ImportFromHttp),
    ImportFromWally(ImportFromWally),
    CreateScript(CreateScript),
    DebugScript(DebugScript),
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
            FunctionDeclaration {
                name: "ask_planning_question".to_string(),
                description: "Ask the user a clarifying question before creating a plan. Use this BEFORE `propose_plan` when the user's request is ambiguous or has multiple valid implementation approaches (e.g., which UI framework, whether to use ModuleScripts, preferred visual style, complexity level). Provide 2-4 distinct options. The user can also type a custom answer. Ask at most 2 questions before proposing the plan — keep it focused.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "question": {
                            "type": "string",
                            "description": "The clarifying question to ask the user (e.g., 'Which UI framework should I use?')"
                        },
                        "options": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "2-4 distinct options for the user to choose from (e.g., ['ScreenGui / Frames', 'Fusion reactive UI', 'React-Lua'])"
                        }
                    },
                    "required": ["question", "options"]
                })),
            },
            // ── Agent Tools ──────────────────────────────────────────
            FunctionDeclaration {
                name: "read_file".to_string(),
                description: "Read the full source code of a Script, LocalScript, or ModuleScript. Returns the entire Source property. Use this before making edits to understand current code.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Dot-separated path to the script (e.g. 'ServerScriptService.MainScript')"
                        }
                    },
                    "required": ["path"]
                })),
            },
            FunctionDeclaration {
                name: "read_lines".to_string(),
                description: "Read specific lines from a Script/LocalScript/ModuleScript source. Returns numbered lines in the given range. Use this to inspect a portion of a large script.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Dot-separated path to the script"
                        },
                        "startLine": {
                            "type": "integer",
                            "description": "First line number to read (1-based)"
                        },
                        "endLine": {
                            "type": "integer",
                            "description": "Last line number to read (1-based, inclusive)"
                        }
                    },
                    "required": ["path", "startLine", "endLine"]
                })),
            },
            FunctionDeclaration {
                name: "get_hierarchy".to_string(),
                description: "Get the overall hierarchy of the game as JSON. Returns top-level services (Workspace, ServerScriptService, ReplicatedStorage, etc.) and their children up to maxDepth. BaseParts are excluded by default (only count shown) to keep output manageable.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "includeBaseParts": {
                            "type": "boolean",
                            "description": "Whether to include BasePart instances in the output (default: false, only shows count)"
                        },
                        "maxDepth": {
                            "type": "integer",
                            "description": "Maximum depth to traverse (default: 3)"
                        }
                    }
                })),
            },
            FunctionDeclaration {
                name: "get_hierarchy_of".to_string(),
                description: "Get the hierarchy of a specific instance and its descendants as JSON. Returns the instance tree up to depth 10.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Dot-separated path to the instance (e.g. 'Workspace.Map')"
                        },
                        "includeBaseParts": {
                            "type": "boolean",
                            "description": "Whether to include BasePart instances (default: false)"
                        }
                    },
                    "required": ["path"]
                })),
            },
            FunctionDeclaration {
                name: "get_properties".to_string(),
                description: "Read properties of an instance. If specific property names are given, returns only those. Otherwise returns common properties (Name, ClassName, Parent, Position, Size, Color, etc.).".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Dot-separated path to the instance"
                        },
                        "properties": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional list of specific property names to read"
                        }
                    },
                    "required": ["path"]
                })),
            },
            FunctionDeclaration {
                name: "find_instances".to_string(),
                description: "Search for instances by name pattern and/or ClassName. Returns up to 50 matching instances with their full paths. At least one of name or className must be provided.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Name or pattern to search for (case-insensitive substring match)"
                        },
                        "className": {
                            "type": "string",
                            "description": "ClassName to filter by (exact match)"
                        },
                        "root": {
                            "type": "string",
                            "description": "Dot-separated path to search root (default: searches entire game)"
                        }
                    }
                })),
            },
            FunctionDeclaration {
                name: "add_instance".to_string(),
                description: "Create a new Instance and parent it. Supports setting properties with automatic type conversion (arrays → Vector3/Color3/UDim2, strings → Enum values).".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "className": {
                            "type": "string",
                            "description": "The ClassName of the instance to create (e.g. 'Part', 'Script', 'Folder')"
                        },
                        "parent": {
                            "type": "string",
                            "description": "Dot-separated path to the parent instance"
                        },
                        "properties": {
                            "type": "object",
                            "description": "Optional properties to set. Values can be: number, string, boolean, [x,y,z] for Vector3, [r,g,b] for Color3, [sx,ox,sy,oy] for UDim2, 'EnumType.Value' for enums"
                        }
                    },
                    "required": ["className", "parent"]
                })),
            },
            FunctionDeclaration {
                name: "remove_instance".to_string(),
                description: "Remove (Destroy) an instance at the given path. This action is undoable via Ctrl+Z.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Dot-separated path to the instance to remove"
                        }
                    },
                    "required": ["path"]
                })),
            },
            FunctionDeclaration {
                name: "add_json_instance".to_string(),
                description: "Create a tree of instances from a JSON definition. Each node: { \"ClassName\": string, \"Name\"?: string, \"Properties\"?: {}, \"Children\"?: [] }. Useful for building complex hierarchies in one call.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "json": {
                            "type": "object",
                            "description": "JSON tree definition. Format: { ClassName: string, Name?: string, Properties?: { prop: value }, Children?: [same format] }"
                        },
                        "parent": {
                            "type": "string",
                            "description": "Dot-separated path to the parent instance"
                        }
                    },
                    "required": ["json", "parent"]
                })),
            },
            FunctionDeclaration {
                name: "replace_lines_with".to_string(),
                description: "Replace specific lines in a Script/LocalScript/ModuleScript source. You MUST use read_file or read_lines first to see the current content before using this tool.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Dot-separated path to the script"
                        },
                        "startLine": {
                            "type": "integer",
                            "description": "First line to replace (1-based)"
                        },
                        "endLine": {
                            "type": "integer",
                            "description": "Last line to replace (1-based, inclusive)"
                        },
                        "newContent": {
                            "type": "string",
                            "description": "The new content to insert in place of the specified lines"
                        }
                    },
                    "required": ["path", "startLine", "endLine", "newContent"]
                })),
            },
            FunctionDeclaration {
                name: "replace_with".to_string(),
                description: "Replace the entire Source of a Script/LocalScript/ModuleScript. You MUST use read_file first to see the current content before using this tool.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Dot-separated path to the script"
                        },
                        "newSource": {
                            "type": "string",
                            "description": "The new full source code"
                        }
                    },
                    "required": ["path", "newSource"]
                })),
            },
            FunctionDeclaration {
                name: "move_instance".to_string(),
                description: "Move an instance to a new parent. The instance keeps its properties and children.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Dot-separated path to the instance to move"
                        },
                        "newParent": {
                            "type": "string",
                            "description": "Dot-separated path to the new parent"
                        }
                    },
                    "required": ["path", "newParent"]
                })),
            },
            FunctionDeclaration {
                name: "clone_instance".to_string(),
                description: "Clone an instance (deep copy including children). Optionally place the clone under a different parent.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Dot-separated path to the instance to clone"
                        },
                        "newParent": {
                            "type": "string",
                            "description": "Dot-separated path to the parent for the clone (default: same parent as original)"
                        }
                    },
                    "required": ["path"]
                })),
            },
            FunctionDeclaration {
                name: "import_from_http".to_string(),
                description: "Download content from a URL and create a Script/LocalScript/ModuleScript with that content as its Source. Useful for importing libraries from GitHub raw URLs.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "URL to fetch the source code from"
                        },
                        "parent": {
                            "type": "string",
                            "description": "Dot-separated path to the parent instance"
                        },
                        "instanceType": {
                            "type": "string",
                            "description": "Type of script to create: Script, LocalScript, or ModuleScript",
                            "enum": ["Script", "LocalScript", "ModuleScript"]
                        },
                        "name": {
                            "type": "string",
                            "description": "Name for the new script instance"
                        }
                    },
                    "required": ["url", "parent", "instanceType", "name"]
                })),
            },
            FunctionDeclaration {
                name: "import_from_wally".to_string(),
                description: "Import a Wally package into the game. Downloads the package using the Wally CLI and creates ModuleScript instances from the package files. Requires Wally to be installed on the system.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "package": {
                            "type": "string",
                            "description": "Wally package identifier (e.g. 'jsdotlua/react@17.1.0')"
                        },
                        "parent": {
                            "type": "string",
                            "description": "Dot-separated path to the parent instance where the package will be placed"
                        }
                    },
                    "required": ["package", "parent"]
                })),
            },
            FunctionDeclaration {
                name: "create_script".to_string(),
                description: "Create a new Script, LocalScript, or ModuleScript with the given source code under the specified parent. Use this instead of add_instance when you need to create a script with source code.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "scriptType": {
                            "type": "string",
                            "description": "Type of script to create",
                            "enum": ["Script", "LocalScript", "ModuleScript"]
                        },
                        "name": {
                            "type": "string",
                            "description": "Name of the new script"
                        },
                        "source": {
                            "type": "string",
                            "description": "The Luau source code for the script"
                        },
                        "parent": {
                            "type": "string",
                            "description": "Dot-separated path to the parent instance (e.g. 'ServerScriptService', 'ReplicatedStorage.Modules')"
                        }
                    },
                    "required": ["scriptType", "name", "source", "parent"]
                })),
            },
            FunctionDeclaration {
                name: "debug_script".to_string(),
                description: "Debug/test Luau code in a sandbox environment. Runs the code, captures all output (print/warn/error), tracks every Instance created via Instance.new(), reports runtime errors with details, then automatically cleans up (destroys) all created instances. Use this to verify code correctness before deploying it. Set cleanup=false to keep created instances alive for inspection.".to_string(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "code": {
                            "type": "string",
                            "description": "The Luau code to debug/test in sandbox mode"
                        },
                        "cleanup": {
                            "type": "boolean",
                            "description": "Whether to destroy all created instances after execution (default: true)"
                        }
                    },
                    "required": ["code"]
                })),
            },
        ]),
        google_search: None,
        code_execution: None,
    }]
}

pub fn build_system_instruction(custom_prompt: Option<String>, thinking_level: Option<&str>) -> GeminiContent {
    let mut text = "You are a Roblox Studio assistant. You must be aware of the current studio mode before using any tools. Infer the mode from conversation context or use get_studio_mode.\n\
        \n\
        ## Tool Usage Guidelines\n\
        \n\
        ### Reading & Exploring\n\
        - Use `get_hierarchy` first to understand the game structure. BaseParts are excluded by default (only count shown).\n\
        - Use `get_hierarchy_of` to inspect a specific subtree in detail.\n\
        - Use `find_instances` to search by name/className across the game.\n\
        - Use `get_properties` to read specific properties of an instance.\n\
        - Use `read_file` to read a script's full source. Use `read_lines` for specific line ranges.\n\
        \n\
        ### Editing Scripts\n\
        - **ALWAYS** use `read_file` or `read_lines` BEFORE using `replace_lines_with` or `replace_with`. Never edit blindly.\n\
        - Use `replace_lines_with` for surgical edits (specific line ranges).\n\
        - Use `replace_with` only when rewriting the entire script source.\n\
        \n\
        ### Creating & Modifying Instances\n\
        - Use `add_instance` for single instances with properties.\n\
        - Use `add_json_instance` for complex hierarchies. JSON format: `{ \"ClassName\": \"Frame\", \"Name\": \"MyFrame\", \"Properties\": { \"Size\": [0, 200, 0, 100] }, \"Children\": [...] }`\n\
        - Property value conversion: `[x,y,z]` → Vector3, `[r,g,b]` → Color3, `[sx,ox,sy,oy]` → UDim2, `[x,y]` → Vector2/UDim, `\"EnumType.Value\"` → Enum.\n\
        - Use `move_instance` to reparent, `clone_instance` to duplicate, `remove_instance` to delete.\n\
        \n\
        ### Importing\n\
        - Use `import_from_http` to download and import Luau/Lua scripts from any URL directly into the game tree.\n\
        - When you find useful open-source Roblox libraries or utilities online, use `import_from_http` with the raw GitHub URL to bring them in.\n\
        - The backend fetches the file content, so provide the **raw** URL (e.g., `https://raw.githubusercontent.com/...`).\n\
        - After importing, you can use `read_file` + `replace_with` to adapt the code if needed (e.g., fix require paths, rename, etc.).\n\
        - Use `import_from_wally` to install Wally packages. Wally is auto-downloaded if missing.\n\
        \n\
        ### Creating Scripts\n\
        - Use `create_script` to create Script, LocalScript, or ModuleScript with source code under a parent.\n\
        \n\
        ### Legacy Tools\n\
        - Use `run_code` for ad-hoc Luau execution, querying data, or operations not covered by structured tools.\n\
        - Prefer structured tools over `run_code` when available.\n\
        After calling run_script_in_play_mode, the datamodel status will be reset to stop mode.\n\
        Prefer using start_stop_play tool instead of run_script_in_play_mode. Only use run_script_in_play_mode to run one time unit test code on server datamodel.\n\
        \n\
        ### Planning\n\
        When receiving a request, evaluate its scope:\n\
        - For SIMPLE commands, minor bug fixes, or isolated code edits: proceed normally.\n\
        - For COMPLEX tasks (new systems, full UI layouts, cross-script logic): first use `ask_planning_question` (at most 2 times) to clarify ambiguous choices, THEN use `propose_plan` to show a structured markdown plan before writing any code.\n\
        `ask_planning_question` opens an interactive card in the UI — keep questions short and options concrete (e.g., 'Fusion / React-Lua / Plain ScreenGui').".to_string();

    // Debugging instructions only for thoughtful/planning modes (not fast/none)
    let is_thinking = thinking_level
        .map(|l| !l.is_empty() && l != "none")
        .unwrap_or(false);

    if is_thinking {
        text.push_str("\n\
        \n\
        ### Debugging (Thoughtful Mode)\n\
        - **ALWAYS** use `debug_script` after writing or modifying code to verify it works correctly.\n\
        - `debug_script` runs code in a sandbox, tracks all Instance.new() calls, captures output, and reports errors.\n\
        - After debug, all sandbox instances are automatically cleaned up (set cleanup=false to inspect).\n\
        - If debug reveals errors, fix the code and debug again until it passes.");
    }

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

/// Returns (url, is_oauth) — if the credential starts with "Bearer " it is an OAuth token.
fn build_gemini_url(base: &str, model: &str, credential: &str) -> (String, bool) {
    if credential.starts_with("Bearer ") {
        (format!("{}/{}:streamGenerateContent?alt=sse", base, model), true)
    } else {
        (format!("{}/{}:streamGenerateContent?alt=sse&key={}", base, model, credential), false)
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
    let (url, is_oauth) = build_gemini_url(GEMINI_API_BASE, model, api_key);

    let request_body = GeminiRequest {
        contents: contents.to_vec(),
        tools: if tools.is_empty() { None } else { Some(tools.to_vec()) },
        system_instruction: Some(system_instruction.clone()),
        generation_config,
    };

    let mut builder = client
        .post(&url)
        .header("Content-Type", "application/json");

    if is_oauth {
        builder = builder.header("Authorization", api_key);
    }

    let response = builder
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
        "ask_planning_question" => {
            let question = fc
                .args
                .get("question")
                .and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("ask_planning_question missing 'question' argument"))?
                .to_string();
            let options = fc
                .args
                .get("options")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>())
                .unwrap_or_default();
            Ok(ToolArgumentValues::AskPlanningQuestion(AskPlanningQuestion { question, options }))
        }
        // ── Agent Tools ──────────────────────────────────────────
        "read_file" => {
            let path = fc.args.get("path").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("read_file missing 'path'"))?.to_string();
            Ok(ToolArgumentValues::ReadFile(ReadFile { path }))
        }
        "read_lines" => {
            let path = fc.args.get("path").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("read_lines missing 'path'"))?.to_string();
            let start_line = fc.args.get("startLine").and_then(|v| v.as_u64())
                .ok_or_else(|| eyre!("read_lines missing 'startLine'"))? as u32;
            let end_line = fc.args.get("endLine").and_then(|v| v.as_u64())
                .ok_or_else(|| eyre!("read_lines missing 'endLine'"))? as u32;
            Ok(ToolArgumentValues::ReadLines(ReadLines { path, start_line, end_line }))
        }
        "get_hierarchy" => {
            let include_base_parts = fc.args.get("includeBaseParts").and_then(|v| v.as_bool());
            let max_depth = fc.args.get("maxDepth").and_then(|v| v.as_u64()).map(|v| v as u32);
            Ok(ToolArgumentValues::GetHierarchy(GetHierarchy { include_base_parts, max_depth }))
        }
        "get_hierarchy_of" => {
            let path = fc.args.get("path").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("get_hierarchy_of missing 'path'"))?.to_string();
            let include_base_parts = fc.args.get("includeBaseParts").and_then(|v| v.as_bool());
            Ok(ToolArgumentValues::GetHierarchyOf(GetHierarchyOf { path, include_base_parts }))
        }
        "get_properties" => {
            let path = fc.args.get("path").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("get_properties missing 'path'"))?.to_string();
            let properties = fc.args.get("properties").and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>());
            Ok(ToolArgumentValues::GetProperties(GetProperties { path, properties }))
        }
        "find_instances" => {
            let name = fc.args.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
            let class_name = fc.args.get("className").and_then(|v| v.as_str()).map(|s| s.to_string());
            let root = fc.args.get("root").and_then(|v| v.as_str()).map(|s| s.to_string());
            Ok(ToolArgumentValues::FindInstances(FindInstances { name, class_name, root }))
        }
        "add_instance" => {
            let class_name = fc.args.get("className").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("add_instance missing 'className'"))?.to_string();
            let parent = fc.args.get("parent").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("add_instance missing 'parent'"))?.to_string();
            let properties = fc.args.get("properties").cloned();
            Ok(ToolArgumentValues::AddInstance(AddInstance { class_name, parent, properties }))
        }
        "remove_instance" => {
            let path = fc.args.get("path").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("remove_instance missing 'path'"))?.to_string();
            Ok(ToolArgumentValues::RemoveInstance(RemoveInstance { path }))
        }
        "add_json_instance" => {
            let json = fc.args.get("json").cloned()
                .ok_or_else(|| eyre!("add_json_instance missing 'json'"))?;
            let parent = fc.args.get("parent").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("add_json_instance missing 'parent'"))?.to_string();
            Ok(ToolArgumentValues::AddJsonInstance(AddJsonInstance { json, parent }))
        }
        "replace_lines_with" => {
            let path = fc.args.get("path").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("replace_lines_with missing 'path'"))?.to_string();
            let start_line = fc.args.get("startLine").and_then(|v| v.as_u64())
                .ok_or_else(|| eyre!("replace_lines_with missing 'startLine'"))? as u32;
            let end_line = fc.args.get("endLine").and_then(|v| v.as_u64())
                .ok_or_else(|| eyre!("replace_lines_with missing 'endLine'"))? as u32;
            let new_content = fc.args.get("newContent").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("replace_lines_with missing 'newContent'"))?.to_string();
            Ok(ToolArgumentValues::ReplaceLinesWith(ReplaceLinesWith { path, start_line, end_line, new_content }))
        }
        "replace_with" => {
            let path = fc.args.get("path").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("replace_with missing 'path'"))?.to_string();
            let new_source = fc.args.get("newSource").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("replace_with missing 'newSource'"))?.to_string();
            Ok(ToolArgumentValues::ReplaceWith(ReplaceWith { path, new_source }))
        }
        "move_instance" => {
            let path = fc.args.get("path").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("move_instance missing 'path'"))?.to_string();
            let new_parent = fc.args.get("newParent").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("move_instance missing 'newParent'"))?.to_string();
            Ok(ToolArgumentValues::MoveInstance(MoveInstance { path, new_parent }))
        }
        "clone_instance" => {
            let path = fc.args.get("path").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("clone_instance missing 'path'"))?.to_string();
            let new_parent = fc.args.get("newParent").and_then(|v| v.as_str()).map(|s| s.to_string());
            Ok(ToolArgumentValues::CloneInstance(CloneInstance { path, new_parent }))
        }
        "import_from_http" => {
            let url = fc.args.get("url").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("import_from_http missing 'url'"))?.to_string();
            let parent = fc.args.get("parent").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("import_from_http missing 'parent'"))?.to_string();
            let instance_type = fc.args.get("instanceType").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("import_from_http missing 'instanceType'"))?.to_string();
            let name = fc.args.get("name").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("import_from_http missing 'name'"))?.to_string();
            Ok(ToolArgumentValues::ImportFromHttp(ImportFromHttp { url, parent, instance_type, name }))
        }
        "import_from_wally" => {
            let package = fc.args.get("package").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("import_from_wally missing 'package'"))?.to_string();
            let parent = fc.args.get("parent").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("import_from_wally missing 'parent'"))?.to_string();
            Ok(ToolArgumentValues::ImportFromWally(ImportFromWally { package, parent, json: None }))
        }
        "create_script" => {
            let script_type = fc.args.get("scriptType").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("create_script missing 'scriptType'"))?.to_string();
            let name = fc.args.get("name").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("create_script missing 'name'"))?.to_string();
            let source = fc.args.get("source").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("create_script missing 'source'"))?.to_string();
            let parent = fc.args.get("parent").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("create_script missing 'parent'"))?.to_string();
            Ok(ToolArgumentValues::CreateScript(CreateScript { script_type, name, source, parent }))
        }
        "debug_script" => {
            let code = fc.args.get("code").and_then(|v| v.as_str())
                .ok_or_else(|| eyre!("debug_script missing 'code'"))?.to_string();
            let cleanup = fc.args.get("cleanup").and_then(|v| v.as_bool());
            Ok(ToolArgumentValues::DebugScript(DebugScript { code, cleanup }))
        }
        other => Err(eyre!("Unknown function call: {}", other)),
    }
}

/// Runs `wally install` for a given package, reads the resulting files, and returns a JSON file tree.
async fn run_wally_install(package: &str) -> color_eyre::Result<serde_json::Value> {
    // Check if wally is available
    let wally_check = tokio::process::Command::new("which")
        .arg("wally")
        .output()
        .await;

    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let projects_dir = working_dir.join("Projects");
    let tmp_path = projects_dir.join("auto_wally");
    
    std::fs::create_dir_all(&tmp_path)?;

    // Determine Wally paths
    let has_global_wally = wally_check.is_ok() && wally_check.unwrap().status.success();
    let local_wally_bin = if std::env::consts::OS == "windows" {
        tmp_path.join("wally.exe")
    } else {
        tmp_path.join("wally")
    };

    // Auto-download Wally if missing globally
    if !has_global_wally && !local_wally_bin.exists() {
        let os_str = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos", _) => "macos",
            ("windows", _) => "windows",
            _ => "linux",
        };
        let download_url = format!("https://github.com/UpliftGames/wally/releases/download/v0.3.2/wally-v0.3.2-{}.zip", os_str);
        
        // Download zip
        let _ = tokio::process::Command::new("curl")
            .args(&["-L", "-o", "wally.zip", &download_url])
            .current_dir(&tmp_path)
            .output()
            .await;
            
        // Unzip
        if std::env::consts::OS == "windows" {
            let _ = tokio::process::Command::new("powershell")
                .args(&["-Command", "Expand-Archive -Path wally.zip -DestinationPath . -Force"])
                .current_dir(&tmp_path)
                .output()
                .await;
        } else {
            let _ = tokio::process::Command::new("unzip")
                .args(&["-o", "wally.zip"])
                .current_dir(&tmp_path)
                .output()
                .await;
                
            let _ = tokio::process::Command::new("chmod")
                .args(&["+x", "wally"])
                .current_dir(&tmp_path)
                .output()
                .await;
        }
    }



    // Parse package name for wally.toml (e.g., "jsdotlua/react@17.1.0" → name = "react", dep = "jsdotlua/react@17.1.0")
    let pkg_name = package
        .split('/')
        .last()
        .unwrap_or(package)
        .split('@')
        .next()
        .unwrap_or("package");

    let wally_toml = format!(
        r#"[package]
name = "temp/wally-import"
version = "0.1.0"
registry = "https://github.com/UpliftGames/wally-index"
realm = "shared"

[dependencies]
{} = "{}"
"#,
        pkg_name, package
    );

    std::fs::write(tmp_path.join("wally.toml"), &wally_toml)?;
    
    // Determine which wally executable to run
    let wally_cmd = if has_global_wally {
        "wally".to_string()
    } else {
        local_wally_bin.to_string_lossy().to_string()
    };

    // Run wally install
    let output = tokio::process::Command::new(&wally_cmd)
        .arg("install")
        .current_dir(&tmp_path)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("wally install failed: {}", stderr));
    }

    // Read the Packages directory and build a file tree
    let packages_dir = tmp_path.join("Packages");
    if !packages_dir.exists() {
        return Err(eyre!("Packages directory not created by wally install"));
    }

    fn read_dir_to_json(dir: &std::path::Path) -> color_eyre::Result<Vec<serde_json::Value>> {
        let mut files = Vec::new();

        // Skip known non-source directories
        let skip_dirs = ["docs", "test", "tests", "spec", ".github", "node_modules", "testez-cli"];

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files/dirs
            if name.starts_with('.') {
                continue;
            }

            if path.is_dir() {
                // Skip known non-source directories
                let name_lower = name.to_lowercase();
                if skip_dirs.iter().any(|s| name_lower == *s) {
                    continue;
                }

                let children = read_dir_to_json(&path)?;

                // Check if there's an init.luau or init.lua inside
                let init_source = std::fs::read_to_string(path.join("init.luau"))
                    .or_else(|_| std::fs::read_to_string(path.join("init.lua")))
                    .ok();

                // Filter out init files from children
                let filtered_children: Vec<_> = children.into_iter().filter(|c| {
                    let n = c.get("Name").and_then(|v| v.as_str()).unwrap_or("");
                    n != "init" && n != "init.luau" && n != "init.lua"
                }).collect();

                // Only create a node if there's an init source or Lua children
                if init_source.is_none() && filtered_children.is_empty() {
                    continue; // Skip empty directories with no Lua content
                }

                let class_name = if init_source.is_some() { "ModuleScript" } else { "Folder" };

                let mut node = serde_json::json!({
                    "ClassName": class_name,
                    "Name": name,
                });

                if let Some(source) = init_source {
                    node["Source"] = serde_json::Value::String(source);
                }

                if !filtered_children.is_empty() {
                    node["Children"] = serde_json::Value::Array(filtered_children);
                }

                files.push(node);
            } else if name.ends_with(".luau") || name.ends_with(".lua") {
                // Skip spec/test files
                if name.contains(".spec.") || name.contains(".test.") || name.contains("_spec.") || name.contains("_test.") {
                    continue;
                }

                let source = std::fs::read_to_string(&path)?;
                let script_name = name.trim_end_matches(".luau").trim_end_matches(".lua");

                // Determine script type from filename
                let class_name = if name.ends_with(".server.lua") || name.ends_with(".server.luau") {
                    "Script"
                } else if name.ends_with(".client.lua") || name.ends_with(".client.luau") {
                    "LocalScript"
                } else {
                    "ModuleScript"
                };

                // Clean up script name (remove .server/.client suffix)
                let clean_name = script_name
                    .trim_end_matches(".server")
                    .trim_end_matches(".client");

                files.push(serde_json::json!({
                    "ClassName": class_name,
                    "Name": clean_name,
                    "Source": source,
                }));
            }
            // Skip all non-Lua files (README.md, .toml, .json, etc.)
        }

        Ok(files)
    }

    let file_tree = read_dir_to_json(&packages_dir)?;

    Ok(serde_json::json!({ "files": file_tree }))
}

pub async fn dispatch_function_call(
    state: &PackedState,
    fc: &GeminiFunctionCall,
    chat_id: &str,
) -> color_eyre::Result<String> {
    if fc.name == "propose_plan" || fc.name == "ask_planning_question" {
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        {
            let mut app_state = state.lock().await;
            app_state.plan_waiters.insert(chat_id.to_string(), tx);
        }

        tracing::debug!("Waiting for user response on {} (chat_id: {})", fc.name, chat_id);

        let result = rx.await.unwrap_or_else(|_| "Error: interaction was closed or connection lost.".to_string());
        return Ok(result);
    }

    // Special backend handler for Wally: run CLI, build file tree, then dispatch as ImportFromWally with json payload
    if fc.name == "import_from_wally" {
        let package = fc.args.get("package").and_then(|v| v.as_str())
            .ok_or_else(|| eyre!("import_from_wally missing 'package'"))?.to_string();
        let parent = fc.args.get("parent").and_then(|v| v.as_str())
            .ok_or_else(|| eyre!("import_from_wally missing 'parent'"))?.to_string();

        let wally_result = run_wally_install(&package).await;
        match wally_result {
            Ok(file_tree_json) => {
                // Log the file tree summary for debugging
                if let Some(files) = file_tree_json.get("files").and_then(|f| f.as_array()) {
                    let has_source_count = files.iter().filter(|f| f.get("Source").is_some()).count();
                    tracing::info!("Wally file tree: {} total nodes, {} with Source", files.len(), has_source_count);
                    for f in files.iter().take(3) {
                        let name = f.get("Name").and_then(|n| n.as_str()).unwrap_or("?");
                        let has_src = f.get("Source").is_some();
                        let has_children = f.get("Children").is_some();
                        tracing::info!("  - {} (Source: {}, Children: {})", name, has_src, has_children);
                    }
                }

                // Send the file tree to the plugin as ImportFromWally with the json payload embedded
                let tool_args = ToolArgumentValues::ImportFromWally(ImportFromWally {
                    package: package.clone(),
                    parent: parent.clone(),
                    json: Some(file_tree_json),
                });
                let (command, id) = ToolArguments::new(tool_args);

                let (tx, mut rx) = mpsc::unbounded_channel::<Result<String>>();
                let trigger = {
                    let mut app_state = state.lock().await;
                    app_state.process_queue.push_back(command);
                    app_state.output_map.insert(id, tx);
                    app_state.trigger.clone()
                };
                trigger.send(()).map_err(|e| eyre!("Unable to trigger send: {}", e))?;
                let result = rx.recv().await
                    .ok_or_else(|| eyre!("Couldn't receive response from Roblox Studio"))?;
                {
                    let mut app_state = state.lock().await;
                    app_state.output_map.remove_entry(&id);
                }
                return match result {
                    Ok(response) => Ok(response),
                    Err(err) => Ok(format!("Error: {}", err)),
                };
            }
            Err(e) => {
                return Ok(format!("Error installing Wally package: {}", e));
            }
        }
    }

    let tool_args = convert_function_call_to_tool_args(fc)?;
    let (command, id) = ToolArguments::new(tool_args);
    tracing::debug!("Dispatching function call to Roblox Studio: {:?}", command);

    let (tx, mut rx) = mpsc::unbounded_channel::<Result<String>>();

    let trigger = {
        let mut app_state = state.lock().await;
        app_state.current_tool = Some(fc.name.clone());
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
        app_state.current_tool = None;
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
    
    let system_instruction = build_system_instruction(opts.system_instruction, opts.thinking_level.as_deref());
    
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

#[derive(Serialize)]
pub struct AgentStatusResponse {
    pub is_working: bool,
    pub current_tool: Option<String>,
}

pub async fn agent_status_handler(
    State(state): State<PackedState>,
) -> Result<impl IntoResponse> {
    let app_state = state.lock().await;
    Ok(Json(AgentStatusResponse {
        is_working: !app_state.active_generations.is_empty(),
        current_tool: app_state.current_tool.clone(),
    }))
}

pub async fn plugin_install_handler() -> Result<impl IntoResponse> {
    match crate::install::do_install().await {
        Ok(msg) => Ok(Json(serde_json::json!({
            "success": true,
            "message": msg
        }))),
        Err(e) => Err(eyre!("Failed to install plugin: {}", e).into()),
    }
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
    let is_oauth = api_key.starts_with("Bearer ");

    loop {
        let mut url = if is_oauth {
            format!("https://generativelanguage.googleapis.com/v1beta/models?pageSize=1000")
        } else {
            format!("https://generativelanguage.googleapis.com/v1beta/models?key={}&pageSize=1000", api_key)
        };
        if let Some(ref token) = page_token {
            url.push_str(&format!("&pageToken={}", token));
        }

        let mut req = client.get(&url);
        if is_oauth {
            req = req.header("Authorization", &api_key);
        }
        let res = req.send().await?;
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

use crate::db::{ChatSession, ChatSessionFull, ProjectRow, CreateProjectRequest};

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub project_id: Option<String>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub project_id: Option<String>,
}

pub async fn history_search_handler(
    State(state): State<PackedState>,
    axum::extract::Query(query): axum::extract::Query<SearchQuery>,
) -> Result<Json<Vec<ChatSession>>, axum::http::StatusCode> {
    let state = state.lock().await;
    match state.db.search_chats(&query.q, query.project_id.as_deref()) {
        Ok(chats) => Ok(Json(chats)),
        Err(e) => {
            tracing::error!("Failed to search chats: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn history_list_handler(
    State(state): State<PackedState>,
    axum::extract::Query(query): axum::extract::Query<ListQuery>,
) -> Result<Json<Vec<ChatSession>>, axum::http::StatusCode> {
    let state = state.lock().await;
    let result = if let Some(pid) = query.project_id.as_deref() {
        state.db.list_chats_by_project(pid)
    } else {
        state.db.list_chats()
    };
    match result {
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

// ── Projects ──────────────────────────────────────────────────────────────────

pub async fn projects_list_handler(
    State(state): State<PackedState>,
) -> Result<Json<Vec<ProjectRow>>, axum::http::StatusCode> {
    let state = state.lock().await;
    match state.db.list_projects() {
        Ok(projects) => Ok(Json(projects)),
        Err(e) => {
            tracing::error!("Failed to list projects: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn project_create_handler(
    State(state): State<PackedState>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let state = state.lock().await;
    match state.db.create_project(&id, &req.name, req.details.as_deref(), now) {
        Ok(_) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => {
            tracing::error!("Failed to create project: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn project_delete_handler(
    State(state): State<PackedState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::http::StatusCode, axum::http::StatusCode> {
    let state = state.lock().await;
    match state.db.delete_project(&id) {
        Ok(_) => Ok(axum::http::StatusCode::OK),
        Err(e) => {
            tracing::error!("Failed to delete project: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn project_memory_handler(
    State(state): State<PackedState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Vec<String>>, axum::http::StatusCode> {
    let state = state.lock().await;
    match state.db.get_project_memory(&id) {
        Ok(memories) => Ok(Json(memories)),
        Err(e) => {
            tracing::error!("Failed to get project memory: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(serde::Deserialize)]
pub struct UpdateProjectContextRequest {
    pub context: String,
}

pub async fn project_update_context_handler(
    State(state): State<PackedState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(req): Json<UpdateProjectContextRequest>,
) -> Result<axum::http::StatusCode, axum::http::StatusCode> {
    let state = state.lock().await;
    match state.db.update_project_context(&id, &req.context) {
        Ok(_) => Ok(axum::http::StatusCode::OK),
        Err(e) => {
            tracing::error!("Failed to update project context: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
