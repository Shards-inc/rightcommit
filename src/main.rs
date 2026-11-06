mod a2a;
mod agent_activity;
mod agent_auth;
mod agents;
mod artifacts;
mod file_watch;
mod api;
mod auth;
mod users;
mod compaction;
mod connectors;
mod github_api;
mod servicenow;
mod servicenow_oauth;
mod itsm_batch;
mod servicenow_config;
mod servicenow_request;
mod pi_check;
mod pi_local;
mod pi_providers;
mod startup_env;
mod continuation;
mod deletion;
mod distribution;
mod engine;
mod schedules;
mod finops;
mod claude_time;
mod codex_cost;
mod manager;
mod mcp_auth;
mod mcp_bridge;
mod mcp_client;
mod mcp_workspace;
mod model;
mod parallelism;
mod presets;
mod templates;
mod rag;
mod revision;
mod source_access;
mod storage;
mod store;
mod topics;
mod subworkflows;
mod supervision;
#[cfg(test)]
mod tests;
mod twg;

use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct App {
    pub store: store::Store,
    pub(crate) artifact_tracker: Arc<artifacts::Tracker>,
    pub mcp: Arc<mcp_client::Hub>,
    pub rag: Arc<rag::Hub>,
    pub data: PathBuf,
    pub assets: PathBuf,
    pub sources: PathBuf,
    pub busy: Arc<Mutex<HashSet<String>>>,
    pub workers: Arc<Mutex<std::collections::HashMap<String, engine::RunControl>>>,
    pub lifecycle: Arc<tokio::sync::RwLock<()>>,
    pub slots: Arc<parallelism::Scheduler>,
    pub bus: broadcast::Sender<(i64, model::Event)>,
    pub isolate: bool,
    pub secure_cookie: bool,
}

/// Docker mounts the project folder at /sources; a native installation keeps it next to ./data.
fn sources_dir() -> anyhow::Result<PathBuf> {
    if let Ok(dir) = std::env::var("NODO_SOURCES") {
        return Ok(PathBuf::from(dir));
    }
    let mounted = PathBuf::from("/sources");
    if mounted.is_dir() {
        return Ok(mounted);
    }
    std::fs::create_dir_all("sources")?;
    Ok(std::fs::canonicalize("sources")?)
}

fn main() -> anyhow::Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => {
            println!("rightcommit {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("--help" | "-h") => {
            println!("rightcommit [setup | --version | --help]\n\nAvvio: http://localhost:8080\nsetup: installa agenti ed estensioni (richiede Node.js 24 o almeno 22.19, con npm).\nDati: ./data oppure NODO_DATA. Progetti: ./sources oppure NODO_SOURCES. Risorse esterne: NODO_ASSETS.\nAmbiente: .env nella cartella di avvio; le variabili già impostate hanno precedenza.");
            return Ok(());
        }
        Some("setup" | "mcp-bridge" | "mcp-workspace") | None => {}
        Some(arg) => anyhow::bail!("Argomento sconosciuto: {arg}; usa --help"),
    }
    if matches!(std::env::args().nth(1).as_deref(),None|Some("setup")) {
        // SAFETY: no runtime or application threads exist yet. Utility MCP processes must
        // not load a .env from an agent's working directory.
        unsafe { startup_env::load()?; }
    }
    tokio::runtime::Builder::new_multi_thread().enable_all().build()?.block_on(run())
}

async fn run() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("mcp-bridge") {
        return mcp_bridge::run().await;
    }
    if std::env::args().nth(1).as_deref() == Some("mcp-workspace") {
        return mcp_workspace::run().await;
    }
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "rightcommit=info".into()))
        .init();
    let data = PathBuf::from(std::env::var("NODO_DATA").unwrap_or_else(|_| "data".into()));
    std::fs::create_dir_all(&data)?;
    let data = std::fs::canonicalize(data)?;
    let assets = distribution::assets(&data)?;
    if std::env::args().nth(1).as_deref() == Some("setup") {
        return distribution::setup(&data, &assets);
    }
    let store = store::Store::open(&data)?;
    store.interrupt_running()?;
    compaction::recover(&store)?;
    let (bus, _) = broadcast::channel(512);
    let app = App {
        store,
        artifact_tracker: Arc::default(),
        mcp: Arc::default(),
        rag: Arc::default(),
        data,
        assets,
        sources: sources_dir()?,
        busy: Arc::default(),
        workers: Arc::default(),
        lifecycle: Arc::default(),
        slots: Arc::default(),
        bus,
        isolate: std::env::var("NODO_ISOLATE_USERS").is_ok_and(|s| s == "1"),
        secure_cookie: std::env::var("NODO_SECURE_COOKIE").is_ok_and(|s| s == "1"),
    };
    artifacts::recover_legacy_runs(&app)?;
    let mcp = app.mcp.clone();
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            timer.tick().await;
            mcp.sweep().await;
        }
    });
    if app.isolate && unsafe { libc::geteuid() } != 0 {
        anyhow::bail!("NODO_ISOLATE_USERS richiede root per avviare child con UID distinti");
    }
    let addr = std::env::var("NODO_LISTEN").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(%addr, isolated=app.isolate, "rightcommit pronto");
    schedules::start_service(app.clone())?;
    axum::serve(listener, api::router(app))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
