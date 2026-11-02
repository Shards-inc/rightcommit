use crate::{
    App,
    auth::{ApiResult, bad},
    model::*,
};
use anyhow::{Context, Result, ensure};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, time::Duration};

pub fn http() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()?)
}
pub fn url(value: &str) -> Result<reqwest::Url> {
    let u = reqwest::Url::parse(value)?;
    ensure!(
        u.username().is_empty()
            && u.password().is_none()
            && u.fragment().is_none()
            && (u.scheme() == "https"
                || u.scheme() == "http"
                    && matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))),
        "MCP richiede HTTPS (HTTP consentito solo su loopback), senza credenziali nell'URL"
    );
    Ok(u)
}
pub fn fingerprint(cfg: &McpConfig) -> String {
    let mut c = cfg.clone();
    c.auth.access_token.clear();
    c.auth.refresh_token.clear();
    c.auth.expires = 0;
    crate::store::hash(serde_json::to_vec(&c).unwrap())
}
pub async fn endpoints(cfg: &McpConfig) -> Result<(String, String)> {
    let a = &cfg.auth;
    if !a.token_url.is_empty() {
        url(&a.token_url)?;
        if !a.authorization_url.is_empty() {
            url(&a.authorization_url)?;
        }
        return Ok((a.authorization_url.clone(), a.token_url.clone()));
    }
    let issuer = url(&a.issuer)?;
    let origin = issuer.origin().ascii_serialization();
    let path = issuer.path().trim_end_matches('/');
    let metadata = format!("{origin}/.well-known/oauth-authorization-server{path}");
    let res = http()?.get(metadata).send().await?;
    ensure!(
        res.status().is_success(),
        "Metadata OAuth non disponibili: configura gli endpoint"
    );
    let m: Value = res.json().await?;
    ensure!(
        m["issuer"]
            .as_str()
            .is_some_and(|s| s.trim_end_matches('/') == a.issuer.trim_end_matches('/')),
        "Issuer OAuth non corrispondente"
    );
    let auth = m["authorization_endpoint"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let token = m["token_endpoint"]
        .as_str()
        .context("Token endpoint OAuth mancante")?
        .to_string();
    if !auth.is_empty() {
        url(&auth)?;
    }
    url(&token)?;
    Ok((auth, token))
}
async fn exchange(cfg: &McpConfig, fields: &BTreeMap<String, String>) -> Result<Value> {
    let (_, endpoint) = endpoints(cfg).await?;
    let mut form = fields.clone();
    form.insert("resource".into(), cfg.url.clone());
    if !cfg.auth.scope.is_empty()
        && fields
            .get("grant_type")
            .is_none_or(|s| s != "authorization_code")
    {
        form.insert("scope".into(), cfg.auth.scope.clone());
    }
    let mut req = http()?.post(endpoint);
    match cfg.auth.token_auth.as_str() {
        "basic" => req = req.basic_auth(&cfg.auth.client_id, Some(&cfg.auth.client_secret)),
        "post" => {
            form.insert("client_id".into(), cfg.auth.client_id.clone());
            form.insert("client_secret".into(), cfg.auth.client_secret.clone());
        }
        _ => {
            form.insert("client_id".into(), cfg.auth.client_id.clone());
        }
    }
    let res = req
        .form(&form)
        .send()
        .await
        .context("OAuth non raggiungibile")?;
    ensure!(
        res.status().is_success(),
        "Autenticazione OAuth rifiutata (HTTP {})",
        res.status().as_u16()
    );
    let value: Value = res.json().await.context("Risposta OAuth non valida")?;
    ensure!(
        value["token_type"]
            .as_str()
            .is_some_and(|v| v.eq_ignore_ascii_case("bearer"))
            && value["access_token"]
                .as_str()
                .is_some_and(|v| !v.is_empty()),
        "Token OAuth Bearer mancante"
    );
    Ok(value)
}
fn save_tokens(app: &App, uid: i64, id: &str, cfg: &McpConfig, value: &Value) -> Result<String> {
    let settings = app.store.update_settings(uid, |settings| {
        let current = settings
            .mcp_servers
            .get_mut(id)
            .context("Server MCP rimosso")?;
        ensure!(
            fingerprint(current) == fingerprint(cfg),
            "Configurazione MCP cambiata: ripeti il login"
        );
        current.auth.access_token = value["access_token"].as_str().unwrap().into();
        if let Some(refresh) = value["refresh_token"].as_str() {
            current.auth.refresh_token = refresh.into();
        }
        current.auth.expires = value["expires_in"]
            .as_i64()
            .filter(|s| *s > 0)
            .map(|s| now() + s.min(31_536_000))
            .unwrap_or(0);
        Ok(())
    })?;
    Ok(settings.mcp_servers[id].auth.access_token.clone())
}
pub async fn headers(
    app: &App,
    uid: i64,
    id: &str,
) -> Result<(McpConfig, BTreeMap<String, String>)> {
    let _guard = app.mcp.auth_lock.lock().await;
    let mut cfg = app
        .store
        .settings(uid)?
        .mcp_servers
        .remove(id)
        .context("Configura il server MCP nelle impostazioni personali")?;
    let mut h = cfg.headers.clone();
    let a = &cfg.auth;
    let authorization = match a.mode.as_str() {
        "none" => None,
        "bearer" => Some(format!("Bearer {}", a.token)),
        "api_key" => {
            h.insert(a.header.clone(), a.token.clone());
            None
        }
        "basic" => Some(format!(
            "Basic {}",
            STANDARD.encode(format!("{}:{}", a.username, a.password))
        )),
        "oauth_pkce" | "oauth_client_credentials" => {
            let token = if !a.access_token.is_empty() && (a.expires == 0 || a.expires > now() + 60)
            {
                a.access_token.clone()
            } else {
                let fields = if !a.refresh_token.is_empty() {
                    BTreeMap::from([
                        ("grant_type".into(), "refresh_token".into()),
                        ("refresh_token".into(), a.refresh_token.clone()),
                    ])
                } else {
                    ensure!(
                        a.mode == "oauth_client_credentials",
                        "Accedi al server MCP con OAuth prima di eseguire il workflow"
                    );
                    BTreeMap::from([("grant_type".into(), "client_credentials".into())])
                };
                let value = exchange(&cfg, &fields).await?;
                let token = save_tokens(app, uid, id, &cfg, &value)?;
                cfg = app.store.settings(uid)?.mcp_servers.remove(id).unwrap();
                token
            };
            Some(format!("Bearer {token}"))
        }
        _ => anyhow::bail!("Autenticazione MCP non supportata"),
    };
    if let Some(v) = authorization {
        h.insert("Authorization".into(), v);
    }
    Ok((cfg, h))
}
macro_rules! api_ensure {
    ($condition:expr,$message:expr) => {
        if !$condition {
            return Err(bad($message));
        }
    };
}
pub struct Login {
    pub uid: i64,
    pub server: String,
    pub verifier: String,
    pub redirect: String,
    pub expires: i64,
    pub fingerprint: String,
}
#[derive(Deserialize)]
pub struct Start {
    pub redirect_uri: String,
}
pub async fn start(
    State(app): State<App>,
    u: User,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Start>,
) -> ApiResult<Json<Value>> {
    let cfg = app
        .store
        .settings(u.id)?
        .mcp_servers
        .remove(&id)
        .context("Server MCP non trovato")?;
    api_ensure!(cfg.auth.mode == "oauth_pkce", "Seleziona OAuth con PKCE");
    let redirect = url(&body.redirect_uri)?;
    let host = headers
        .get("host")
        .and_then(|h| h.to_str().ok())
        .context("Host mancante")?;
    api_ensure!(
        redirect.path() == "/mcp/callback"
            && redirect.query().is_none()
            && redirect.authority() == host,
        "Redirect OAuth non consentito"
    );
    let (auth, _) = endpoints(&cfg).await?;
    let mut target = url(&auth)?;
    let state = format!("{}{}", crate::model::id(), crate::model::id());
    let verifier = format!("{}{}", crate::model::id(), crate::model::id());
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    target.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("client_id", cfg.auth.client_id.as_str()),
        ("redirect_uri", body.redirect_uri.as_str()),
        ("state", state.as_str()),
        ("code_challenge", challenge.as_str()),
        ("code_challenge_method", "S256"),
        ("scope", cfg.auth.scope.as_str()),
        ("resource", cfg.url.as_str()),
    ]);
    let mut logins = app.mcp.logins.lock().unwrap();
    logins.retain(|_, s| s.expires > now());
    api_ensure!(logins.len() < 100, "Troppi login OAuth in attesa");
    logins.insert(
        crate::store::hash(&state),
        Login {
            uid: u.id,
            server: id,
            verifier,
            redirect: body.redirect_uri,
            expires: now() + 600,
            fingerprint: fingerprint(&cfg),
        },
    );
    Ok(Json(json!({"url":target.as_str()})))
}
#[derive(Deserialize)]
pub struct Callback {
    pub state: String,
    pub code: String,
}
pub async fn callback(
    State(app): State<App>,
    u: User,
    Json(body): Json<Callback>,
) -> ApiResult<Json<Value>> {
    let key = crate::store::hash(&body.state);
    let login = {
        let mut logins = app.mcp.logins.lock().unwrap();
        let l = logins
            .get(&key)
            .context("Login OAuth scaduto o già utilizzato")?;
        api_ensure!(
            l.uid == u.id && l.expires > now(),
            "Login OAuth non valido per questo utente"
        );
        logins.remove(&key).unwrap()
    };
    api_ensure!(
        !body.code.is_empty() && body.code.len() < 8192,
        "Codice OAuth non valido"
    );
    let _guard = app.mcp.auth_lock.lock().await;
    let cfg = app
        .store
        .settings(u.id)?
        .mcp_servers
        .remove(&login.server)
        .context("Server MCP non trovato")?;
    api_ensure!(
        fingerprint(&cfg) == login.fingerprint,
        "Configurazione MCP cambiata"
    );
    let fields = BTreeMap::from([
        ("grant_type".into(), "authorization_code".into()),
        ("code".into(), body.code),
        ("code_verifier".into(), login.verifier),
        ("redirect_uri".into(), login.redirect),
    ]);
    let value = exchange(&cfg, &fields).await?;
    save_tokens(&app, u.id, &login.server, &cfg, &value)?;
    app.mcp.clear_user(u.id).await;
    Ok(Json(json!({"ok":true})))
}
pub async fn disconnect(
    State(app): State<App>,
    u: User,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    app.store.update_settings(u.id, |s| {
        let c = s
            .mcp_servers
            .get_mut(&id)
            .context("Server MCP non trovato")?;
        c.auth.access_token.clear();
        c.auth.refresh_token.clear();
        c.auth.expires = 0;
        Ok(())
    })?;
    app.mcp.clear_user(u.id).await;
    Ok(Json(json!({"ok":true})))
}
