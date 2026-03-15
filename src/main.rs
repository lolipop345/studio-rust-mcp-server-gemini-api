use axum::routing::{get, post};
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderValue, Method};
use clap::Parser;
use color_eyre::eyre::Result;
use rbx_studio_server::*;
use std::io;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_subscriber::{self, EnvFilter};
use std::path::PathBuf;

mod error;
mod install;
pub mod db;
mod rbx_studio_server;

#[derive(Parser)]
#[command(version, about = "Gemini-powered agent for Roblox Studio")]
struct Args {
    #[arg(long)]
    install: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(io::stderr)
        .with_target(false)
        .with_thread_ids(true)
        .init();

    let args = Args::parse();
    if args.install {
        return install::install().await;
    }

    let api_key = std::env::var("GEMINI_API_KEY").ok();
    let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let db_path = working_dir.join("gemini_studio.db");
    let db_client = db::DbClient::new(&db_path).expect("Failed to initialize SQLite database");

    let server_state = Arc::new(Mutex::new(AppState::new(db_client)));

    if let Some(key) = api_key {
        let mut state = server_state.lock().await;
        state.api_key = Some(key);
    }

    let (close_tx, close_rx) = tokio::sync::oneshot::channel();

    let listener =
        tokio::net::TcpListener::bind((Ipv4Addr::new(127, 0, 0, 1), STUDIO_PLUGIN_PORT)).await;

    let server_state_clone = Arc::clone(&server_state);
    let server_handle = if let Ok(listener) = listener {
        let app = axum::Router::new()
            .route("/request", get(request_handler))
            .route("/response", post(response_handler))
            .route("/proxy", post(proxy_handler))
            .route("/chat/send", post(chat_send_handler).layer(DefaultBodyLimit::max(20 * 1024 * 1024)))
            .route("/chat/stop", post(chat_stop_handler))
            .route("/chat/plan_response", post(chat_plan_response_handler))
            .route("/chat/events/{id}", get(chat_events_handler))
            .route("/chat/api-key", post(api_key_handler))
            .route("/chat/status", get(status_handler))
            .route("/agent_status", get(agent_status_handler))
            .route("/plugin/install", post(plugin_install_handler))
            .route("/chat/models", get(models_handler))
            .route("/history/list", get(history_list_handler))
            .route("/history/load/{id}", get(history_load_handler))
            .route("/history/save", post(history_save_handler))
            .route("/history/delete/{id}", post(history_delete_handler))
            .route("/history/clear_all", post(history_clear_all_handler))
            .route("/history/update_title", post(history_update_title_handler))
            .route("/history/toggle_pin", post(history_toggle_pin_handler))
            .route("/history/search", get(history_search_handler))
            .route("/projects", get(projects_list_handler))
            .route("/projects", post(project_create_handler))
            .route("/projects/{id}", axum::routing::delete(project_delete_handler))
            .route("/projects/{id}/context", post(project_update_context_handler))
            .route("/projects/{id}/memory", get(project_memory_handler))
            .with_state(server_state_clone)
            // Security: restrict CORS to localhost only
            .layer(
                tower_http::cors::CorsLayer::new()
                    .allow_origin([
                        "http://127.0.0.1".parse::<HeaderValue>().unwrap(),
                        "http://localhost".parse::<HeaderValue>().unwrap(),
                        "file://".parse::<HeaderValue>().unwrap(),
                    ])
                    .allow_methods([Method::GET, Method::POST, Method::DELETE])
                    .allow_headers(tower_http::cors::Any)
            );
            
        tracing::info!("HTTP server listening on port {STUDIO_PLUGIN_PORT}");
        
        // Ensure npm is installed and dependencies are ready, then launch Electron
        tokio::spawn(async move {
            let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let electron_dir = working_dir.join("electron");
            
            if electron_dir.exists() {
                tracing::info!("Starting Electron UI...");
                
                // npm install if node_modules is missing
                if !electron_dir.join("node_modules").exists() {
                    tracing::info!("Running npm install in electron directory...");
                    let _ = tokio::process::Command::new("npm")
                        .arg("install")
                        .current_dir(&electron_dir)
                        .output()
                        .await;
                }
                
                // npm start
                let mut child = tokio::process::Command::new("npm")
                    .arg("start")
                    .current_dir(&electron_dir)
                    .spawn()
                    .expect("Failed to start Electron app");
                    
                let _ = child.wait().await;
                tracing::info!("Electron app closed.");
                // Terminate backend if Electron closes
                std::process::exit(0);
            } else {
                tracing::error!("Electron directory not found at {:?}", electron_dir);
            }
        });

        tokio::spawn(async {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    _ = close_rx.await;
                })
                .await
                .unwrap();
        })
    } else {
        tracing::info!("Port busy, using proxy mode (disabling UI launch)");
        eprintln!("⚠️  Port {} is already in use by another instance.", STUDIO_PLUGIN_PORT);
        eprintln!("⚠️  Running in proxy mode and disabling Electron UI launch.");
        eprintln!("⚠️  To launch the UI, please close the existing server first.");
        tokio::spawn(async move {
            dud_proxy_loop(server_state_clone, close_rx).await;
        })
    };

    // Wait for the HTTP server loop indefinitely
    let _ = server_handle.await;

    close_tx.send(()).ok();
    tracing::info!("Bye!");

    Ok(())
}
