//! GitHub's full REST surface and GraphQL use one bounded HTTP executor.
use crate::{agents, connectors, model::*};
use anyhow::{Context, Result, ensure};
use base64::Engine;
use reqwest::{
    Method, Url,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde_json::{Value, json};

const LIMIT: usize = 8 * 1024 * 1024;
pub(crate) struct Request {
    method: Method,
    url: Url,
    headers: HeaderMap,
    body: Option<Vec<u8>>,
}
fn object(raw: &str, label: &str) -> Result<serde_json::Map<String, Value>> {
    if raw.trim().is_empty() {
        return Ok(Default::default());
    }
    ensure!(raw.len() <= LIMIT, "{label}: dimensione massima 8 MiB");
    serde_json::from_str::<Value>(raw)?
        .as_object()
        .cloned()
        .with_context(|| format!("{label}: serve un oggetto JSON"))
}
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn prepare(n: &Node, c: &ConnectionConfig) -> Result<Request> {
    let method = if n.api_method.is_empty() {
        "GET"
    } else {
        n.api_method.trim()
    }
    .to_ascii_uppercase();
    ensure!(
        ["GET", "HEAD", "OPTIONS", "POST", "PUT", "PATCH", "DELETE"].contains(&method.as_str()),
        "Metodo GitHub non supportato"
    );
    let mut path = n.api_path.trim().to_string();
    ensure!(
        !path.is_empty()
            && path.len() <= 16384
            && !path.chars().any(|c| c.is_control())
            && !path.contains('\\')
            && !path.contains('#'),
        "Percorso GitHub non valido"
    );
    let parts: Vec<_> = c.repository.split('/').collect();
    let mut params = object(&n.api_parameters, "Parametri percorso")?;
    if parts.len() == 2 {
        params.entry("owner").or_insert(json!(parts[0]));
        params.entry("repo").or_insert(json!(parts[1]));
    }
    if !n.organization.is_empty() {
        params.entry("org").or_insert(json!(n.organization));
    }
    for (key, value) in params {
        ensure!(
            value.is_string() || value.is_number(),
            "Parametro percorso {key}: usa una stringa o un numero"
        );
        let value = value
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| value.to_string());
        ensure!(
            !value.is_empty() && value != "." && value != "..",
            "Parametro percorso {key} non valido"
        );
        path = path.replace(&format!("{{{key}}}"), &encode(&value));
    }
    ensure!(
        !path.contains('{') && !path.contains('}'),
        "Completa tutti i parametri del percorso GitHub"
    );
    let base = connectors::base(c)?;
    let origin = Url::parse(&base)?;
    let mut url = if path.starts_with("https://") {
        let url = Url::parse(&path)?;
        ensure!(
            url.origin() == origin.origin() || url.host_str() == Some("uploads.github.com"),
            "URL GitHub fuori dalla connessione configurata"
        );
        url
    } else {
        ensure!(
            path.starts_with('/') && !path.starts_with("//"),
            "Usa un percorso GitHub che inizia con / oppure un URL GitHub HTTPS"
        );
        Url::parse(&format!("{base}{path}"))?
    };
    ensure!(
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none_or(|p| p == 443),
        "URL GitHub non valido"
    );
    for (key, value) in object(&n.api_query, "Parametri query")? {
        let values = match value {
            Value::Array(values) => values,
            value => vec![value],
        };
        for value in values {
            ensure!(
                value.is_string() || value.is_number() || value.is_boolean(),
                "Parametro query {key} non valido"
            );
            url.query_pairs_mut()
                .append_pair(&key, value.as_str().unwrap_or(&value.to_string()));
        }
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        "accept",
        HeaderValue::from_static("application/vnd.github+json"),
    );
    headers.insert(
        "x-github-api-version",
        HeaderValue::from_static("2026-03-10"),
    );
    for (key, value) in object(&n.api_headers, "Header GitHub")? {
        let key = key.to_ascii_lowercase();
        ensure!(
            ![
                "authorization",
                "proxy-authorization",
                "host",
                "cookie",
                "connection",
                "content-length",
                "transfer-encoding",
                "user-agent",
                "upgrade",
                "te",
                "trailer"
            ]
            .contains(&key.as_str())
                && !key.starts_with("proxy-")
                && !key.starts_with("sec-"),
            "Header {key} riservato alla connessione"
        );
        headers.insert(
            HeaderName::from_bytes(key.as_bytes())?,
            HeaderValue::from_str(
                value
                    .as_str()
                    .context("I valori degli header devono essere stringhe")?,
            )?,
        );
    }
    let body = if n.api_body.trim().is_empty() {
        None
    } else {
        ensure!(
            !["GET", "HEAD", "OPTIONS"].contains(&method.as_str()),
            "Il metodo selezionato non ammette un corpo"
        );
        ensure!(
            n.api_body.len() <= LIMIT,
            "Corpo GitHub: dimensione massima 8 MiB"
        );
        Some(match n.api_encoding.as_str() {
            "" | "json" => {
                let body: Value =
                    serde_json::from_str(&n.api_body).context("Corpo GitHub JSON non valido")?;
                if !headers.contains_key("content-type") {
                    headers.insert("content-type", HeaderValue::from_static("application/json"));
                }
                serde_json::to_vec(&body)?
            }
            "text" => {
                if !headers.contains_key("content-type") {
                    headers.insert("content-type", HeaderValue::from_static("text/plain"));
                }
                n.api_body.as_bytes().to_vec()
            }
            "base64" => {
                if !headers.contains_key("content-type") {
                    headers.insert(
                        "content-type",
                        HeaderValue::from_static("application/octet-stream"),
                    );
                }
                base64::engine::general_purpose::STANDARD
                    .decode(n.api_body.trim())
                    .context("Corpo Base64 non valido")?
            }
            _ => anyhow::bail!("Formato corpo GitHub non supportato"),
        })
    };
    Ok(Request {
        method: Method::from_bytes(method.as_bytes())?,
        url,
        headers,
        body,
    })
}
pub(crate) fn preview(n: &Node, c: &ConnectionConfig) -> Result<String> {
    let c = connectors::node_config(n, c)?;
    let r = prepare(n, &c)?;
    Ok(serde_json::to_string_pretty(
        &json!({"method":r.method.as_str(),"url":r.url.as_str(),"headers":r.headers.iter().map(|(k,v)|(k.as_str(),v.to_str().unwrap_or(""))).collect::<std::collections::BTreeMap<_,_>>(),"body_encoding":n.api_encoding,"body":n.api_body}),
    )?)
}
pub(crate) async fn execute(
    n: &Node,
    c: &ConnectionConfig,
    settings: &Settings,
) -> Result<AgentResult> {
    send(prepare(n, c)?, c, settings).await
}
async fn send(r: Request, c: &ConnectionConfig, settings: &Settings) -> Result<AgentResult> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let mut url = r.url.clone();
    let mut redirects = 0;
    let mut resp = loop {
        let mut request = client
            .request(r.method.clone(), url.clone())
            .headers(r.headers.clone())
            .header("User-Agent", "rightcommit/0.1");
        // Download redirects never inherit the GitHub bearer token on another origin.
        if url.origin() == r.url.origin() {
            request = request.bearer_auth(&c.token);
        }
        if let Some(body) = &r.body {
            request = request.body(body.clone());
        }
        let resp=request.send().await.context("Richiesta GitHub non completata; per le scritture verifica l'esito remoto prima di riprovare")?;
        if resp.status().is_redirection() && r.method == Method::GET {
            ensure!(redirects < 5, "Troppi redirect GitHub");
            let next = url.join(
                resp.headers()
                    .get("location")
                    .context("Redirect GitHub senza Location")?
                    .to_str()?,
            )?;
            let host = next.host_str().unwrap_or("");
            ensure!(
                next.scheme() == "https"
                    && next.username().is_empty()
                    && next.password().is_none()
                    && (next.origin() == r.url.origin()
                        || host == "github.com"
                        || host.ends_with(".githubusercontent.com")),
                "Destinazione redirect GitHub non consentita"
            );
            url = next;
            redirects += 1;
            continue;
        }
        break resp;
    };
    let status = resp.status();
    let headers: std::collections::BTreeMap<_, _> = [
        "content-type",
        "link",
        "location",
        "x-ratelimit-remaining",
        "x-ratelimit-reset",
        "retry-after",
        "etag",
        "last-modified",
    ]
    .iter()
    .filter_map(|k| {
        resp.headers()
            .get(*k)
            .and_then(|v| v.to_str().ok())
            .map(|v| (*k, v.to_string()))
    })
    .collect();
    let mut bytes = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= LIMIT,
            "Risposta GitHub oltre 8 MiB: usa filtri o paginazione"
        );
        bytes.extend_from_slice(&chunk);
    }
    let (body, encoding) = if bytes.is_empty() {
        (Value::Null, "empty")
    } else if let Ok(json) = serde_json::from_slice::<Value>(&bytes) {
        (json, "json")
    } else if let Ok(text) = std::str::from_utf8(&bytes) {
        (json!(text), "text")
    } else {
        (
            json!(base64::engine::general_purpose::STANDARD.encode(&bytes)),
            "base64",
        )
    };
    ensure!(
        status.is_success() || status.as_u16() == 304,
        "GitHub HTTP {}: {}. Le scritture non vengono ritentate automaticamente.",
        status,
        agents::redact(&body.to_string(), settings)
            .chars()
            .take(1500)
            .collect::<String>()
    );
    ensure!(
        url.path() != "/graphql" || body.get("errors").is_none(),
        "GitHub GraphQL: {}",
        agents::redact(&body["errors"].to_string(), settings)
    );
    Ok(AgentResult {
        status: "completed".into(),
        decision: "pass".into(),
        summary: agents::redact(
            &serde_json::to_string_pretty(
                &json!({"status":status.as_u16(),"headers":headers,"encoding":encoding,"body":body}),
            )?,
            settings,
        ),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> ConnectionConfig {
        ConnectionConfig {
            base_url: "https://api.github.com".into(),
            repository: "team/demo".into(),
            ..Default::default()
        }
    }
    fn node() -> Node {
        Node {
            provider: "github".into(),
            operation: "api_request".into(),
            api_method: "GET".into(),
            api_path: "/repos/{owner}/{repo}/issues".into(),
            ..Default::default()
        }
    }
    #[test]
    fn targets_parameters_queries_and_headers() {
        let n = Node {
            api_path: "/repos/{owner}/{repo}/contents/{path}".into(),
            api_parameters: r#"{"path":"docs/my file.md"}"#.into(),
            api_query: r#"{"ref":"feature/test","page":2,"labels":["one","two"]}"#.into(),
            ..node()
        };
        let r = prepare(&n, &config()).unwrap();
        assert_eq!(
            r.url.path(),
            "/repos/team/demo/contents/docs%2Fmy%20file.md"
        );
        assert!(r.url.query().unwrap().contains("ref=feature%2Ftest"));
        assert_eq!(
            r.url.query_pairs().filter(|(k, _)| k == "labels").count(),
            2
        );
        assert_eq!(r.headers["x-github-api-version"], "2026-03-10");
    }
    #[test]
    fn blocks_foreign_targets_and_credential_overrides() {
        for path in [
            "https://evil.test/api",
            "//evil.test/api",
            "/repos/{missing}",
            "/user#secret",
            "/user\\evil",
        ] {
            assert!(
                prepare(
                    &Node {
                        api_path: path.into(),
                        ..node()
                    },
                    &config()
                )
                .is_err(),
                "{path}"
            );
        }
        for header in [
            "Authorization",
            "Host",
            "Cookie",
            "Proxy-Authorization",
            "Content-Length",
        ] {
            assert!(
                prepare(
                    &Node {
                        api_headers: json!({header:"bad"}).to_string(),
                        ..node()
                    },
                    &config()
                )
                .is_err()
            );
        }
    }
    #[test]
    fn all_write_methods_require_approval_and_track_payload() {
        for method in ["POST", "PUT", "PATCH", "DELETE", "invalid", ""] {
            assert!(connectors::is_write(&Node {
                api_method: method.into(),
                ..node()
            }));
        }
        for method in ["GET", "HEAD", "OPTIONS", "get"] {
            assert!(!connectors::is_write(&Node {
                api_method: method.into(),
                ..node()
            }));
        }
        let n = Node {
            api_method: "POST".into(),
            api_path: "/graphql".into(),
            api_body: r#"{"query":"query { viewer { login } }"}"#.into(),
            ..node()
        };
        let r = prepare(&n, &config()).unwrap();
        assert!(r.body.is_some());
        let run:Run=serde_json::from_value(json!({"id":"test","workflow":{"id":"test","name":"Test","description":"","entry":"","nodes":[],"edges":[]},"status":"running","current":"test","created":0,"updated":0,"workspace":"","states":{},"sessions":{},"outputs":{},"pending":null,"active_edge":null,"pause_requested":false,"attempts":{},"reworks":{},"context":{},"last_task":null,"last_node":null,"error":null,"input":""})).unwrap();
        let before = connectors::effect_digest(&n, &run, &Settings::default());
        for field in [
            "api_body",
            "api_path",
            "api_method",
            "api_parameters",
            "api_query",
            "api_headers",
            "api_encoding",
        ] {
            let mut changed = serde_json::to_value(&n).unwrap();
            changed[field] = json!("changed");
            assert_ne!(
                before,
                connectors::effect_digest(
                    &serde_json::from_value(changed).unwrap(),
                    &run,
                    &Settings::default()
                ),
                "{field}"
            );
        }
    }
    #[tokio::test]
    async fn http_execution_handles_payloads_empty_binary_pagination_and_errors() {
        use axum::{
            Router,
            body::Bytes,
            http::{HeaderMap, StatusCode, Uri},
            response::IntoResponse,
        };
        async fn handler(
            method: axum::http::Method,
            uri: Uri,
            headers: HeaderMap,
            body: Bytes,
        ) -> axum::response::Response {
            assert_eq!(headers["authorization"], "Bearer fixture-token");
            match uri.path() {
                "/empty" => {
                    assert_eq!(method, Method::DELETE);
                    StatusCode::NO_CONTENT.into_response()
                }
                "/binary" => (
                    [("content-type", "application/octet-stream")],
                    vec![0u8, 255],
                )
                    .into_response(),
                "/limited" => (
                    StatusCode::TOO_MANY_REQUESTS,
                    [("retry-after", "60")],
                    "rate limited",
                )
                    .into_response(),
                "/graphql" => axum::Json(json!({"errors":[{"message":"Denied"}]})).into_response(),
                _ => {
                    assert_eq!(method, Method::POST);
                    assert_eq!(uri.query(), Some("page=2"));
                    assert_eq!(
                        serde_json::from_slice::<Value>(&body).unwrap()["title"],
                        "Example"
                    );
                    (
                        [(
                            "link",
                            "<https://api.github.com/user/repos?page=3>; rel=\"next\"",
                        )],
                        axum::Json(json!({"ok":true})),
                    )
                        .into_response()
                }
            }
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, Router::new().fallback(handler))
                .await
                .unwrap()
        });
        let mut c = config();
        c.token = "fixture-token".into();
        for (path, method, payload, expected) in [
            ("/payload?page=2", "POST", r#"{"title":"Example"}"#, 201),
            ("/empty", "DELETE", "", 204),
            ("/binary", "GET", "", 200),
            ("/limited", "GET", "", 429),
            (
                "/graphql",
                "POST",
                r#"{"query":"query { viewer { login } }"}"#,
                400,
            ),
        ] {
            let mut request = prepare(
                &Node {
                    api_path: path.into(),
                    api_method: method.into(),
                    api_body: payload.into(),
                    ..node()
                },
                &c,
            )
            .unwrap();
            request.url = Url::parse(&format!("http://{addr}{path}")).unwrap();
            let result = send(request, &c, &Settings::default()).await;
            if expected >= 400 {
                assert!(result.is_err(), "{path}");
                continue;
            }
            let result: Value = serde_json::from_str(&result.unwrap().summary).unwrap();
            if path == "/empty" {
                assert_eq!(result["status"], 204);
                assert!(result["body"].is_null());
            }
            if path == "/binary" {
                assert_eq!(result["encoding"], "base64");
                assert_eq!(result["body"], "AP8=");
            }
            if path.starts_with("/payload") {
                assert!(
                    result["headers"]["link"]
                        .as_str()
                        .unwrap()
                        .contains("rel=\"next\"")
                );
            }
        }
        task.abort();
    }
    #[test]
    fn uploads_binary_and_body_validation() {
        let n = Node {
            api_method: "POST".into(),
            api_path:
                "https://uploads.github.com/repos/{owner}/{repo}/releases/1/assets?name=file.bin"
                    .into(),
            api_body: "AP8=".into(),
            api_encoding: "base64".into(),
            ..node()
        };
        assert_eq!(prepare(&n, &config()).unwrap().body.unwrap(), [0, 255]);
        assert!(
            prepare(
                &Node {
                    api_encoding: "json".into(),
                    ..n.clone()
                },
                &config()
            )
            .is_err()
        );
        assert!(
            prepare(
                &Node {
                    api_method: "GET".into(),
                    ..n
                },
                &config()
            )
            .is_err()
        );
    }
}
