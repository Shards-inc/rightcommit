use crate::{
    App, agents,
    auth::{ApiError, ApiResult},
    model::*,
};
use anyhow::{Context, Result, ensure};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use rmcp::{
    RoleClient, ServiceExt,
    model::*,
    service::RunningService,
    transport::{
        StreamableHttpClientTransport, TokioChildProcess,
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::OnceCell;

pub struct Session {
    last_used: std::sync::atomic::AtomicI64,
    _process: Option<agents::ProcessGroup>,
    pub service: RunningService<RoleClient, ClientConfig>,
    pub timeout: u64,
}
#[derive(Clone)]
pub struct Bound {
    pub node: String,
    pub session: Arc<Session>,
    pub allowed: Vec<String>,
}
#[derive(Clone)]
pub struct Grant {
    pub uid: i64,
    pub run: String,
    pub task: String,
    pub servers: Vec<Bound>,
    pub rag: Vec<crate::model::Node>,
    pub expires: i64,
}
#[derive(Default)]
pub struct Hub {
    sessions: tokio::sync::Mutex<HashMap<String, Arc<OnceCell<Arc<Session>>>>>,
    pub grants: Mutex<HashMap<String, Grant>>,
    pub logins: Mutex<HashMap<String, crate::mcp_auth::Login>>,
    pub auth_lock: tokio::sync::Mutex<()>,
}
impl Hub {
    pub async fn sweep(&self) {
        self.sessions.lock().await.retain(|_, cell| {
            cell.get().is_none_or(|s| {
                s.last_used.load(std::sync::atomic::Ordering::Relaxed) > now() - 900
                    || Arc::strong_count(s) > 1
            })
        });
        self.logins.lock().unwrap().retain(|_, l| l.expires > now());
    }
    pub async fn clear_user(&self, uid: i64) {
        self.grants.lock().unwrap().retain(|_, g| g.uid != uid);
        self.sessions
            .lock()
            .await
            .retain(|k, _| !k.starts_with(&format!("{uid}:")));
    }
    pub async fn release_run(&self, uid: i64, rid: &str) {
        self.sessions
            .lock()
            .await
            .retain(|k, _| !k.starts_with(&format!("{uid}:{rid}:")));
    }
}
pub fn validate(c: &McpConfig) -> Result<()> {
    ensure!(
        c.name.len() < 180 && c.timeout_secs >= 1 && c.timeout_secs <= 300,
        "MCP: nome o timeout non valido (1–300 s)"
    );
    ensure!(
        [
            "none",
            "bearer",
            "api_key",
            "basic",
            "oauth_pkce",
            "oauth_client_credentials"
        ]
        .contains(&c.auth.mode.as_str()),
        "Autenticazione MCP non valida"
    );
    ensure!(
        ["none", "basic", "post"].contains(&c.auth.token_auth.as_str()),
        "Autenticazione token endpoint non valida"
    );
    if c.transport == "http" {
        crate::mcp_auth::url(&c.url)?;
    } else {
        ensure!(
            c.transport == "stdio"
                && !c.command.is_empty()
                && c.command.len() < 4096
                && c.args.len() <= 100,
            "Comando MCP stdio non valido"
        );
        ensure!(
            c.auth.mode == "none" && c.headers.is_empty(),
            "Per stdio usa le variabili d'ambiente per le credenziali"
        );
    }
    ensure!(
        c.args.iter().all(|a| a.len() < 8192) && c.env.len() <= 50 && c.headers.len() <= 30,
        "Configurazione MCP troppo grande"
    );
    for (k, v) in &c.headers {
        reqwest::header::HeaderName::from_bytes(k.as_bytes())?;
        reqwest::header::HeaderValue::from_str(v)?;
        ensure!(
            ![
                "host",
                "content-length",
                "connection",
                "mcp-session-id",
                "mcp-protocol-version"
            ]
            .contains(&k.to_lowercase().as_str()),
            "Header MCP riservato"
        );
    }
    if c.auth.mode == "api_key" {
        reqwest::header::HeaderName::from_bytes(c.auth.header.as_bytes())?;
    }
    ensure!(
        c.env.iter().all(|(k, v)| !k.is_empty()
            && k.len() < 128
            && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !v.contains('\0')
            && v.len() <= 16384
            && !k.starts_with("LD_")
            && !k.starts_with("DYLD_")
            && ![
                "GCONV_PATH",
                "GLIBC_TUNABLES",
                "LOCPATH",
                "NLSPATH",
                "LIBPATH",
                "SHLIB_PATH"
            ]
            .contains(&k.as_str())),
        "Variabili MCP non valide"
    );
    if c.auth.mode.starts_with("oauth_") {
        ensure!(!c.auth.client_id.is_empty(), "Client ID OAuth obbligatorio");
        if !c.auth.issuer.is_empty() {
            crate::mcp_auth::url(&c.auth.issuer)?;
        }
        if !c.auth.token_url.is_empty() {
            crate::mcp_auth::url(&c.auth.token_url)?;
        }
        ensure!(
            !c.auth.issuer.is_empty() || !c.auth.token_url.is_empty(),
            "Configura issuer o token endpoint OAuth"
        );
    }
    Ok(())
}
pub fn public_config(c: &McpConfig) -> Value {
    let mut v = serde_json::to_value(c).unwrap();
    for k in [
        "token",
        "password",
        "client_secret",
        "access_token",
        "refresh_token",
    ] {
        v["auth"][k] = json!("");
    }
    v["auth"]["connected"] = json!(!c.auth.access_token.is_empty());
    v["auth"]["configured"] = json!(
        !c.auth.token.is_empty() || !c.auth.password.is_empty() || !c.auth.client_secret.is_empty()
    );
    for k in ["env", "headers"] {
        v[k] = json!(
            c_field(c, k)
                .keys()
                .map(|k| (k.clone(), String::new()))
                .collect::<BTreeMap<_, _>>()
        );
    }
    v
}
fn c_field<'a>(c: &'a McpConfig, k: &str) -> &'a BTreeMap<String, String> {
    if k == "env" { &c.env } else { &c.headers }
}
pub fn merge_secret(new: &mut String, old: &str) {
    if new.is_empty() {
        *new = old.into()
    } else if new == "__CLEAR__" {
        new.clear()
    }
}
pub fn merge(c: &mut McpConfig, old: &McpConfig) {
    merge_secret(&mut c.auth.token, &old.auth.token);
    merge_secret(&mut c.auth.password, &old.auth.password);
    merge_secret(&mut c.auth.client_secret, &old.auth.client_secret);
    for (k, v) in &mut c.env {
        if let Some(old) = old.env.get(k) {
            merge_secret(v, old)
        }
    }
    for (k, v) in &mut c.headers {
        if let Some(old) = old.headers.get(k) {
            merge_secret(v, old)
        }
    }
    // Browser never gets or supplies OAuth tokens. Keep them only for the same authorization settings.
    c.auth.access_token.clear();
    c.auth.refresh_token.clear();
    c.auth.expires = 0;
    if crate::mcp_auth::fingerprint(c) == crate::mcp_auth::fingerprint(old) {
        c.auth.access_token = old.auth.access_token.clone();
        c.auth.refresh_token = old.auth.refresh_token.clone();
        c.auth.expires = old.auth.expires;
    }
}
async fn connect(
    app: &App,
    uid: i64,
    cfg: &McpConfig,
    headers: BTreeMap<String, String>,
    cwd: &std::path::Path,
) -> Result<Arc<Session>> {
    validate(cfg)?;
    let timeout = Duration::from_secs(cfg.timeout_secs);
    let client = ClientConfig::default();
    let mut process = None;
    let service = if cfg.transport == "stdio" {
        let settings = app.store.settings(uid)?;
        let mut command = agents::command(app, uid, &cfg.command, cwd, &settings)?;
        command
            .args(&cfg.args)
            .envs(&cfg.env)
            .stderr(std::process::Stdio::null());
        let transport = TokioChildProcess::new(command)?;
        process = transport.id().map(agents::ProcessGroup);
        tokio::time::timeout(timeout, client.serve(transport))
            .await
            .context("Timeout connessione MCP")?
            .context("Handshake MCP stdio fallito")?
    } else {
        let mut config = StreamableHttpClientTransportConfig::with_uri(cfg.url.clone());
        config.custom_headers = headers
            .into_iter()
            .map(|(k, v)| {
                Ok((
                    reqwest::header::HeaderName::from_bytes(k.as_bytes())?,
                    reqwest::header::HeaderValue::from_str(&v)?,
                ))
            })
            .collect::<Result<_>>()?;
        config.reinit_on_expired_session = false; // Never replay a potentially mutating call automatically.
        config.max_sse_event_size = 4 * 1024 * 1024;
        let transport = StreamableHttpClientTransport::with_client(
            reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(10))
                .build()?,
            config,
        );
        tokio::time::timeout(timeout, client.serve(transport))
            .await
            .context("Timeout connessione MCP")?
            .context("Handshake MCP HTTP fallito; verifica endpoint e autenticazione")?
    };
    service
        .set_response_cache_config(rmcp::ClientCacheConfig::disabled())
        .await;
    Ok(Arc::new(Session {
        last_used: std::sync::atomic::AtomicI64::new(now()),
        _process: process,
        service,
        timeout: cfg.timeout_secs,
    }))
}
async fn cached(app: &App, uid: i64, r: &Run, node: &Node) -> Result<Arc<Session>> {
    app.mcp.sweep().await;
    let (cfg, headers) = crate::mcp_auth::headers(app, uid, &node.mcp_id).await?;
    let key = format!(
        "{uid}:{}:{}:{}",
        r.id,
        node.mcp_id,
        crate::store::hash(serde_json::to_vec(&cfg)?)
    );
    let cell = {
        let mut entries = app.mcp.sessions.lock().await;
        ensure!(
            entries.len() < 128 || entries.contains_key(&key),
            "Limite connessioni MCP raggiunto"
        );
        entries.entry(key).or_default().clone()
    };
    Ok(cell
        .get_or_try_init(|| connect(app, uid, &cfg, headers, std::path::Path::new(&r.workspace)))
        .await?
        .clone())
}
fn alias(node: &str, name: &str) -> String {
    let clean = |s: &str, n| {
        s.chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .take(n)
            .collect::<String>()
    };
    format!(
        "nodo_{}_{}_{}",
        clean(node, 10),
        clean(name, 28),
        &crate::store::hash(format!("{node}:{name}"))[..8]
    )
}
fn metadata_tools(s: &Session) -> Vec<Tool> {
    let Some(info) = s.service.peer_info() else {
        return vec![];
    };
    let mut tools = vec![];
    let mut add = |name: &str, description: &str, schema: Value| {
        tools.push(Tool::new(
            name.to_string(),
            description.to_string(),
            schema.as_object().unwrap().clone(),
        ))
    };
    if info.capabilities.resources.is_some() {
        add(
            "@resources/list",
            "List MCP resources and URI templates",
            json!({"type":"object","properties":{}}),
        );
        add(
            "@resources/read",
            "Read an MCP resource by its URI",
            json!({"type":"object","properties":{"uri":{"type":"string"}},"required":["uri"]}),
        );
    }
    if info.capabilities.prompts.is_some() {
        add(
            "@prompts/list",
            "List MCP prompt templates",
            json!({"type":"object","properties":{}}),
        );
        add(
            "@prompts/get",
            "Get an MCP prompt template with arguments",
            json!({"type":"object","properties":{"name":{"type":"string"},"arguments":{"type":"object","additionalProperties":{"type":"string"}}},"required":["name"]}),
        );
    }
    tools
}
async fn listed(s: &Session) -> Result<Vec<Tool>> {
    s.last_used
        .store(now(), std::sync::atomic::Ordering::Relaxed);
    let mut tools = if s
        .service
        .peer_info()
        .is_some_and(|i| i.capabilities.tools.is_some())
    {
        s.service.list_all_tools().await?
    } else {
        vec![]
    };
    tools.extend(metadata_tools(s));
    Ok(tools)
}
async fn metadata(s: &Session, name: &str, args: Value) -> Result<Value> {
    let result = match name {
        "@resources/list" => {
            json!({"resources":s.service.list_all_resources().await?,"templates":s.service.list_all_resource_templates().await?})
        }
        "@resources/read" => serde_json::to_value(
            s.service
                .read_resource(serde_json::from_value(args)?)
                .await?,
        )?,
        "@prompts/list" => json!({"prompts":s.service.list_all_prompts().await?}),
        "@prompts/get" => {
            serde_json::to_value(s.service.get_prompt(serde_json::from_value(args)?).await?)?
        }
        _ => anyhow::bail!("Metodo metadati MCP non valido"),
    };
    Ok(json!({"content":[{"type":"text","text":serde_json::to_string(&result)?}],"isError":false}))
}
async fn inventory(g: &Grant) -> Result<Vec<Tool>> {
    let mut tools = vec![];
    for server in &g.servers {
        let listed = tokio::time::timeout(
            Duration::from_secs(server.session.timeout),
            listed(&server.session),
        )
        .await
        .context("Timeout discovery MCP")??;
        ensure!(listed.len() <= 500, "Troppi tool MCP");
        for mut tool in listed {
            if server.allowed.is_empty() || server.allowed.iter().any(|n| n == tool.name.as_ref()) {
                tool.name = alias(&server.node, &tool.name).into();
                tools.push(tool)
            }
        }
    }
    for node in &g.rag {
        tools.extend(crate::rag::tools(node)?);
    }
    Ok(tools)
}
pub struct Prepared {
    hub: Arc<Hub>,
    token: String,
    pub tools: Vec<Tool>,
    pub dir: PathBuf,
    pub endpoint: String,
}
impl Drop for Prepared {
    fn drop(&mut self) {
        self.hub
            .grants
            .lock()
            .unwrap()
            .remove(&crate::store::hash(&self.token));
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
impl Prepared {
    pub fn token(&self) -> &str {
        &self.token
    }
}
pub async fn prepare(app: &App, uid: i64, r: &Run) -> Result<Option<Prepared>> {
    let task = r.workflow.node(&r.current)?;
    let agent = &task.agent;
    let mut servers = vec![];
    let mut rag = vec![];
    for edge in r
        .workflow
        .edges
        .iter()
        .filter(|e| e.kind == "context" && e.to == *agent)
    {
        let node = r.workflow.node(&edge.from)?;
        if node.kind == "RAG" {
            let i = crate::rag::get(app, uid, &node.rag_index)?;
            ensure!(
                i.directory && i.generation.is_some(),
                "Indice RAG non disponibile"
            );
            rag.push(node.clone());
        }
        if node.kind == "MCP" {
            servers.push(Bound {
                node: node.id.clone(),
                session: cached(app, uid, r, node).await?,
                allowed: node.mcp_tools.clone(),
            });
        }
    }
    if servers.is_empty() && rag.is_empty() {
        return Ok(None);
    }
    let grant = Grant {
        uid,
        run: r.id.clone(),
        task: r.current.clone(),
        servers,
        rag,
        expires: now() + app.store.settings(uid)?.task_timeout_secs as i64 + 60,
    };
    let tools = inventory(&grant).await?;
    let token = format!("{}{}", id(), id());
    let dir = agents::user_home(app, uid)?
        .join("tmp")
        .join(format!("mcp-{}", id()));
    std::fs::create_dir(&dir)?;
    crate::store::private(&dir, true)?;
    agents::own(app, uid, &dir)?;
    let endpoint = format!(
        "http://127.0.0.1:{}/api/mcp/agent",
        std::env::var("NODO_LISTEN")
            .ok()
            .and_then(|s| s.rsplit(':').next()?.parse::<u16>().ok())
            .unwrap_or(8080)
    );
    app.mcp
        .grants
        .lock()
        .unwrap()
        .insert(crate::store::hash(&token), grant);
    Ok(Some(Prepared {
        hub: app.mcp.clone(),
        token,
        tools,
        dir,
        endpoint,
    }))
}
#[derive(Deserialize, Serialize)]
pub struct Request {
    pub method: String,
    #[serde(default)]
    pub params: Value,
}
fn grant(app: &App, h: &HeaderMap) -> Result<Grant> {
    let token = h
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .context("Credenziale MCP mancante")?;
    ensure!(token.len() < 200, "Credenziale MCP non valida");
    let g = app
        .mcp
        .grants
        .lock()
        .unwrap()
        .get(&crate::store::hash(token))
        .cloned()
        .context("Credenziale MCP revocata")?;
    crate::auth::ensure_active(app, g.uid)?;
    ensure!(
        g.expires > now() && app.busy.lock().unwrap().contains(&g.run),
        "Turno agente MCP non attivo"
    );
    let r = app.store.run(g.uid, &g.run)?;
    ensure!(r.current == g.task, "Il task MCP è cambiato");
    Ok(g)
}
pub async fn agent(
    State(app): State<App>,
    h: HeaderMap,
    Json(req): Json<Request>,
) -> ApiResult<Json<Value>> {
    let g = grant(&app, &h).map_err(|_| {
        ApiError(
            StatusCode::UNAUTHORIZED,
            "Accesso MCP non valido o scaduto".into(),
        )
    })?;
    let result=dispatch(&app,&g,req).await.map_err(|_|crate::auth::bad("Richiesta MCP fallita. Verifica connessione, tool consentiti e timeout; nessun retry automatico."))?;
    Ok(Json(result))
}
async fn dispatch(app: &App, g: &Grant, req: Request) -> Result<Value> {
    match req.method.as_str() {
        "tools/list" => Ok(json!({"tools":inventory(g).await?})),
        "tools/call" => {
            let name = req.params["name"].as_str().context("Tool mancante")?;
            for node in &g.rag {
                if let Some(result) = crate::rag::call(
                    app,
                    g.uid,
                    node,
                    name,
                    req.params.get("arguments").cloned().unwrap_or(json!({})),
                )
                .await?
                {
                    return Ok(result);
                }
            }
            for server in &g.servers {
                let listed = tokio::time::timeout(
                    Duration::from_secs(server.session.timeout),
                    listed(&server.session),
                )
                .await??;
                if let Some(tool) = listed.into_iter().find(|t| {
                    alias(&server.node, &t.name) == name
                        && (server.allowed.is_empty()
                            || server.allowed.iter().any(|n| n == t.name.as_ref()))
                }) {
                    if tool.name.starts_with('@') {
                        let args = req.params.get("arguments").cloned().unwrap_or(json!({}));
                        return tokio::time::timeout(
                            Duration::from_secs(server.session.timeout),
                            metadata(&server.session, &tool.name, args),
                        )
                        .await
                        .context("Timeout metadati MCP")?;
                    }
                    let mut params: CallToolRequestParams =
                        serde_json::from_value(req.params.clone())?;
                    params.name = tool.name.clone();
                    let start = std::time::Instant::now();
                    agents::emit(
                        app,
                        g.uid,
                        &g.run,
                        "mcp_tool",
                        &g.task,
                        &json!({"server_node":server.node,"tool":tool.name,"status":"started"})
                            .to_string(),
                    );
                    let result = tokio::time::timeout(
                        Duration::from_secs(server.session.timeout),
                        server.session.service.call_tool(params),
                    )
                    .await;
                    let success = result.as_ref().is_ok_and(|r| {
                        r.as_ref().is_ok_and(|r| {
                            serde_json::to_value(r)
                                .ok()
                                .is_some_and(|v| v["isError"] != true)
                        })
                    });
                    agents::emit(app,g.uid,&g.run,"mcp_tool",&g.task,&json!({"server_node":server.node,"tool":tool.name,"status":if success{"completed"}else{"failed"},"elapsed_ms":start.elapsed().as_millis()}).to_string());
                    return Ok(serde_json::to_value(result.context(
                        "Timeout MCP: verifica gli effetti prima del retry",
                    )??)?);
                }
            }
            anyhow::bail!("Tool MCP non associato all'agente")
        }
        _ => anyhow::bail!("Metodo MCP non supportato"),
    }
}
pub async fn probe(
    State(app): State<App>,
    u: User,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let (cfg, h) = crate::mcp_auth::headers(&app, u.id, &id).await?;
    let dir = agents::user_home(&app, u.id)?;
    let session = connect(&app, u.id, &cfg, h, &dir).await?;
    let g = Grant {
        rag: vec![],
        uid: u.id,
        run: String::new(),
        task: String::new(),
        servers: vec![Bound {
            node: id,
            session: session.clone(),
            allowed: vec![],
        }],
        expires: 0,
    };
    let tools = tokio::time::timeout(Duration::from_secs(session.timeout), listed(&session))
        .await
        .context("Timeout discovery MCP")?
        .context("Discovery tool MCP fallita")?;
    let info = serde_json::to_value(session.service.peer_info()).map_err(anyhow::Error::from)?;
    let _ = g;
    Ok(Json(json!({"ok":true,"server":info,"tools":tools})))
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
pub(crate) mod tests;
