use rusqlite::OptionalExtension;
use crate::{
    App, agents,
    auth::{self, ApiResult, bad},
    engine,
    model::*,
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{
        Html, IntoResponse, Sse,
        sse::{Event as SseEvent, KeepAlive},
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, convert::Infallible};

pub fn router(app: App) -> Router {
    Router::new()
        .route("/github-api.js", axum::routing::get(|| async { ([(header::CONTENT_TYPE,"text/javascript")], include_str!("../web/github-api.js")) }))
        .route("/schedules.js", axum::routing::get(|| async { ([(header::CONTENT_TYPE,"text/javascript")], include_str!("../web/schedules.js")) }))
        .route("/api/schedules", axum::routing::get(crate::schedules::list).post(crate::schedules::create))
        .route("/api/schedules/preview", axum::routing::post(crate::schedules::preview))
        .route("/api/schedules/{id}", axum::routing::put(crate::schedules::update).delete(crate::schedules::delete))
        .route("/api/schedules/{id}/history", axum::routing::get(crate::schedules::history))
        .route(
            "/api/a2a/connections",
            axum::routing::get(crate::a2a::connections).post(crate::a2a::save_connection),
        )
        .route(
            "/api/a2a/connections/{id}/probe",
            axum::routing::post(crate::a2a::probe),
        )
        .route(
            "/api/a2a/workflows/{id}",
            axum::routing::get(crate::a2a::publication).post(crate::a2a::publish),
        )
        .route(
            "/a2a/{uid}/{id}/.well-known/agent-card.json",
            axum::routing::get(crate::a2a::card),
        )
        .route("/a2a/{uid}/{id}", axum::routing::post(crate::a2a::rpc))
        .route("/api/workflow-topics", axum::routing::get(crate::topics::read).post(crate::topics::edit))
        .route("/", axum::routing::get(index))
        .route("/app.css", axum::routing::get(css))
        .route("/app.js", axum::routing::get(js))
        .route(
            "/templates.js",
            axum::routing::get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/templates.js"),
                )
            }),
        )
        .route(
            "/api/template-catalog",
            axum::routing::get(crate::templates::read),
        )
        .route(
            "/api/admin/templates",
            axum::routing::get(crate::templates::read_admin).put(crate::templates::save)
                .layer(axum::extract::DefaultBodyLimit::max(16 * 1024 * 1024)),
        )
        .route(
            "/api/admin/templates/validate",
            axum::routing::post(crate::templates::validate_import)
                .layer(axum::extract::DefaultBodyLimit::max(16 * 1024 * 1024)),
        )
        .route(
            "/api/admin/templates/history",
            axum::routing::get(crate::templates::history),
        )
        .route(
            "/api/admin/templates/history/{revision}",
            axum::routing::get(crate::templates::historical),
        )
        .route(
            "/api/admin/templates/assist",
            axum::routing::post(crate::templates::assist),
        )
        .route("/controls.js", axum::routing::get(|| async {
            ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../web/controls.js"))
        }))
        .route(
            "/a2a.js",
            axum::routing::get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/a2a.js"),
                )
            }),
        )
        .route(
            "/api/manager/chat/reset",
            axum::routing::post(crate::manager::reset),
        )
        .route(
            "/manager.js",
            axum::routing::get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/manager.js"),
                )
            }),
        )
        .route(
            "/api/manager/chat",
            axum::routing::get(crate::manager::history).post(crate::manager::chat),
        )
        .route("/finops.js", axum::routing::get(finops_js))
        .route("/i18n.js", axum::routing::get(i18n))
        .route("/icons.svg", axum::routing::get(icons))
        .route(
            "/healthz",
            axum::routing::get(|| async {
                Json(json!({"status":"ok","version":env!("CARGO_PKG_VERSION")}))
            }),
        )
        .route("/api/auth/status", axum::routing::get(auth_status))
        .route("/api/auth/register", axum::routing::post(register))
        .route("/api/auth/login", axum::routing::post(login))
        .route("/api/auth/logout", axum::routing::post(logout))
        .route("/api/auth/me", axum::routing::get(me))
        .route("/api/auth/password", axum::routing::post(change_password))
        .route("/api/users", axum::routing::get(crate::users::list).post(add_user))
        .route("/api/users/{id}/status", axum::routing::post(crate::users::set_status))
        .route("/api/users/{id}", axum::routing::delete(crate::users::remove))
        .route(
            "/api/runs/{id}/compact",
            axum::routing::post(crate::compaction::start),
        )
        .route(
            "/api/settings",
            axum::routing::get(settings).put(save_settings).patch(patch_settings),
        )
        .route(
            "/api/rag",
            axum::routing::get(crate::rag::list).post(crate::rag::start),
        )
        .route(
            "/api/rag/{id}",
            axum::routing::get(crate::rag::detail).delete(crate::rag::delete),
        )
        .route(
            "/api/rag/{id}/cancel",
            axum::routing::post(crate::rag::cancel),
        )
        .route(
            "/api/rag/{id}/search",
            axum::routing::post(crate::rag::search),
        )
        .route(
            "/rag.js",
            axum::routing::get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/rag.js"),
                )
            }),
        )
        .route(
            "/api/markdown/preview",
            axum::routing::post(markdown_preview),
        )
        .route("/api/storage", axum::routing::get(crate::storage::list))
        .route("/api/storage/entry-sizes", axum::routing::get(crate::storage::entry_sizes))
        .route(
            "/api/storage/source-size",
            axum::routing::get(crate::storage::source_size),
        )
        .route(
            "/api/models/openrouter",
            axum::routing::get(openrouter_models),
        )
        .route(
            "/api/storage/download",
            axum::routing::get(crate::storage::download),
        )
        .route(
            "/api/storage/upload",
            axum::routing::post(crate::storage::upload)
                .layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .route(
            "/api/mcp/{id}/probe",
            axum::routing::post(crate::mcp_client::probe),
        )
        .route(
            "/api/mcp/{id}/oauth/start",
            axum::routing::post(crate::mcp_auth::start),
        )
        .route(
            "/api/mcp/{id}/oauth/disconnect",
            axum::routing::post(crate::mcp_auth::disconnect),
        )
        .route(
            "/api/mcp/oauth/callback",
            axum::routing::post(crate::mcp_auth::callback),
        )
        .route(
            "/api/mcp/agent",
            axum::routing::post(crate::mcp_client::agent),
        )
        .route(
            "/mcp/callback",
            axum::routing::get(|| async { Html(include_str!("../web/mcp-callback.html")) }),
        )
        .route(
            "/mcp.js",
            axum::routing::get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/mcp.js"),
                )
            }),
        )
        .route(
            "/mcp-oauth.js",
            axum::routing::get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/mcp-oauth.js"),
                )
            }),
        )
        .route("/api/finops", axum::routing::get(crate::finops::get))
        .route(
            "/api/connections/atlassian/status",
            axum::routing::get(twg_status).post(twg_check),
        )
        .route("/api/connections/atlassian/login", axum::routing::post(twg_login).get(twg_login_status))
        .route("/api/agents/pi/status", axum::routing::post(crate::pi_check::check))
        .route("/api/connections/servicenow/status", axum::routing::get(crate::servicenow::status).post(crate::servicenow::check_draft))
        .route("/api/capabilities", axum::routing::get(capabilities))
        .route("/api/environment", axum::routing::get(environment))
        .route(
            "/api/workflows",
            axum::routing::get(workflows).post(save_workflow),
        )
        .route(
            "/api/workflows/{id}/deletion",
            axum::routing::get(crate::deletion::preview),
        )
        .route(
            "/api/workflows/{id}",
            axum::routing::delete(crate::deletion::delete).patch(rename_workflow),
        )
        .route(
            "/api/workflow-templates",
            axum::routing::get(crate::templates::legacy),
        )
        .route("/api/workflows/validate", axum::routing::post(validate))
        .route("/api/runs", axum::routing::get(runs).post(create_run))
        .route("/api/runs/active-count", axum::routing::get(active_workflow_count))
        .route("/api/runs/{id}", axum::routing::get(run))
        .route("/api/runs/{id}/start", axum::routing::post(start))
        .route("/api/runs/{id}/continue", axum::routing::post(continue_run))
        .route(
            "/api/runs/{id}/revision/preview",
            axum::routing::post(revision_preview),
        )
        .route(
            "/api/runs/{id}/revision",
            axum::routing::post(revision_create),
        )
        .route("/api/runs/{id}/pause", axum::routing::post(pause))
        .route("/api/runs/{id}/stop", axum::routing::post(stop))
        .route("/api/runs/{id}/note", axum::routing::post(note))
        .route("/api/runs/{id}/decide", axum::routing::post(decide))
        .route("/api/runs/{id}/events", axum::routing::get(events))
        .route("/api/runs/{id}/stream", axum::routing::get(stream))
        .route("/api/artifacts/{id}", axum::routing::get(artifact))
        .route("/api/artifacts/{id}/download", axum::routing::get(artifact_download))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .layer(middleware::from_fn(security))
        .with_state(app)
}
async fn security(req: Request, next: Next) -> axum::response::Response {
    if !req.uri().path().starts_with("/a2a/")
        && ![axum::http::Method::GET, axum::http::Method::HEAD].contains(req.method())
        && req
            .headers()
            .get("x-nodo-request")
            .and_then(|h| h.to_str().ok())
            != Some("1")
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"Header CSRF mancante"})),
        )
            .into_response();
    }
    if let (Some(origin), Some(host)) = (
        req.headers().get("origin").and_then(|h| h.to_str().ok()),
        req.headers().get("host").and_then(|h| h.to_str().ok()),
    ) && reqwest::Url::parse(origin)
        .ok()
        .and_then(|u| {
            u.host_str().map(|h| {
                format!(
                    "{}{}",
                    h,
                    u.port().map(|p| format!(":{p}")).unwrap_or_default()
                )
            })
        })
        .as_deref()
        != Some(host)
    {
        return (StatusCode::FORBIDDEN, "Origin non consentita").into_response();
    }
    let mut r = next.run(req).await;
    for (k, v) in [
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "no-referrer"),
        ("cache-control", "no-store"),
        (
            "content-security-policy",
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        ),
    ] {
        r.headers_mut().insert(
            axum::http::HeaderName::from_static(k),
            axum::http::HeaderValue::from_static(v),
        );
    }
    r
}
async fn index() -> Html<&'static str> {
    Html(include_str!("../web/index.html"))
}
async fn css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../web/app.css"),
    )
}
async fn js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript")],
        include_str!("../web/app.js"),
    )
}
async fn finops_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript")],
        include_str!("../web/finops.js"),
    )
}
async fn i18n() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript")],
        include_str!("../web/i18n.js"),
    )
}
async fn icons() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "image/svg+xml")],
        include_str!("../web/icons.svg"),
    )
}
#[derive(Deserialize)]
struct Credentials {
    email: String,
    password: String,
}
async fn auth_status(State(a): State<App>) -> ApiResult<Json<Value>> {
    let count: i64 = a
        .store
        .db
        .lock()
        .unwrap()
        .query_row("SELECT count(*) FROM users", [], |r| r.get(0))
        .map_err(anyhow::Error::from)?;
    Ok(Json(
        json!({"setup_required":count==0,"registration_open":std::env::var("NODO_ALLOW_REGISTRATION").is_ok_and(|v|v=="1"),"version":env!("CARGO_PKG_VERSION")}),
    ))
}
async fn register(
    State(a): State<App>,
    Json(c): Json<Credentials>,
) -> ApiResult<impl IntoResponse> {
    let count: i64 = a
        .store
        .db
        .lock()
        .unwrap()
        .query_row("SELECT count(*) FROM users", [], |r| r.get(0))
        .map_err(anyhow::Error::from)?;
    if count > 0 && std::env::var("NODO_ALLOW_REGISTRATION").unwrap_or_default() != "1" {
        return Err(bad(
            "Registrazione chiusa. Chiedi all'amministratore di creare un account",
        ));
    }
    auth::create_user(&a, c.email.clone(), c.password.clone()).await?;
    let (u, t) = auth::login(&a, c.email, c.password).await?;
    Ok((
        [(header::SET_COOKIE, auth::cookie(&a, &t))],
        Json(json!({"id":u.id,"email":u.email})),
    ))
}
async fn login(State(a): State<App>, Json(c): Json<Credentials>) -> ApiResult<impl IntoResponse> {
    let (u, t) = auth::login(&a, c.email, c.password).await?;
    Ok((
        [(header::SET_COOKIE, auth::cookie(&a, &t))],
        Json(json!({"id":u.id,"email":u.email})),
    ))
}
async fn logout(State(a): State<App>, u: User, h: HeaderMap) -> ApiResult<impl IntoResponse> {
    let _ = u;
    let token = h
        .get("cookie")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| {
            s.split(';')
                .find_map(|p| p.trim().strip_prefix("nodo_session="))
        })
        .unwrap_or("");
    a.store
        .db
        .lock()
        .unwrap()
        .execute(
            "DELETE FROM tokens WHERE hash=?",
            [crate::store::hash(token)],
        )
        .map_err(anyhow::Error::from)?;
    Ok((
        [(
            header::SET_COOKIE,
            "nodo_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0",
        )],
        Json(json!({"ok":true})),
    ))
}
async fn me(State(a): State<App>, u: User) -> Json<Value> {
    Json(
        json!({"id":u.id,"email":u.email,"admin":u.id==1,"host_import":false,"isolated":a.isolate}),
    )
}
#[derive(Deserialize)]
struct PasswordChange {
    current: String,
    new: String,
}
async fn change_password(
    State(a): State<App>,
    u: User,
    Json(b): Json<PasswordChange>,
) -> ApiResult<impl IntoResponse> {
    auth::change_password(&a, &u, b.current, b.new).await?;
    Ok((
        [(
            header::SET_COOKIE,
            "nodo_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0",
        )],
        Json(json!({"ok":true})),
    ))
}
async fn add_user(
    State(a): State<App>,
    u: User,
    Json(c): Json<Credentials>,
) -> ApiResult<Json<Value>> {
    if u.id != 1 {
        return Err(bad("Solo l'amministratore può creare utenti"));
    }
    let user = auth::create_user(&a, c.email, c.password).await?;
    Ok(Json(json!({"id":user.id,"email":user.email})))
}
fn api_key_preview(key: &str) -> Option<String> {
    if key.is_empty() { return None; }
    let chars: Vec<_> = key.chars().collect();
    if chars.len() <= 8 { return Some("...".into()); }
    Some(format!("{}...{}", chars[..5].iter().collect::<String>(), chars[chars.len()-3..].iter().collect::<String>()))
}
fn public_settings(a: &App, u: &User, s: &Settings) -> Value {
    let mut effective = s.clone();
    crate::compaction::normalize(&mut effective);
    let s = &effective;
    let mut v = serde_json::to_value(s).unwrap();
    v["_revision"] = json!(settings_revision(s));
    v["pi_providers"]=crate::pi_providers::public_catalog();
    v["pi_local_endpoint_available"]=json!(std::env::var("PI_LOCAL_BASE_URL").is_ok_and(|value|!value.trim().is_empty()));
    let home = a.data.join("users").join(u.id.to_string());
    for (rt, cfg) in &s.agents {
        v["agents"][rt]["api_key"] = json!("");
        v["agents"][rt]["api_key_preview"] = json!(api_key_preview(&cfg.api_key));
        let inherited = agents::effective_agent_config(a, u.id, rt, cfg).unwrap_or_else(|_|cfg.clone());
        let provider_env = agents::provider_environment_in_home(rt, &inherited, &home, u.id);
        v["agents"][rt]["uses_admin_api_key"] = json!(u.id != 1 && !inherited.api_key.is_empty() && (cfg.api_key.is_empty() || (rt == "pi" && cfg.multi_provider)));
        let cloud = agents::uses_cloud_provider(&provider_env);
        v["agents"][rt]["configured"] = json!(if rt=="pi" {
            if cfg.multi_provider {crate::pi_providers::any_available_for_user(a,u.id,s,&home)}
            else {crate::pi_providers::available(&inherited,&home) && (cfg.provider!="local" || !cfg.model.trim().is_empty())}
        } else {
            agents::has_provider_credentials(&provider_env) || (!cloud && match rt.as_str() {
                "codex"=>home.join(".codex/auth.json").is_file() || (home.join(".codex/config.toml").is_file()&&home.join(".aws/credentials").is_file()),
                "claude"=>home.join(".claude/.credentials.json").is_file(), _=>false,
            })
        });
    }
    for (p, cfg) in &s.connections {
        v["connections"][p]["token"] = json!("");
        v["connections"][p]["password"] = json!("");
        v["connections"][p]["client_secret"] = json!("");
        v["connections"][p]["configured"] = json!(if p == "servicenow" { crate::servicenow::configured(cfg) } else { !cfg.token.is_empty() });
    }
    v["mcp_servers"] = json!(
        s.mcp_servers
            .iter()
            .map(|(id, c)| (id.clone(), crate::mcp_client::public_config(c)))
            .collect::<BTreeMap<_, _>>()
    );
    v
}
async fn settings(State(a): State<App>, u: User) -> ApiResult<Json<Value>> {
    Ok(Json(public_settings(&a, &u, &a.store.settings(u.id)?)))
}
async fn openrouter_models(_u: User) -> ApiResult<Json<Value>> {
    // The public catalog needs no API key. Proxy it through our authenticated API.
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|_| bad("Impossibile caricare i modelli OpenRouter"))?
        .get("https://openrouter.ai/api/v1/models")
        .send()
        .await
        .map_err(|_| {
            bad("OpenRouter non raggiungibile. Riprova o inserisci il modello manualmente")
        })?
        .error_for_status()
        .map_err(|_| bad("Catalogo OpenRouter non disponibile. Riprova più tardi"))?;
    let catalog: Value = response
        .json()
        .await
        .map_err(|_| bad("Catalogo OpenRouter non valido"))?;
    let entries = catalog["data"]
        .as_array()
        .ok_or_else(|| bad("Catalogo OpenRouter non valido"))?;
    let mut models: Vec<_> = entries
        .iter()
        .filter_map(|m| {
            let id = m["id"].as_str().filter(|id| !id.is_empty())?;
            Some(json!({"id":id,"name":m["name"].as_str().unwrap_or(id)}))
        })
        .collect();
    models.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    models.dedup_by(|a, b| a["id"] == b["id"]);
    if models.is_empty() {
        return Err(bad("Il catalogo OpenRouter è vuoto. Riprova più tardi"));
    }
    Ok(Json(json!({"models":models})))
}
fn reject_cross_service_credentials(s: &Settings) -> anyhow::Result<()> {
    if let Some(sn) = s.connections.get("servicenow") {
        for secret in [&sn.password, &sn.client_secret, &sn.token].into_iter().filter(|v| !v.is_empty()) {
            anyhow::ensure!(!s.agents.values().any(|agent| &agent.api_key == secret),
                "Una credenziale ServiceNow è finita nei campi di un agente. Salvataggio bloccato: controlla l’autocompilazione del browser o del gestore password.");
        }
    }
    Ok(())
}
fn backup_settings(a: &App, uid: i64, settings: &Settings) -> anyhow::Result<()> {
    use std::io::Write;
    let dir=a.data.join("system/settings-history").join(uid.to_string());
    std::fs::create_dir_all(&dir)?;crate::store::private(&dir,true)?;
    let path=dir.join(format!("{}-{}.enc",now(),id()));
    let mut file=std::fs::OpenOptions::new().create_new(true).write(true).open(&path)?;
    crate::store::private(&path,false)?;
    file.write_all(&a.store.encrypt(&serde_json::to_vec(settings)?)?)?;file.sync_all()?;
    Ok(())
}
const SETTINGS_CONFLICT: &str = "Le impostazioni sono cambiate. Riapri le impostazioni prima di salvare. Le modifiche inserite non sono state salvate.";
fn settings_revision(settings: &Settings) -> String {
    let mut s=settings.clone();crate::compaction::normalize(&mut s);
    crate::store::hash(serde_json::to_vec(&s).unwrap())
}
fn validate_settings(s: &mut Settings, old: &Settings) -> anyhow::Result<()> {
    if !["codex", "claude", "pi"].contains(&s.default_runtime.as_str()) {
        return Err(anyhow::anyhow!("Runtime non valido"));
    }
    if !["preview", "full"].contains(&s.activity_log_verbosity.as_str()) {
        return Err(anyhow::anyhow!("Verbosità registro attività non valida"));
    }
    if !(1..=100).contains(&s.activity_log_page_size) {
        return Err(anyhow::anyhow!("Esecuzioni per pagina: inserisci un valore da 1 a 100"));
    }
    s.source_copy_limit_bytes()?;
    s.file_preview_limit_bytes()?;
    crate::compaction::normalize(s);
    crate::compaction::validate(s)?;
    for (rt, cfg) in &mut s.agents {
        if !["codex", "claude", "pi"].contains(&rt.as_str())
            || cfg.model.len() > 200
            || cfg.api_key.len() > 4096
            || cfg.base_url.len() > 2048
        {
            return Err(anyhow::anyhow!("Configurazione agente non valida"));
        }
        if rt=="pi" {anyhow::ensure!(crate::pi_providers::known(&cfg.provider),"Provider Pi non valido.");}
        if !cfg.multi_provider || !cfg.base_url.trim().is_empty(){crate::pi_local::validate(cfg)?;}
        if cfg.api_key.is_empty() {
            cfg.api_key = old
                .agents
                .get(rt)
                .filter(|previous| rt != "pi" || (!crate::pi_local::endpoint_changed(cfg, previous) && cfg.provider==previous.provider))
                .map(|c| c.api_key.clone())
                .unwrap_or_default()
        } else if cfg.api_key == "__CLEAR__" {
            cfg.api_key.clear();
        }
        if rt == "claude" || (rt == "pi" && cfg.provider == "anthropic") {
            anyhow::ensure!(!cfg.api_key.starts_with("sk-ant-oat"), "Inserisci una API key Anthropic, non un token dell’abbonamento Claude.");
        }
    }
    for (p, c) in &mut s.connections {
        if !["github", "azure", "servicenow", "atlassian"].contains(&p.as_str()) {
            return Err(anyhow::anyhow!("Connettore sconosciuto"));
        }
        if p == "servicenow" {
            let previous = old.connections.get(p).cloned().unwrap_or_default();
            crate::servicenow::merge(c, &previous);
            crate::servicenow::validate_config(c)?;
        }
        if c.token.is_empty() {
            c.token = old
                .connections
                .get(p)
                .filter(|old| p != "atlassian" || old.username == c.username)
                .map(|c| c.token.clone())
                .unwrap_or_default()
        } else if c.token == "__CLEAR__" {
            c.token.clear();
        }
        if p == "atlassian" { crate::twg::validate_config(c)?; }
    }
    reject_cross_service_credentials(s)?;
    if s.mcp_servers.len() > 30 {
        return Err(anyhow::anyhow!("Massimo 30 server MCP personali"));
    }
    for (id, c) in &mut s.mcp_servers {
        if id.is_empty()
            || id.len() > 80
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(anyhow::anyhow!("ID server MCP non valido"));
        }
        crate::mcp_client::merge(c, old.mcp_servers.get(id).unwrap_or(&McpConfig::default()));
        crate::mcp_client::validate(c)?;
    }
    Ok(())
}
fn merge_config_patch(target: &mut Value, patch: &Value, schema: &Value) -> anyhow::Result<()> {
    let changes=patch.as_object().ok_or_else(||anyhow::anyhow!("Modifiche impostazioni non valide"))?;
    for (key,value) in changes {
        let template=schema.get(key).ok_or_else(||anyhow::anyhow!("Campo impostazioni non valido: {key}"))?;
        if key == "auth" { merge_config_patch(&mut target[key],value,template)?; }
        else { target[key]=value.clone(); }
    }
    Ok(())
}
fn merge_settings_patch(current: &Settings, changes: &Value) -> anyhow::Result<Settings> {
    let mut result=serde_json::to_value(current)?;
    let defaults=serde_json::to_value(Settings::default())?;
    for (group,value) in changes.as_object().ok_or_else(||anyhow::anyhow!("Modifiche impostazioni non valide"))? {
        anyhow::ensure!(defaults.get(group).is_some(),"Gruppo impostazioni non valido: {group}");
        if ["agents","connections","compaction","mcp_servers"].contains(&group.as_str()) {
            let map=value.as_object().ok_or_else(||anyhow::anyhow!("Gruppo impostazioni non valido"))?;
            for (id,patch) in map {
                if patch.is_null() {
                    anyhow::ensure!(group=="mcp_servers","Rimozione impostazioni non consentita");
                    result[group].as_object_mut().unwrap().remove(id);continue;
                }
                let schema=match group.as_str() {
                    "agents"=>{anyhow::ensure!(["codex","claude","pi"].contains(&id.as_str()),"Runtime non valido");serde_json::to_value(AgentConfig::default())?},
                    "connections"=>{anyhow::ensure!(["github","azure","servicenow","atlassian"].contains(&id.as_str()),"Connettore sconosciuto");serde_json::to_value(ConnectionConfig::default())?},
                    "compaction"=>{anyhow::ensure!(["codex","claude","pi"].contains(&id.as_str()),"Runtime non valido");serde_json::to_value(CompactionConfig::for_runtime(id))?},
                    _=>serde_json::to_value(McpConfig::default())?,
                };
                if result[group].get(id).is_none(){result[group][id]=schema.clone();}
                merge_config_patch(&mut result[group][id],patch,&schema)?;
            }
        } else { result[group]=value.clone(); }
    }
    Ok(serde_json::from_value(result)?)
}
async fn patch_settings(State(a): State<App>,u: User,Json(payload): Json<Value>) -> ApiResult<Json<Value>> {
    write_settings(a,u,payload,true).await
}
async fn save_settings(State(a): State<App>,u: User,Json(payload): Json<Value>) -> ApiResult<Json<Value>> {
    write_settings(a,u,payload,false).await
}
async fn write_settings(a: App,u: User,payload: Value,partial: bool) -> ApiResult<Json<Value>> {
    let revision=payload[if partial {"revision"}else{"_revision"}].as_str().ok_or_else(||auth::ApiError(StatusCode::CONFLICT,SETTINGS_CONFLICT.into()))?;
    let mut mcp_changed=false;
    let result=a.store.update_settings(u.id,|latest| {
        anyhow::ensure!(revision==settings_revision(latest),"{SETTINGS_CONFLICT}");
        let mut next=if partial {merge_settings_patch(latest,&payload["changes"])?}else{serde_json::from_value::<Settings>(payload.clone())?};
        if partial && payload["changes"]["agents"]["pi"].get("api_key").is_none()
            && let (Some(config),Some(previous))=(next.agents.get_mut("pi"),latest.agents.get("pi"))
            && (crate::pi_local::endpoint_changed(config,previous) || config.provider!=previous.provider) {
            config.api_key.clear();
        }
        if partial && payload["changes"]["connections"]["servicenow"]["client_secret"].as_str().is_none_or(|s|s.is_empty())
            && let (Some(config),Some(previous))=(next.connections.get_mut("servicenow"),latest.connections.get("servicenow"))
            && (config.client_id!=previous.client_id || config.base_url.trim_end_matches('/')!=previous.base_url.trim_end_matches('/')) {
            config.client_secret.clear();
        }
        validate_settings(&mut next,latest)?;
        mcp_changed=serde_json::to_value(&next.mcp_servers)?!=serde_json::to_value(&latest.mcp_servers)?;
        if serde_json::to_value(&next)?!=serde_json::to_value(&*latest)? {backup_settings(&a,u.id,latest)?;}
        *latest=next;Ok(())
    });
    let settings=result.map_err(|error|if error.to_string()==SETTINGS_CONFLICT {auth::ApiError(StatusCode::CONFLICT,SETTINGS_CONFLICT.into())}else{error.into()})?;
    if mcp_changed {a.mcp.clear_user(u.id).await;}
    a.slots.changed.notify_waiters();
    Ok(Json(public_settings(&a,&u,&settings)))
}

async fn twg_status(State(a): State<App>, u: User) -> ApiResult<Json<Value>> {
    Ok(Json(crate::twg::status(&a, u.id).await))
}
async fn twg_check(State(a): State<App>, u: User, Json(config):Json<ConnectionConfig>)->ApiResult<Json<Value>>{Ok(Json(crate::twg::check(&a,u.id,config).await?))}
async fn twg_login(State(a):State<App>,u:User)->ApiResult<Json<Value>>{Ok(Json(json!(crate::twg::begin_login(&a,u.id).await?)))}
async fn twg_login_status(State(a):State<App>,u:User)->ApiResult<Json<Value>>{Ok(Json(json!(crate::twg::login_status(&a,u.id).await)))}
async fn environment(State(a): State<App>, u: User) -> ApiResult<Json<Value>> {
    // Return only names with a nonblank value. Never expose values in responses or logs.
    let mut names: Vec<String> = std::env::vars_os()
        .filter(|(_, value)| !value.to_string_lossy().trim().is_empty())
        .map(|(name, _)| name.to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.dedup();
    let authentication = crate::agent_auth::describe(&a, u.id, &a.store.settings(u.id)?).await?;
    Ok(Json(
        json!({"names": names, "authentication": authentication}),
    ))
}
async fn capabilities(State(a): State<App>, u: User) -> ApiResult<Json<Value>> {
    let home = agents::user_home(&a, u.id)?;
    let mut result = BTreeMap::new();
    for rt in ["codex", "claude", "pi", "git", "cargo", "twg"] {
        let value = match agents::tool(&a, u.id, &home, rt, &["--version"], 15).await {
            Ok(v) => json!({"available":true,"version":v.trim()}),
            Err(e) => json!({"available":false,"error":e.to_string()}),
        };
        result.insert(rt, value);
    }
    Ok(Json(json!(result)))
}
async fn workflows(State(a): State<App>, u: User) -> ApiResult<Json<Vec<Workflow>>> {
    Ok(Json(a.store.workflows(u.id)?))
}
async fn save_workflow(
    State(a): State<App>,
    u: User,
    Json(mut w): Json<Workflow>,
) -> ApiResult<Json<Workflow>> {
    let lifecycle = a.lifecycle.clone();
    let _guard = lifecycle.read().await;
    if w.id.is_empty() {
        w.id = id();
    }
    if w.id.len() > 80 || w.nodes.len() > 200 || w.edges.len() > 600 || w.name.chars().count() > 160
    {
        return Err(bad("Bozza troppo grande"));
    }
    a.store.save_workflow(u.id, &w)?;
    Ok(Json(a.store.workflow(u.id, &w.id)?))
}
#[derive(Deserialize)]
struct RenameWorkflow {
    name: String,
}
async fn rename_workflow(
    State(a): State<App>,
    u: User,
    Path(id): Path<String>,
    Json(body): Json<RenameWorkflow>,
) -> ApiResult<Json<Workflow>> {
    let lifecycle = a.lifecycle.clone();
    let _guard = lifecycle.read().await;
    Ok(Json(a.store.rename_workflow(u.id, &id, &body.name)?))
}
async fn validate(
    State(a): State<App>,
    u: User,
    Json(w): Json<Workflow>,
) -> ApiResult<Json<Value>> {
    crate::subworkflows::validate_tree(&a, u.id, &w)?;
    Ok(Json(json!({"valid":true})))
}
#[derive(Deserialize)]
struct RunFilter {
    workflow_id: Option<String>,
}
async fn active_workflow_count(State(a): State<App>, u: User) -> ApiResult<Json<Value>> {
    Ok(Json(json!({"count":a.store.active_workflow_count(u.id)?})))
}
async fn runs(
    State(a): State<App>,
    u: User,
    Query(filter): Query<RunFilter>,
) -> ApiResult<Json<Value>> {
    let runs = match filter.workflow_id {
        Some(id) => a.store.workflow_runs(u.id, &id)?,
        None => a.store.runs(u.id)?,
    };
    let summaries = runs.iter().map(|r| -> anyhow::Result<Value> {
        let schedule_id = crate::schedules::run_schedule(&a, u.id, &r.id)?;
        Ok(json!({"id":r.id,"name":r.workflow.name,"workflow_id":r.workflow.id,"status":r.status,"current":r.current,"created":r.created,"updated":r.updated,"parent":r.parent,"schedule_id":schedule_id,"execution":r.execution}))
    }).collect::<anyhow::Result<Vec<_>>>()?;
    Ok(Json(json!(summaries)))
}
#[derive(Deserialize)]
pub(crate) struct StartRun {
    #[serde(default)]
    pub locale: Option<String>,
    pub workflow_id: String,
    #[serde(default)]
    pub input: String,
    pub base_run: Option<String>,
    #[serde(default)]
    pub allow_large_source_copy: bool,
    pub execution: Option<ExecutionPolicy>,
}
async fn create_run(
    State(a): State<App>,
    u: User,
    Json(b): Json<StartRun>,
) -> ApiResult<Json<Run>> {
    let lifecycle = a.lifecycle.clone();
    let _guard = lifecycle.write().await;
    Ok(Json(prepare_run(&a, u.id, b, None).await?))
}
pub(crate) async fn prepare_run(a: &App, uid: i64, b: StartRun, reserved_id: Option<String>) -> anyhow::Result<Run> {
    crate::auth::ensure_active(a, uid)?;
    let mut w = a.store.workflow(uid, &b.workflow_id)?;
    if let Some(policy) = b.execution {
        crate::supervision::validate(&policy)?;
        w.execution = policy;
    }
    crate::templates::resolve_report_language(&mut w, b.locale.as_deref())?;
    let copy_limit = a.store.settings(uid)?.source_copy_limit_bytes()?;
    for (i, n) in w.nodes.iter().filter(|n| n.kind == "Source").enumerate() {
        if n.source_type == "folder"
            && !crate::source_access::direct(n)
            && !(i == 0 && b.base_run.is_some())
        {
            let path = crate::storage::source_folder(&a, "sources", &n.location)?;
            let bytes = crate::artifacts::blocking(move || crate::agents::source_copy_size(&path, copy_limit)).await?;
            if bytes > copy_limit && !b.allow_large_source_copy {
                anyhow::bail!(
                    "La cartella supera {} MiB: conferma la copia oppure scegli Modifica sorgente permanentemente",
                    copy_limit / (1024 * 1024)
                );
            }
        }
    }
    let r = engine::create_reserved(a, uid, w, b.input, b.base_run, reserved_id).await?;
    engine::launch(a.clone(), uid, r.id.clone())?;
    Ok(r)
}

async fn run(State(a): State<App>, u: User, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let r = a.store.run(u.id, &id)?;
    let workspace_successor: Option<String> = a.store.db.lock().unwrap().query_row(
        "SELECT id FROM runs WHERE user_id=? AND json_extract(json,'$.context.workspace-predecessor')=? LIMIT 1",
        rusqlite::params![u.id,id],|row|row.get(0)).optional().map_err(anyhow::Error::from)?;
    let artifact_app=&a;
    let artifacts=a.store.artifacts(u.id,&id)?.iter().map(|a|json!({"id":a.id,"node":a.node,"path":a.path,"version":a.version,"hash":a.hash,"session":a.session,"turn":a.turn,"created":a.created,"file":crate::artifacts::file_info(artifact_app,u.id,&r,a)})).collect::<Vec<_>>();
    let children = r.children.iter().map(|call| {
        let child = a.store.run(u.id, &call.run)?;
        Ok(json!({"id":child.id,"node":call.node,"attempt":call.attempt,"name":child.workflow.name,"status":child.status,"current":child.current,"created":child.created}))
    }).collect::<anyhow::Result<Vec<Value>>>()?;
    let mut attention = r.clone();
    for _ in 0..crate::subworkflows::MAX_DEPTH {
        if attention.pending.is_some() {
            break;
        }
        let Some(call) = attention.children.iter().rev().find(|c| {
            c.node == attention.current && Some(&c.attempt) == attention.attempts.get(&c.node)
        }) else {
            break;
        };
        attention = a.store.run(u.id, &call.run)?;
    }
    let review_run = attention.pending.as_ref().map(|_| attention.id.clone());
    Ok(Json(
        json!({"workspace_successor":workspace_successor,"revision_available":crate::revision::is_available(&a,u.id,&r),"continue_nodes":crate::continuation::candidates(&a,u.id,&r),"run":r,"children":children,"review_run":review_run,"artifacts":artifacts,"decisions":a.store.decisions(u.id,&id)?}),
    ))
}
#[derive(Deserialize)]
struct Resume {
    #[serde(default)]
    acknowledge: bool,
}
async fn start(
    State(a): State<App>,
    u: User,
    Path(id): Path<String>,
    Json(b): Json<Resume>,
) -> ApiResult<Json<Value>> {
    let lifecycle = a.lifecycle.clone();
    let _guard = lifecycle.read().await;
    let r = a.store.run(u.id, &id)?;
    if r.pending.as_ref().is_some_and(|p| {
        r.execution.mode == ExecutionMode::Interactive
            || (r.execution.mode == ExecutionMode::Assisted && p.proposal.is_some())
    }) {
        return Err(bad("Rispondi alla review in attesa"));
    }
    if r.status == "completed" {
        return Err(bad("Run già completato: crea una nuova esecuzione"));
    }
    if ["failed", "interrupted"].contains(&r.status.as_str()) && !b.acknowledge {
        return Err(bad("Conferma il riesame degli effetti prima del retry"));
    }
    engine::launch(a, u.id, id)?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
struct ContinueRun {
    node: String,
    #[serde(default)]
    acknowledge: bool,
}
async fn continue_run(
    State(a): State<App>,
    u: User,
    Path(id): Path<String>,
    Json(b): Json<ContinueRun>,
) -> ApiResult<Json<Value>> {
    let lifecycle = a.lifecycle.clone();
    let _guard = lifecycle.write().await;
    crate::continuation::prepare(&a, u.id, &id, &b.node, b.acknowledge)?;
    engine::launch(a, u.id, id)?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
struct RevisionBody {
    workflow: Workflow,
    node: Option<String>,
    #[serde(default)]
    token: String,
    #[serde(default)]
    acknowledge: bool,
}
async fn revision_preview(
    State(a): State<App>,
    u: User,
    Path(id): Path<String>,
    Json(b): Json<RevisionBody>,
) -> ApiResult<Json<crate::revision::Plan>> {
    let lifecycle = a.lifecycle.clone();
    let _guard = lifecycle.read().await;
    Ok(Json(crate::artifacts::blocking(move || crate::revision::preview(
        &a, u.id, &id, &b.workflow, b.node.as_deref(),
    )).await?))
}
async fn revision_create(
    State(a): State<App>,
    u: User,
    Path(id): Path<String>,
    Json(b): Json<RevisionBody>,
) -> ApiResult<Json<Run>> {
    let lifecycle = a.lifecycle.clone();
    let _guard = lifecycle.write().await;
    if !b.acknowledge {
        return Err(bad("Conferma l’anteprima prima di continuare"));
    }
    let (r, created) =
        crate::revision::create(&a, u.id, &id, b.workflow, b.node.as_deref(), &b.token).await?;
    if created && let Err(error) = engine::launch(a.clone(), u.id, r.id.clone()) {
        let mut saved = a.store.run(u.id, &r.id)?;
        saved.error = Some(format!(
            "Run creata; avvio non riuscito: {error}. Usa Riprendi per riprovare."
        ));
        a.store.save_run(u.id, &saved)?;
    }
    Ok(Json(a.store.run(u.id, &r.id)?))
}

async fn stop(State(a): State<App>, u: User, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let lifecycle = a.lifecycle.clone();
    let _guard = lifecycle.write().await;
    crate::subworkflows::stop(&a, u.id, &id).await?;
    Ok(Json(json!({"ok": true})))
}

async fn pause(State(a): State<App>, u: User, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let lifecycle = a.lifecycle.clone();
    let _guard = lifecycle.read().await;
    crate::subworkflows::pause(&a, u.id, &id)?;
    Ok(Json(
        json!({"ok":true,"message":"Pausa al termine del turno attivo: nessuna sessione viene persa"}),
    ))
}
#[derive(Deserialize)]
struct NoteBody {
    text: String,
}
async fn note(
    State(a): State<App>,
    u: User,
    Path(id): Path<String>,
    Json(b): Json<NoteBody>,
) -> ApiResult<Json<Run>> {
    let lifecycle = a.lifecycle.clone();
    let _guard = lifecycle.read().await;
    Ok(Json(engine::annotate(&a, &u, &id, &b.text)?))
}
#[derive(Deserialize)]
struct DecisionBody {
    action: String,
    #[serde(default)]
    answer: String,
}
async fn decide(
    State(a): State<App>,
    u: User,
    Path(id): Path<String>,
    Json(b): Json<DecisionBody>,
) -> ApiResult<Json<Run>> {
    let lifecycle = a.lifecycle.clone();
    let _guard = lifecycle.read().await;
    let before = a.store.run(u.id, &id)?;
    crate::subworkflows::can_launch(&a, u.id, &before)?;
    let r = engine::decide(&a, &u, &id, &b.action, &b.answer)?;
    if r.status != "failed" {
        engine::launch(a, u.id, id)?;
    }
    Ok(Json(r))
}
#[derive(Deserialize, Default)]
struct After {
    #[serde(default)]
    after: i64,
}
async fn events(
    State(a): State<App>,
    u: User,
    Path(id): Path<String>,
    Query(q): Query<After>,
) -> ApiResult<Json<Vec<Event>>> {
    a.store.run(u.id, &id)?;
    Ok(Json(a.store.events(u.id, &id, q.after)?))
}
async fn stream(
    State(a): State<App>,
    u: User,
    Path(id): Path<String>,
    h: HeaderMap,
) -> ApiResult<Sse<impl futures::Stream<Item = Result<SseEvent, Infallible>>>> {
    a.store.run(u.id, &id)?;
    let mut rx = a.bus.subscribe();
    let token_hash = crate::store::hash(
        h.get("cookie")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| {
                s.split(';')
                    .find_map(|p| p.trim().strip_prefix("nodo_session="))
            })
            .unwrap_or(""),
    );
    let after = h
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let stream = async_stream::stream! {
        let valid=||a.store.db.lock().unwrap().query_row("SELECT count(*) FROM tokens WHERE hash=? AND expires>?",rusqlite::params![token_hash,now()],|r|r.get::<_,i64>(0)).unwrap_or(0)==1;
        // Always reconcile the snapshot on connection, including after a server restart.
        if valid(){yield Ok(SseEvent::default().event("refresh").data("refresh"));}
        let mut cursor=after;
        loop{if !valid(){break}let batch=a.store.events(u.id,&id,cursor).unwrap_or_default();let len=batch.len();for e in batch{cursor=e.seq;yield Ok(SseEvent::default().id(e.seq.to_string()).json_data(&e).unwrap());}if len<500{break}}
        loop{match rx.recv().await{Ok((uid,e))if uid==u.id&&e.run==id=>{if !valid(){break}yield Ok(SseEvent::default().id(e.seq.to_string()).json_data(&e).unwrap());},Ok(_)=>(),Err(tokio::sync::broadcast::error::RecvError::Lagged(_))=>{if !valid(){break}yield Ok(SseEvent::default().event("refresh").data("refresh"));},Err(_)=>break}}
    };
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
async fn artifact(State(a): State<App>, u: User, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let mut v = a.store.artifact(u.id, &id)?;
    let run = a.store.run(u.id, &v.run)?;
    let settings=a.store.settings(u.id)?;
    let limit=settings.file_preview_limit_bytes()?;
    let preview=crate::artifacts::text_preview(&a,u.id,&v,limit)?;
    let mut info=crate::artifacts::file_info(&a,u.id,&run,&v);
    info.preview=preview.is_some();
    let reason=if info.preview {None} else {Some(if info.size>limit {"size"} else {"format"})};
    v.content=preview.unwrap_or_default();v.file=Some(info);
    let safe = render_markdown(&v.content);
    Ok(Json(json!({"artifact":v,"html":safe,"preview_reason":reason,"preview_limit_mb":settings.file_preview_limit_mb})))
}
async fn artifact_download(State(app): State<App>, user: User, Path(id): Path<String>) -> ApiResult<axum::response::Response> {
    use tokio::io::AsyncReadExt;
    let artifact=app.store.artifact(user.id,&id)?;
    let body=if let Some(path)=crate::artifacts::download_path(&app,user.id,&artifact)? {
        let mut file=tokio::fs::File::open(path).await.map_err(|e|bad(e.to_string()))?;
        let stream=async_stream::stream! {
            let mut buffer=vec![0u8;65536];
            loop {match file.read(&mut buffer).await {
                Ok(0)=>break,
                Ok(n)=>yield Ok::<_,std::io::Error>(axum::body::Bytes::copy_from_slice(&buffer[..n])),
                Err(error)=>{yield Err(error);break}
            }}
        };
        axum::body::Body::from_stream(stream)
    } else {axum::body::Body::from(artifact.content)};
    let name=std::path::Path::new(&artifact.path).file_name().unwrap_or_default().to_string_lossy();
    let encoded=name.bytes().map(|b|if b.is_ascii_alphanumeric() || b"-_.".contains(&b) {(b as char).to_string()} else {format!("%{b:02X}")}).collect::<String>();
    let mut response=body.into_response();
    response.headers_mut().insert(header::CONTENT_TYPE,axum::http::HeaderValue::from_static("application/octet-stream"));
    response.headers_mut().insert(header::CONTENT_DISPOSITION,format!("attachment; filename*=UTF-8''{encoded}").parse().map_err(|_|bad("Nome file non valido"))?);
    Ok(response)
}
#[derive(Deserialize)]
struct MarkdownPreview {
    text: String,
}
async fn markdown_preview(_u: User, Json(body): Json<MarkdownPreview>) -> ApiResult<Json<Value>> {
    if body.text.len() > 64000 {
        return Err(bad("Commento troppo lungo"));
    }
    Ok(Json(json!({"html":render_markdown(&body.text)})))
}
fn markdown_html(content: &str) -> String {
    let mut html = String::new();
    pulldown_cmark::html::push_html(
        &mut html,
        pulldown_cmark::Parser::new_ext(
            content,
            pulldown_cmark::Options::ENABLE_TABLES
                | pulldown_cmark::Options::ENABLE_TASKLISTS
                | pulldown_cmark::Options::ENABLE_STRIKETHROUGH,
        )
        .map(|event| match event {
            pulldown_cmark::Event::TaskListMarker(checked) => {
                pulldown_cmark::Event::Text(if checked {
                    "[x] ".into()
                } else {
                    "[ ] ".into()
                })
            }
            other => other,
        }),
    );
    html
}
pub(crate) fn render_markdown(content: &str) -> String {
    ammonia::Builder::default()
        .url_schemes(std::collections::HashSet::new())
        .clean(&markdown_html(content))
        .to_string()
}
pub(crate) fn render_chat_markdown(content: &str) -> String {
    ammonia::Builder::default()
        .url_schemes(["http", "https", "mailto"].into_iter().collect())
        .rm_tags(&["img"])
        .clean(&markdown_html(content))
        .to_string()
}

#[cfg(test)]
mod api_key_preview_tests {
    use super::api_key_preview;
    #[test]
    fn previews_never_return_a_complete_key() {
        assert_eq!(api_key_preview(""), None);
        assert_eq!(api_key_preview("sk-12-secret-middle-xyz").as_deref(), Some("sk-12...xyz"));
        assert_eq!(api_key_preview("short").as_deref(), Some("..."));
        assert_eq!(api_key_preview("12345678").as_deref(), Some("..."));
        assert_eq!(api_key_preview("àèìòù-hidden-ééé").as_deref(), Some("àèìòù...ééé"));
    }
}

#[cfg(test)]
mod artifact_download_tests {
    use super::*;
    #[tokio::test]
    async fn artifact_download_preserves_binary_bytes_names_and_ownership() {
        let dir=tempfile::tempdir().unwrap();
        let app=crate::tests::app(dir.path());
        let bytes=b"\0\xffbinary file";
        let hash=crate::store::hash(bytes);
        let path=crate::artifacts::blob_path(&app,1,&hash).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();std::fs::write(&path,bytes).unwrap();
        let artifact=Artifact {id:id(),run:"run".into(),node:"task".into(),path:"risultato è.bin".into(),version:1,hash,content:String::new(),session:String::new(),turn:1,created:now(),file:Some(ArtifactFile {modified:now() as u64*1000,root:"/data".into(),location:"/data/result.bin".into(),change:"created".into(),size:bytes.len() as u64,preview:false,blob:true})};
        app.store.add_artifact(1,&artifact).unwrap();
        let response=artifact_download(State(app.clone()),User {id:1,email:String::new()},Path(artifact.id.clone())).await.unwrap();
        assert_eq!(response.headers()[header::CONTENT_TYPE],"application/octet-stream");
        assert!(response.headers()[header::CONTENT_DISPOSITION].to_str().unwrap().contains("risultato%20%C3%A8.bin"));
        assert_eq!(&axum::body::to_bytes(response.into_body(),1024).await.unwrap()[..],bytes);
        assert!(artifact_download(State(app),User {id:2,email:String::new()},Path(artifact.id)).await.is_err());
    }
}

#[cfg(test)]
mod settings_isolation_tests {
    use super::*;
    fn user()->User{User{id:1,email:"test@local".into()}}
    #[tokio::test]
    async fn atlassian_settings_are_private_preserved_and_cleared_independently() {
        let dir=tempfile::tempdir().unwrap();let app=crate::tests::app(dir.path());
        let initial=public_settings(&app,&user(),&app.store.settings(1).unwrap());
        let first=patch_settings(State(app.clone()),user(),Json(json!({"revision":initial["_revision"],"changes":{"connections":{"atlassian":{"base_url":"https://demo.atlassian.net","username":"user@example.test","token":"private-bitbucket-token","repository":"workspace/repo"}}}}))).await.unwrap().0;
        assert_eq!(first["connections"]["atlassian"]["token"],"");assert_eq!(first["connections"]["atlassian"]["configured"],true);assert!(!first.to_string().contains("private-bitbucket-token"));
        let next=patch_settings(State(app.clone()),user(),Json(json!({"revision":first["_revision"],"changes":{"connections":{"atlassian":{"token":"","repository":"workspace/other"}}}}))).await.unwrap().0;
        assert_eq!(app.store.settings(1).unwrap().connections["atlassian"].token,"private-bitbucket-token");
        assert!(!std::fs::read(dir.path().join("system/nodo.sqlite")).unwrap().windows(b"private-bitbucket-token".len()).any(|s|s==b"private-bitbucket-token"));
        let other=public_settings(&app,&User{id:2,email:String::new()},&app.store.settings(2).unwrap());assert!(other["connections"].get("atlassian").is_none());
        let changed=patch_settings(State(app.clone()),user(),Json(json!({"revision":next["_revision"],"changes":{"connections":{"atlassian":{"username":"other@example.test","token":""}}}}))).await.unwrap().0;
        assert_eq!(changed["connections"]["atlassian"]["configured"],false);
        let cleared=patch_settings(State(app.clone()),user(),Json(json!({"revision":changed["_revision"],"changes":{"connections":{"atlassian":{"token":"__CLEAR__"}}}}))).await.unwrap().0;assert_eq!(cleared["connections"]["atlassian"]["configured"],false);
    }
    #[tokio::test]
    async fn partial_settings_preserve_other_services_and_reject_stale_writes() {
        let dir=tempfile::tempdir().unwrap();let app=crate::tests::app(dir.path());
        let original=public_settings(&app,&user(),&app.store.settings(1).unwrap());
        let first=patch_settings(State(app.clone()),user(),Json(json!({"revision":original["_revision"],"changes":{"connections":{"servicenow":{"base_url":"https://dev12345.service-now.com","authentication":"basic","username":"api.user","password":"test-pdi-secret"}}}}))).await.unwrap().0;
        let next=patch_settings(State(app.clone()),user(),Json(json!({"revision":first["_revision"],"changes":{"agents":{"pi":{"model":"chosen-model","api_key":"test-pi-key"}}}}))).await.unwrap().0;
        let saved=app.store.settings(1).unwrap();assert_eq!(saved.connections["servicenow"].password,"test-pdi-secret");assert_eq!(saved.agents["pi"].model,"chosen-model");assert_eq!(saved.agents["pi"].api_key,"test-pi-key");
        assert!(!next.to_string().contains("test-pdi-secret"));assert!(!next.to_string().contains("test-pi-key"));
        assert_eq!(save_settings(State(app.clone()),user(),Json(original.clone())).await.unwrap_err().0,StatusCode::CONFLICT);
        assert_eq!(patch_settings(State(app.clone()),user(),Json(json!({"revision":first["_revision"],"changes":{"agents":{"pi":{"model":"stale"}}}}))).await.unwrap_err().0,StatusCode::CONFLICT);
        assert_eq!(save_settings(State(app.clone()),user(),Json(json!({"agents":{}}))).await.unwrap_err().0,StatusCode::CONFLICT);
        assert_eq!(app.store.settings(1).unwrap().agents["pi"].model,"chosen-model");
        assert!(patch_settings(State(app.clone()),user(),Json(json!({"revision":next["_revision"],"changes":{"agents":{"pi":{"model":"should-not-save"}},"connections":{"servicenow":{"base_url":"http://invalid.example"}}}}))).await.is_err());
        assert_eq!(app.store.settings(1).unwrap().agents["pi"].model,"chosen-model");
        let clear=patch_settings(State(app.clone()),user(),Json(json!({"revision":next["_revision"],"changes":{"agents":{"pi":{"api_key":"__CLEAR__"}}}}))).await.unwrap().0;
        assert!(app.store.settings(1).unwrap().agents["pi"].api_key.is_empty());assert_eq!(app.store.settings(1).unwrap().connections["servicenow"].password,"test-pdi-secret");
        assert!(patch_settings(State(app.clone()),user(),Json(json!({"revision":clear["_revision"],"changes":{"unknown":"bad"}}))).await.is_err());
    }
    #[tokio::test]
    async fn concurrent_saves_are_atomic_and_mcp_changes_are_scoped() {
        let dir=tempfile::tempdir().unwrap();let app=crate::tests::app(dir.path());let revision=settings_revision(&app.store.settings(1).unwrap());
        let a=json!({"revision":revision,"changes":{"agents":{"pi":{"model":"first"}}}});
        let b=json!({"revision":revision,"changes":{"agents":{"pi":{"model":"second"}}}});
        let (left,right)=tokio::join!(patch_settings(State(app.clone()),user(),Json(a)),patch_settings(State(app.clone()),user(),Json(b)));
        assert_ne!(left.is_ok(),right.is_ok());let error=if let Err(e)=left{e}else{right.unwrap_err()};assert_eq!(error.0,StatusCode::CONFLICT);
        let revision=settings_revision(&app.store.settings(1).unwrap());
        let added=patch_settings(State(app.clone()),user(),Json(json!({"revision":revision,"changes":{"mcp_servers":{"demo":{"name":"Demo","transport":"stdio","command":"echo","args":[],"env":{"TOKEN":"dummy"},"auth":{"mode":"none"}}}}}))).await.unwrap().0;
        assert_eq!(app.store.settings(1).unwrap().mcp_servers["demo"].env["TOKEN"],"dummy");
        let _ = patch_settings(State(app.clone()),user(),Json(json!({"revision":added["_revision"],"changes":{"mcp_servers":{"demo":null}}}))).await.unwrap();
        let saved=app.store.settings(1).unwrap();assert!(saved.mcp_servers.is_empty());assert!(["first","second"].contains(&saved.agents["pi"].model.as_str()));
    }
    #[tokio::test]
    async fn local_endpoint_changes_do_not_reuse_existing_provider_secrets() {
        let dir=tempfile::tempdir().unwrap();let app=crate::tests::app(dir.path());
        let revision=settings_revision(&app.store.settings(1).unwrap());
        let first=patch_settings(State(app.clone()),user(),Json(json!({"revision":revision,"changes":{"agents":{"pi":{"provider":"local","base_url":"http://127.0.0.1:1234/v1","model":"local-model","api_key":"first-endpoint-key"}}}}))).await.unwrap().0;
        assert_eq!(first["agents"]["pi"]["configured"],true);
        let second=patch_settings(State(app.clone()),user(),Json(json!({"revision":first["_revision"],"changes":{"agents":{"pi":{"base_url":"http://127.0.0.1:11434/v1"}}}}))).await.unwrap().0;
        assert!(app.store.settings(1).unwrap().agents["pi"].api_key.is_empty());assert_eq!(second["agents"]["pi"]["configured"],true);
        let keyed=patch_settings(State(app.clone()),user(),Json(json!({"revision":second["_revision"],"changes":{"agents":{"pi":{"api_key":"second-endpoint-key"}}}}))).await.unwrap().0;
        let _=patch_settings(State(app.clone()),user(),Json(json!({"revision":keyed["_revision"],"changes":{"agents":{"pi":{"provider":"openai"}}}}))).await.unwrap();
        assert!(app.store.settings(1).unwrap().agents["pi"].api_key.is_empty());
        let before=serde_json::to_value(app.store.settings(1).unwrap()).unwrap();let revision=settings_revision(&app.store.settings(1).unwrap());
        assert!(patch_settings(State(app.clone()),user(),Json(json!({"revision":revision,"changes":{"agents":{"pi":{"provider":"local","base_url":"file:///tmp/server"}}}}))).await.is_err());
        assert_eq!(serde_json::to_value(app.store.settings(1).unwrap()).unwrap(),before);
    }

}

#[cfg(test)]
mod servicenow_oauth_settings_tests {
    use super::*;
    fn user()->User{User{id:1,email:"oauth-settings@test.local".into()}}
    #[tokio::test]
    async fn oauth_settings_are_masked_encrypted_preserved_and_cleared(){
        let dir=tempfile::tempdir().unwrap();let app=crate::tests::app(dir.path());
        let initial=public_settings(&app,&user(),&app.store.settings(1).unwrap());
        let first=patch_settings(State(app.clone()),user(),Json(json!({"revision":initial["_revision"],"changes":{"connections":{"servicenow":{"base_url":"https://dev12345.service-now.com","authentication":"client_credentials","client_id":"client-test","client_secret":"private-client-secret-123","oauth_scope":"useraccount"}}}}))).await.unwrap().0;
        assert_eq!(first["connections"]["servicenow"]["client_secret"],"");assert_eq!(first["connections"]["servicenow"]["configured"],true);assert!(!first.to_string().contains("private-client-secret-123"));
        assert_eq!(app.store.settings(1).unwrap().connections["servicenow"].client_secret,"private-client-secret-123");
        let second=patch_settings(State(app.clone()),user(),Json(json!({"revision":first["_revision"],"changes":{"connections":{"servicenow":{"client_secret":"","oauth_scope":""}}}}))).await.unwrap().0;
        assert_eq!(second["connections"]["servicenow"]["configured"],true);
        let rejected=patch_settings(State(app.clone()),user(),Json(json!({"revision":second["_revision"],"changes":{"agents":{"pi":{"api_key":"private-client-secret-123"}}}}))).await;assert!(rejected.is_err());
        let third=patch_settings(State(app.clone()),user(),Json(json!({"revision":second["_revision"],"changes":{"connections":{"servicenow":{"client_id":"new-client"}}}}))).await.unwrap().0;assert_eq!(third["connections"]["servicenow"]["configured"],false);
        let fourth=patch_settings(State(app.clone()),user(),Json(json!({"revision":third["_revision"],"changes":{"connections":{"servicenow":{"client_secret":"replacement-secret"}}}}))).await.unwrap().0;assert_eq!(fourth["connections"]["servicenow"]["configured"],true);
        let cleared=patch_settings(State(app.clone()),user(),Json(json!({"revision":fourth["_revision"],"changes":{"connections":{"servicenow":{"client_secret":"__CLEAR__"}}}}))).await.unwrap().0;assert_eq!(cleared["connections"]["servicenow"]["configured"],false);
        let bytes=std::fs::read(dir.path().join("system/nodo.sqlite")).unwrap();assert!(!bytes.windows("private-client-secret-123".len()).any(|w|w==b"private-client-secret-123"));
    }
}
