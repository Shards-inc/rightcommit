//! ServiceNow credentials stay in encrypted personal settings, outside agent processes.
use crate::{App, agents, model::*};
use anyhow::{Context, Result, bail, ensure};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{io::Write, path::Path};

pub fn configured(c: &ConnectionConfig) -> bool {
    if c.authentication == "client_credentials" {
        !c.client_id.trim().is_empty() && !c.client_secret.is_empty()
    } else if c.authentication == "basic" {
        !c.username.is_empty() && !c.password.is_empty()
    } else {
        !c.token.is_empty()
    }
}
pub fn merge(c: &mut ConnectionConfig, old: &ConnectionConfig) {
    if c.client_secret == "__CLEAR__" { c.client_secret.clear(); }
    else if c.client_secret.is_empty() && c.base_url.trim_end_matches('/') == old.base_url.trim_end_matches('/') && c.client_id == old.client_id { c.client_secret = old.client_secret.clone(); }
    if c.password.is_empty() {
        c.password = old.password.clone();
    } else if c.password == "__CLEAR__" {
        c.password.clear();
    }
}
pub fn validate_config(c: &ConnectionConfig) -> Result<()> {
    ensure!(
        ["", "bearer", "basic", "client_credentials"].contains(&c.authentication.as_str()),
        "Autenticazione ServiceNow non valida"
    );
    ensure!(
        c.username.len() <= 200 && c.password.len() <= 4096 && c.token.len() <= 8192
            && c.client_id.len() <= 256 && c.client_secret.len() <= 4096 && c.oauth_scope.len() <= 2048,
        "Credenziali ServiceNow troppo lunghe"
    );
    ensure!(!c.oauth_scope.chars().any(|v| v.is_control()), "Scope OAuth ServiceNow non valido");
    if !c.base_url.is_empty() {
        base(c)?;
    }
    if !c.catalog_item.is_empty() {
        sys_id(&c.catalog_item)?;
    }
    ensure!(
        c.catalog_variables.len() <= 64000,
        "Variabili catalogo troppo grandi"
    );
    if !c.catalog_variables.trim().is_empty() {
        variables(c)?;
    }
    Ok(())
}
pub(crate) fn base(c: &ConnectionConfig) -> Result<String> {
    let u = reqwest::Url::parse(&c.base_url)?;
    let host = u.host_str().context("Host ServiceNow mancante")?;
    ensure!(
        u.scheme() == "https"
            && host.ends_with(".service-now.com")
            && u.username().is_empty()
            && u.password().is_none()
            && u.query().is_none()
            && u.fragment().is_none()
            && ["", "/"].contains(&u.path()),
        "Usa l’URL HTTPS dell’istanza ServiceNow senza percorsi o credenziali"
    );
    Ok(c.base_url.trim_end_matches('/').to_owned())
}
pub(crate) fn sys_id(s: &str) -> Result<&str> {
    ensure!(
        s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()),
        "Serve un sys_id ServiceNow di 32 caratteri esadecimali"
    );
    Ok(s)
}
pub(crate) fn variables(c: &ConnectionConfig) -> Result<Value> {
    let value: Value = serde_json::from_str(if c.catalog_variables.trim().is_empty() {
        "{}"
    } else {
        &c.catalog_variables
    })?;
    ensure!(
        value.is_object(),
        "Le variabili catalogo devono essere un oggetto JSON"
    );
    Ok(value)
}
pub(crate) fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    let s = v[key]
        .as_str()
        .with_context(|| format!("Campo {key} mancante"))?;
    ensure!(
        !s.trim().is_empty() && s.len() <= 24000,
        "Campo {key} vuoto o troppo grande"
    );
    Ok(s)
}
fn plan(r: &Run, n: &Node) -> Result<Value> {
    let root = std::fs::canonicalize(&r.workspace)
        .with_context(|| format!("Workspace non disponibile per leggere il file piano: {}", n.location))?;
    ensure!(
        !Path::new(&n.location).is_absolute(),
        "Usa un file piano relativo al workspace"
    );
    let path = std::fs::canonicalize(root.join(&n.location)).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            anyhow::anyhow!("File piano mancante: {}. Il task precedente deve generare questo file nel workspace prima di applicarlo a ServiceNow.", n.location)
        } else {
            anyhow::anyhow!("Impossibile accedere al file piano {}: {}", n.location, error)
        }
    })?;
    ensure!(
        path.starts_with(&root) && path.is_file() && path.metadata()?.len() <= if n.operation == "apply_ticket_batch" { 8_000_000 } else { 128000 },
        "File piano fuori dal workspace o troppo grande"
    );
    let bytes = std::fs::read(&path)
        .with_context(|| format!("Impossibile leggere il file piano: {}", n.location))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("File piano JSON non valido: {}", n.location))
}
/// Resolve only the native incident creation result in this run's upstream path.
fn created_incident(r: &Run, n: &Node, selection: Option<&str>) -> Result<(String, String, String)> {
    let mut upstream = std::collections::BTreeSet::new();
    let mut queue = vec![n.id.clone()];
    while let Some(id) = queue.pop() {
        for edge in r.workflow.edges.iter().filter(|e| e.to == id && e.kind != "context") {
            if upstream.insert(edge.from.clone()) { queue.push(edge.from.clone()); }
        }
    }
    let mut candidates = Vec::new();
    for source in &r.workflow.nodes {
        if !upstream.contains(&source.id) || (!n.target.is_empty() && source.id != n.target)
            || source.kind != "Connection" || source.provider != "servicenow"
            || !["create_incident", "apply_ticket_plan", "apply_ticket_batch"].contains(&source.operation.as_str())
            || r.states.get(&source.id).map(String::as_str) != Some("completed") { continue; }
        let Some(output) = r.outputs.get(&source.id).filter(|o| o.status == "completed") else { continue; };
        let data: Value = serde_json::from_str(&output.summary).context("Risultato nativo ServiceNow non leggibile")?;
        if source.operation=="apply_ticket_batch" {
            let Some(selection)=selection else {continue};
            let items=data["result"]["items"].as_array().context("Ricevuta lotto nativo non valida")?;
            for item in items {
                if item["action"]!="create_incident" || !matches!(item["status"].as_str(),Some("applied"|"reused")) || matches!(item["result"]["state"].as_str(),Some("6"|"7")) {continue;}
                let result=&item["result"];
                let selected=selection.strip_prefix("request:").is_some_and(|v|item["request_id"]==v)
                    || selection.strip_prefix("sysid:").is_some_and(|v|result["sys_id"]==v);
                if !selected {continue;}
                let number=text(result,"number")?;ensure!(number.starts_with("INC"),"Record non Incident nel lotto");
                candidates.push((source.id.clone(),sys_id(text(result,"sys_id")?)?.to_string(),number.to_string()));
            }
        } else {
            let result = &data["result"];
            let Some(number) = result["number"].as_str().filter(|s| s.starts_with("INC")) else { continue; };
            let id = sys_id(text(result, "sys_id")?)?;
            ensure!(result["correlation_id"].as_str().is_some_and(|c| c.starts_with(&format!("rightcommit:{}:", r.id))),
                "L’incidente del nodo {} non appartiene alla creazione di questa run", source.id);
            candidates.push((source.id.clone(), id.to_string(), number.to_string()));
        }
    }
    ensure!(candidates.len() == 1,
        "Risoluzione bloccata: serve un solo incidente creato da un nodo ServiceNow precedente della stessa run. Seleziona il nodo di creazione incidente (target). Non usare il risultato di check_connection.");
    Ok(candidates.remove(0))
}
fn resolution_target(r: &Run, n: &Node, p: &Value) -> Result<String> {
    let selection=p["request_id"].as_str().map(|v|format!("request:{v}"));
    let (source, expected, number) = created_incident(r, n, selection.as_deref())?;
    if r.workflow.node(&source)?.operation=="apply_ticket_batch" {
        let data:Value=serde_json::from_str(&r.outputs[&source].summary)?;
        ensure!(p["namespace"]==data["result"]["namespace"],"Namespace risoluzione diverso dal lotto nativo");
    }
    ensure!(sys_id(text(p, "sys_id")?)? == expected,
        "Destinatario risoluzione non valido in {}: atteso {} ({}) creato dal nodo {} di questa run. Il ticket indicato nel piano è diverso; nessuna richiesta inviata.", n.location, number, expected, source);
    ensure!(p.get("number").is_none_or(|v| v == &json!(number))
        && p.get("creation_node").is_none_or(|v| v == &json!(source))
        && p.get("source_run").is_none_or(|v| v == &json!(r.id)),
        "Provenienza del piano di risoluzione diversa dal risultato nativo della creazione");
    Ok(expected)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effect {
    pub instance: String,
    pub method: String,
    pub path: String,
    pub body: Value,
    pub correlation: String,
}
/// Build exactly once before approval. The engine hashes this snapshot and executes it unchanged.
pub fn prepare(n: &Node, r: &Run, c: &ConnectionConfig) -> Result<Effect> {
    let instance = base(c)?;
    let mut effect = Effect {
        instance,
        method: "POST".into(),
        path: "/api/now/table/incident".into(),
        body: Value::Null,
        correlation: String::new(),
    };
    if n.operation == "update_ticket" {
        effect.method = "PATCH".into();
        effect.path = format!("/api/now/table/incident/{}", sys_id(&n.location)?);
        effect.body = json!({"work_notes": format!("rightcommit run {}: {}", r.id, r.outputs.values().map(|o|o.summary.as_str()).collect::<Vec<_>>().join("\n"))});
        return Ok(effect);
    }
    let p = plan(r, n)?;
    if n.operation == "apply_ticket_batch" { return crate::itsm_batch::prepare(r, n, c, &p); }
    prepare_plan(n, r, c, &p)
}
pub(crate) fn prepare_plan(n: &Node, r: &Run, c: &ConnectionConfig, p: &Value) -> Result<Effect> {
    let mut effect = Effect { instance:base(c)?, method:"POST".into(), path:"/api/now/table/incident".into(), body:Value::Null, correlation:String::new() };
    if n.operation == "resolve_ticket" {
        ensure!(
            p["verified"] == true,
            "Confermare la soluzione verificata con verified: true nel piano di risoluzione"
        );
        let incident = resolution_target(r, n, &p)?;
        effect.method = "PATCH".into();
        effect.path = format!("/api/now/table/incident/{incident}");
        effect.body = json!({"state":"6", "close_code":text(&p,"close_code")?, "close_notes":text(&p,"close_notes")?});
        return Ok(effect);
    }
    let action = text(&p, "action")?;
    ensure!(
        n.operation == "apply_ticket_plan" || n.operation == action,
        "Operazione diversa dall’azione nel piano"
    );
    match action {
        "answer_only" => {
            ensure!(p["category"] == "Other", "Categoria incompatibile");
            effect.method = "NONE".into();
            effect.path.clear();
            effect.body = json!({"answer":text(&p,"answer")?});
        }
        "create_incident" => {
            ensure!(
                p["category"] == "Incident",
                "Categoria incompatibile con incident"
            );
            let id = text(&p, "request_id")?;
            ensure!(
                id.len() <= 80
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)),
                "request_id non valido"
            );
            effect.correlation = format!(
                "rightcommit:{}:{}",
                r.id,
                &crate::store::hash(format!("{}:{id}", n.id).as_bytes())[..24]
            );
            effect.body = json!({"short_description":text(&p,"short_description")?.chars().take(160).collect::<String>(),
                "description":text(&p,"description")?, "correlation_id":effect.correlation,
                "impact":"3", "urgency":"3", "work_notes":format!("Proposta KB, soluzione non ancora verificata:\n{}",text(&p,"answer")?)});
        }
        "order_catalog" => {
            ensure!(
                p["category"] == "Request",
                "Categoria incompatibile con richiesta"
            );
            effect.path = format!(
                "/api/sn_sc/servicecatalog/items/{}/order_now",
                sys_id(&c.catalog_item)?
            );
            effect.body = json!({"sysparm_quantity":"1", "variables":variables(c)?});
        }
        _ => bail!("Azione ITSM non supportata"),
    }
    Ok(effect)
}
fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()?)
}
fn auth(request: reqwest::RequestBuilder, c: &ConnectionConfig) -> Result<reqwest::RequestBuilder> {
    ensure!(
        configured(c),
        "Configura le credenziali ServiceNow nelle Personal Settings → Connessioni"
    );
    Ok(if c.authentication == "basic" {
        request.basic_auth(&c.username, Some(&c.password))
    } else {
        request.bearer_auth(&c.token)
    })
}
pub(crate) async fn send(
    client: &reqwest::Client,
    c: &ConnectionConfig,
    method: &str,
    path: &str,
    body: Option<&Value>,
) -> Result<Value> {
    let origin = base(c)?;
    send_at(client,c,method,path,body,&origin).await
}
async fn send_at(client:&reqwest::Client,c:&ConnectionConfig,method:&str,path:&str,body:Option<&Value>,origin:&str)->Result<Value> {
    ensure!(configured(c), "Configura le credenziali ServiceNow nelle Personal Settings → Connessioni");
    let mut response = None;
    for attempt in 0..2 {
        let mut effective = c.clone();
        if c.authentication == "client_credentials" {
            effective.token = crate::servicenow_oauth::token(client,c,origin).await?;
        }
        let mut req = client.request(method.parse()?,format!("{origin}{path}"));
        if let Some(body)=body {req=req.json(body);}
        let received=auth(req,&effective)?.header("User-Agent","rightcommit").send().await?;
        if received.status()==reqwest::StatusCode::UNAUTHORIZED && c.authentication=="client_credentials" {
            crate::servicenow_oauth::invalidate(c,origin,&effective.token).await;
            // Only reads are replayed. Never resend a write automatically.
            if attempt==0 && ["GET","HEAD"].contains(&method) {continue;}
        }
        ensure!(received.status().is_success(), "ServiceNow HTTP {}: verifica URL, credenziali e permessi",received.status());
        response=Some(received);break;
    }
    let response=response.context("Verifica autenticazione ServiceNow non riuscita")?;
    let data: Value = response.json().await?;
    data.get("result")
        .cloned()
        .context("Risposta ServiceNow senza result")
}
/// Unsaved connection checks never modify encrypted settings.
fn draft_config(mut draft: ConnectionConfig, saved: &ConnectionConfig) -> Result<ConnectionConfig> {
    let method = |c: &ConnectionConfig| {
        if c.authentication == "client_credentials" {
            "client_credentials"
        } else if c.authentication == "basic" {
            "basic"
        } else {
            "bearer"
        }
    };
    let same_identity = draft.base_url.trim_end_matches('/')
        == saved.base_url.trim_end_matches('/')
        && method(&draft) == method(saved)
        && (method(&draft) != "basic" || draft.username == saved.username)
        && (method(&draft) != "client_credentials" || draft.client_id == saved.client_id);
    if same_identity {
        if draft.client_secret.is_empty() {draft.client_secret=saved.client_secret.clone();}
        if draft.password.is_empty() {
            draft.password = saved.password.clone();
        }
        if draft.token.is_empty() {
            draft.token = saved.token.clone();
        }
    }
    if draft.client_secret == "__CLEAR__" {draft.client_secret.clear();}
    if draft.password == "__CLEAR__" {
        draft.password.clear();
    }
    if draft.token == "__CLEAR__" {
        draft.token.clear();
    }
    validate_config(&draft)?;
    ensure!(
        configured(&draft),
        "Inserisci le credenziali per verificare questa connessione. Quelle salvate si riutilizzano solo per la stessa istanza, utente e autenticazione."
    );
    Ok(draft)
}
pub async fn check_draft(
    State(app): State<App>,
    user: User,
    Json(draft): Json<ConnectionConfig>,
) -> crate::auth::ApiResult<Json<Value>> {
    let settings = app.store.settings(user.id)?;
    let saved = settings
        .connections
        .get("servicenow")
        .cloned()
        .unwrap_or_default();
    let config = draft_config(draft, &saved)?;
    check(&config).await
}
async fn check(c: &ConnectionConfig) -> crate::auth::ApiResult<Json<Value>> {
    let result = send(
        &client()?,
        c,
        "GET",
        "/api/now/table/incident?sysparm_limit=1&sysparm_fields=sys_id,number",
        None,
    )
    .await?;
    if !result.is_array() {
        return Err(crate::auth::bad(
            "Risposta di verifica ServiceNow non valida",
        ));
    }
    Ok(Json(
        json!({"connected":true,"authentication":if c.authentication.is_empty(){"bearer"}else{&c.authentication},"message":"Connessione e lettura incident verificate. Permessi di scrittura da verificare durante il workflow."}),
    ))
}
pub async fn status(State(app): State<App>, user: User) -> crate::auth::ApiResult<Json<Value>> {
    let settings = app.store.settings(user.id)?;
    let c = settings
        .connections
        .get("servicenow")
        .context("Configura ServiceNow nelle Personal Settings")?;
    check(c).await
}

pub async fn execute(app: &App, uid: i64, r: &Run, n: &Node) -> Result<AgentResult> {
    let settings = app.store.settings(uid)?;
    if n.operation=="read_request" {return crate::servicenow_request::execute(app,uid,r,n,settings.connections.get("servicenow")).await;}
    let c = settings
        .connections
        .get("servicenow")
        .context("Configura ServiceNow nelle Personal Settings")?;
    let client = client()?;
    let value = if n.operation == "check_connection" {
        send(
            &client,
            c,
            "GET",
            "/api/now/table/incident?sysparm_limit=1&sysparm_fields=sys_id,number",
            None,
        )
        .await?
    } else if n.operation == "read_ticket" {
        send(
            &client,
            c,
            "GET",
            &format!("/api/now/table/incident/{}", sys_id(&n.location)?),
            None,
        )
        .await?
    } else {
        // command is an engine-prepared snapshot; no secrets are included.
        let effect: Effect = serde_json::from_str(&n.command)
            .context("Anteprima ServiceNow non preparata dal motore")?;
        ensure!(
            effect.instance == base(c)?,
            "Istanza cambiata dopo l’approvazione"
        );
        if n.operation == "resolve_ticket" {
            let selection=format!("sysid:{}",effect.path.rsplit('/').next().unwrap_or_default());
            let (_, incident, _) = created_incident(r, n, Some(&selection))?;
            ensure!(effect.method == "PATCH" && effect.path == format!("/api/now/table/incident/{incident}"),
                "Anteprima risoluzione diversa dall’incidente creato in questa run; nessuna richiesta inviata");
        }
        if n.operation == "apply_ticket_batch" {
            crate::itsm_batch::execute(app, uid, r, n, c, &effect, &client).await?
        } else { apply(app, uid, r, n, c, &client, &effect).await? }
    };
    let mut artifacts = Vec::new();
    if n.operation == "apply_ticket_batch" || (["create_incident", "apply_ticket_plan", "resolve_ticket"].contains(&n.operation.as_str())
        && value["number"].as_str().is_some_and(|number| number.starts_with("INC"))) {
        ensure!(!n.id.is_empty() && n.id.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)), "ID nodo non valido per la ricevuta");
        let relative = format!("output/servicenow-{}.json", n.id);
        let path = Path::new(&r.workspace).join(&relative);
        let root = std::fs::canonicalize(&r.workspace)?;
        let output = root.join("output");
        std::fs::create_dir_all(&output)?;
        ensure!(std::fs::canonicalize(&output)?.starts_with(&root), "Cartella ricevuta fuori dal workspace");
        ensure!(!path.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()), "La ricevuta non può essere un link");
        let result = if n.operation == "apply_ticket_batch" { value.clone() } else {
            json!({"sys_id":value["sys_id"],"number":value["number"],"state":value["state"],"correlation_id":value["correlation_id"]})
        };
        let snapshot = json!({"source_run":r.id,"creation_node":n.id,"operation":n.operation,"instance":base(c)?,"result":result});
        std::fs::write(&path, serde_json::to_vec_pretty(&snapshot)?)?;
        agents::own(app, uid, &output)?;
        agents::own(app, uid, &path)?;
        artifacts.push(relative);
    }
    Ok(AgentResult {
        status: "completed".into(),
        decision: "pass".into(),
        artifacts,
        summary: agents::redact(
            &serde_json::to_string_pretty(&if n.operation=="check_connection" {json!({"result":value,"instance":base(c)?,"authentication":if c.authentication.is_empty(){"bearer"}else{&c.authentication}})} else {json!({"result":value})})?,
            &settings,
        ),
        ..Default::default()
    })
}
async fn apply(
    app: &App,
    uid: i64,
    r: &Run,
    n: &Node,
    c: &ConnectionConfig,
    client: &reqwest::Client,
    effect: &Effect,
) -> Result<Value> {
    if effect.method == "NONE" {
        return Ok(effect.body.clone());
    }
    let bytes = serde_json::to_vec(effect)?;
    let digest = crate::store::hash(&bytes);
    let dir = app
        .data
        .join("system/servicenow-effects")
        .join(uid.to_string())
        .join(&r.id);
    std::fs::create_dir_all(&dir)?;
    crate::store::private(&dir, true)?;
    let path = dir.join(format!("{}.json.enc", crate::store::hash(n.id.as_bytes())));
    if path.exists() {
        let receipt: Value = serde_json::from_slice(&app.store.decrypt(&std::fs::read(&path)?)?)?;
        ensure!(
            receipt["digest"] == digest,
            "Esiste una precedente azione per questo nodo: riconciliare prima di cambiare piano"
        );
        if receipt["status"] == "applied" {
            return Ok(receipt["result"].clone());
        }
        ensure!(
            !effect.correlation.is_empty(),
            "Esito precedente incerto: verificare ServiceNow prima di ripetere l’azione"
        );
    }
    if !effect.correlation.is_empty() {
        let mut query_url = reqwest::Url::parse("https://example.invalid")?;
        query_url
            .query_pairs_mut()
            .append_pair(
                "sysparm_query",
                &format!("correlation_id={}", effect.correlation),
            )
            .append_pair("sysparm_limit", "2");
        let query = query_url.query().unwrap_or_default();
        let existing = send(
            client,
            c,
            "GET",
            &format!("/api/now/table/incident?{query}"),
            None,
        )
        .await?;
        let records = existing
            .as_array()
            .context("Risposta ricerca incident non valida")?;
        ensure!(
            records.len() <= 1,
            "Più incident con la stessa correlazione: riconciliare manualmente"
        );
        if let Some(record) = records.first() {
            std::fs::write(
                &path,
                app.store.encrypt(&serde_json::to_vec(
                    &json!({"digest":digest,"status":"applied","result":record}),
                )?)?,
            )?;
            crate::store::private(&path, false)?;
            return Ok(record.clone());
        }
        ensure!(
            !path.exists(),
            "Esito precedente incerto e incident non ancora visibile: verificare la PDI prima di riprovare"
        );
    }
    // Durable write-ahead receipt: uncertain POSTs are never blindly replayed.
    let pending = app.store.encrypt(&serde_json::to_vec(
        &json!({"digest":digest,"status":"pending_reconciliation"}),
    )?)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    crate::store::private(&path, false)?;
    file.write_all(&pending)?;
    file.sync_all()?;
    let result = send(client, c, &effect.method, &effect.path, Some(&effect.body)).await?;
    if effect.body.get("state") == Some(&json!("6")) {
        ensure!(
            result["state"] == "6",
            "ServiceNow non ha confermato lo stato Resolved; verificare business rule e ricevuta"
        );
    }
    if effect.method == "POST" {
        ensure!(
            result.get("sys_id").is_some()
                || result.get("request_id").is_some()
                || result.get("request_number").is_some(),
            "ServiceNow non ha restituito un identificativo: riconciliare la ricevuta"
        );
    }
    std::fs::write(
        &path,
        app.store.encrypt(&serde_json::to_vec(
            &json!({"digest":digest,"status":"applied","result":result}),
        )?)?,
    )?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> ConnectionConfig {
        ConnectionConfig {
            base_url: "https://dev12345.service-now.com".into(),
            authentication: "basic".into(),
            username: "admin".into(),
            password: "secret-test-password".into(),
            catalog_item: "a".repeat(32),
            catalog_variables: r#"{"software":"Python"}"#.into(),
            ..Default::default()
        }
    }
    async fn fixture() -> (tempfile::TempDir, App, Run, Node) {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let mut r = crate::engine::create(
            &app,
            1,
            crate::presets::all().remove(0),
            "ITSM".into(),
            None,
        )
        .await
        .unwrap();
        r.id = "test-run".into();
        let n = Node {
            id: "apply".into(),
            kind: "Connection".into(),
            provider: "servicenow".into(),
            operation: "apply_ticket_plan".into(),
            location: "plan.json".into(),
            ..Default::default()
        };
        (dir, app, r, n)
    }
    fn write_plan(r: &Run, value: Value) {
        std::fs::write(
            Path::new(&r.workspace).join("plan.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }
    #[tokio::test]
    async fn missing_and_invalid_plan_errors_name_the_requested_file() {
        let (_dir, _app, r, mut n) = fixture().await;
        n.location = "output/plan.json".into();
        let error = prepare(&n, &r, &config()).unwrap_err().to_string();
        assert!(error.contains("File piano mancante: output/plan.json"), "{error}");
        let output = Path::new(&r.workspace).join("output");
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(output.join("incident-plan.json"), b"{}").unwrap();
        assert!(prepare(&n, &r, &config()).unwrap_err().to_string().contains("File piano mancante: output/plan.json"));
        std::fs::write(output.join("plan.json"), b"not json").unwrap();
        assert!(prepare(&n, &r, &config()).unwrap_err().to_string().contains("File piano JSON non valido: output/plan.json"));
        std::fs::write(output.join("plan.json"), br#"{"action":"answer_only","category":"Other","answer":"Example"}"#).unwrap();
        assert_eq!(prepare(&n, &r, &config()).unwrap().method, "NONE");
    }

    #[test]
    fn auth_is_backward_compatible_and_secrets_redacted() {
        let c = config();
        let request = auth(client().unwrap().get("https://example.invalid"), &c)
            .unwrap()
            .build()
            .unwrap();
        assert!(
            request.headers()["authorization"]
                .to_str()
                .unwrap()
                .starts_with("Basic ")
        );
        let old = ConnectionConfig {
            token: "old-oauth".into(),
            ..Default::default()
        };
        assert!(configured(&old));
        let mut next = c.clone();
        next.password.clear();
        merge(&mut next, &c);
        assert_eq!(next.password, c.password);
        next.password = "__CLEAR__".into();
        merge(&mut next, &c);
        assert!(next.password.is_empty());
        let mut settings = Settings::default();
        settings.connections.insert("servicenow".into(), c.clone());
        let header = request.headers()["authorization"].to_str().unwrap();
        assert!(
            !agents::redact(&format!("{} {header}", c.password), &settings).contains(&c.password)
        );
        assert!(
            !agents::redact(header, &settings).contains(header.strip_prefix("Basic ").unwrap())
        );
    }
    #[test]
    fn draft_checks_reuse_secrets_only_for_same_identity_and_never_mutate_saved() {
        let saved = config();
        let mut draft = saved.clone();
        draft.password.clear();
        assert_eq!(draft_config(draft.clone(), &saved).unwrap().password, saved.password);
        draft.base_url = "https://dev67890.service-now.com".into();
        assert!(draft_config(draft.clone(), &saved).is_err());
        draft.password = "new-draft-password".into();
        assert_eq!(draft_config(draft.clone(), &saved).unwrap().password, "new-draft-password");
        draft = saved.clone(); draft.username = "different-user".into(); draft.password.clear();
        assert!(draft_config(draft, &saved).is_err());
        draft = saved.clone(); draft.password = "__CLEAR__".into();
        assert!(draft_config(draft, &saved).is_err());
        assert_eq!(saved.password, "secret-test-password");
        let oauth = ConnectionConfig { authentication:"bearer".into(), base_url:saved.base_url.clone(), token:"saved-oauth".into(), ..Default::default() };
        let mut draft=oauth.clone(); draft.token.clear();
        assert_eq!(draft_config(draft.clone(), &oauth).unwrap().token, "saved-oauth");
        draft.authentication="basic".into(); draft.username="admin".into();
        assert!(draft_config(draft,&oauth).is_err());
    }
    #[test]
    fn config_validation_rejects_bad_hosts_and_catalog_json() {
        let mut c = config();
        validate_config(&c).unwrap();
        for url in [
            "http://dev12345.service-now.com",
            "https://example.com",
            "https://admin:secret@dev12345.service-now.com",
            "https://dev12345.service-now.com/api",
        ] {
            c.base_url = url.into();
            assert!(validate_config(&c).is_err());
        }
        c = config();
        c.catalog_variables = "[]".into();
        assert!(validate_config(&c).is_err());
        c = config();
        c.catalog_item = "invalid".into();
        assert!(validate_config(&c).is_err());
    }
    #[tokio::test]
    async fn incident_and_catalog_contracts_and_snapshot_digest() {
        let (_dir, _app, r, mut n) = fixture().await;
        let c = config();
        let p = json!({"action":"create_incident","category":"Incident","request_id":"demo-001","short_description":"VPN failure","description":"VPN down","answer":"KB-001: restart client"});
        write_plan(&r, p.clone());
        let effect = prepare(&n, &r, &c).unwrap();
        assert_eq!(effect.method, "POST");
        assert_eq!(effect.path, "/api/now/table/incident");
        assert!(effect.body.get("state").is_none());
        assert!(effect.correlation.starts_with("rightcommit:test-run:"));
        assert!(effect.correlation.len() <= 100);
        n.command = serde_json::to_string(&effect).unwrap();
        let mut settings = Settings::default();
        settings.connections.insert("servicenow".into(), c.clone());
        let before = crate::connectors::effect_digest(&n, &r, &settings);
        let mut changed = p;
        changed["description"] = json!("Changed description");
        write_plan(&r, changed);
        n.command = serde_json::to_string(&prepare(&n, &r, &c).unwrap()).unwrap();
        assert_ne!(before, crate::connectors::effect_digest(&n, &r, &settings));
        write_plan(&r, json!({"action":"order_catalog","category":"Request"}));
        let e = prepare(&n, &r, &c).unwrap();
        assert!(e.path.ends_with("/order_now"));
        assert_eq!(e.body["variables"]["software"], "Python");
        write_plan(&r, json!({"action":"order_catalog","category":"Other"}));
        assert!(prepare(&n, &r, &c).is_err());
        assert!(crate::connectors::is_write(&n));
    }
    fn resolution_creation(r: &mut Run, n: &mut Node) {
        n.target = "created".into();
        r.workflow.nodes.push(Node { id:"created".into(), kind:"Connection".into(), provider:"servicenow".into(), operation:"apply_ticket_plan".into(), ..Default::default() });
        r.workflow.edges.push(Edge { from:"created".into(), to:n.id.clone(), kind:"control".into(), outcome:String::new() });
        r.states.insert("created".into(), "completed".into());
        r.outputs.insert("created".into(), AgentResult { status:"completed".into(), summary:json!({"result":{"sys_id":"b".repeat(32),"number":"INC0010002","correlation_id":format!("rightcommit:{}:test",r.id)}}).to_string(), ..Default::default() });
    }
    #[tokio::test]
    async fn resolution_rejects_wrong_read_record_missing_and_ambiguous_creation() {
        let (_dir, _app, mut r, mut n) = fixture().await;
        n.operation="resolve_ticket".into();
        let mut p=json!({"sys_id":"a".repeat(32),"number":"INC0000060","verified":true,"close_code":"Solution provided","close_notes":"Demo verification"});
        write_plan(&r,p.clone());
        assert!(prepare(&n,&r,&config()).is_err());
        resolution_creation(&mut r,&mut n);
        let error=prepare(&n,&r,&config()).unwrap_err().to_string();
        assert!(error.contains("atteso INC0010002"),"{error}");
        p["sys_id"]=json!("b".repeat(32));p["number"]=json!("INC0010002");write_plan(&r,p.clone());
        assert!(prepare(&n,&r,&config()).is_ok());
        p["source_run"]=json!("another-run");write_plan(&r,p.clone());assert!(prepare(&n,&r,&config()).is_err());
        p.as_object_mut().unwrap().remove("source_run");write_plan(&r,p);
        n.target="read-check".into();assert!(prepare(&n,&r,&config()).is_err());
        n.target.clear();
        let mut source=r.workflow.nodes.iter().find(|n|n.id=="created").unwrap().clone();source.id="created-two".into();r.workflow.nodes.push(source);
        r.workflow.edges.push(Edge { from:"created-two".into(),to:n.id.clone(),kind:"control".into(),outcome:String::new() });
        r.states.insert("created-two".into(),"completed".into());r.outputs.insert("created-two".into(),r.outputs["created"].clone());
        assert!(prepare(&n,&r,&config()).is_err());
        n.target="created".into();assert!(prepare(&n,&r,&config()).is_ok());
        r.states.insert("created".into(),"queued".into());assert!(prepare(&n,&r,&config()).is_err());
    }
    #[tokio::test]
    async fn batch_resolution_is_bound_to_the_selected_native_incident(){
        let (_dir,_app,mut r,mut n)=fixture().await;n.operation="resolve_ticket".into();resolution_creation(&mut r,&mut n);
        r.workflow.nodes.iter_mut().find(|n|n.id=="created").unwrap().operation="apply_ticket_batch".into();
        r.outputs.get_mut("created").unwrap().summary=json!({"result":{"namespace":"customer","items":[{"request_id":"vpn","action":"create_incident","status":"reused","result":{"sys_id":"b".repeat(32),"number":"INC0010002","correlation_id":"rightcommit:older-run:original"}}]}}).to_string();
        let mut p=json!({"request_id":"vpn","namespace":"customer","sys_id":"b".repeat(32),"number":"INC0010002","source_run":r.id,"creation_node":"created","verified":true,"close_code":"Solution provided","close_notes":"Verified human evidence"});write_plan(&r,p.clone());
        assert!(prepare(&n,&r,&config()).is_ok());p["request_id"]=json!("wrong");write_plan(&r,p.clone());assert!(prepare(&n,&r,&config()).is_err());
        p["request_id"]=json!("vpn");p["namespace"]=json!("other-customer");write_plan(&r,p);assert!(prepare(&n,&r,&config()).is_err());
    }
    #[tokio::test]
    async fn resolution_requires_verification_and_scoped_plan() {
        let (_dir, _app, mut r, mut n) = fixture().await;
        n.operation = "resolve_ticket".into();
        resolution_creation(&mut r, &mut n);
        let mut p = json!({"sys_id":"b".repeat(32),"verified":false,"close_code":"Solution provided","close_notes":"User confirmed VPN connectivity"});
        write_plan(&r, p.clone());
        assert!(prepare(&n, &r, &config()).is_err());
        p["verified"] = json!(true);
        write_plan(&r, p);
        let e = prepare(&n, &r, &config()).unwrap();
        assert_eq!(e.body["state"], "6");
        assert_eq!(e.method, "PATCH");
        n.location = "../../outside.json".into();
        assert!(prepare(&n, &r, &config()).is_err());
    }
    #[tokio::test]
    async fn settings_encrypted_and_uncertain_catalog_is_not_replayed() {
        let (_dir, app, r, n) = fixture().await;
        let c = config();
        let mut settings = Settings::default();
        settings.connections.insert("servicenow".into(), c.clone());
        app.store.save_settings(1, &settings).unwrap();
        assert!(app.store.settings(2).unwrap().connections.is_empty());
        let blob: Vec<u8> = app
            .store
            .db
            .lock()
            .unwrap()
            .query_row("SELECT blob FROM settings WHERE user_id=1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(!String::from_utf8_lossy(&blob).contains(&c.password));
        write_plan(&r, json!({"action":"order_catalog","category":"Request"}));
        let effect = prepare(&n, &r, &c).unwrap();
        let dir = app.data.join("system/servicenow-effects/1/test-run");
        std::fs::create_dir_all(&dir).unwrap();
        let digest = crate::store::hash(&serde_json::to_vec(&effect).unwrap());
        std::fs::write(
            dir.join(format!("{}.json.enc", crate::store::hash(n.id.as_bytes()))),
            app.store
                .encrypt(
                    &serde_json::to_vec(
                        &json!({"digest":digest,"status":"pending_reconciliation"}),
                    )
                    .unwrap(),
                )
                .unwrap(),
        )
        .unwrap();
        let error = apply(&app, 1, &r, &n, &c, &client().unwrap(), &effect)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("incerto"));
    }
}

#[cfg(test)]
mod client_credentials_tests {
    use super::*;
    use axum::{Router,routing::{get,post},extract::State};
    use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
    #[derive(Default)]struct Mock{tokens:AtomicUsize,reads:AtomicUsize,writes:AtomicUsize}
    async fn issue(State(s):State<Arc<Mock>>)->Json<Value>{let n=s.tokens.fetch_add(1,Ordering::SeqCst)+1;Json(json!({"access_token":format!("oauth-{n}"),"token_type":"Bearer","expires_in":3600}))}
    async fn read(State(s):State<Arc<Mock>>,headers:axum::http::HeaderMap)->(axum::http::StatusCode,Json<Value>){
        let n=s.reads.fetch_add(1,Ordering::SeqCst);assert!(headers["authorization"].to_str().unwrap().starts_with("Bearer oauth-"));
        if n==0 {(axum::http::StatusCode::UNAUTHORIZED,Json(json!({})))}else{(axum::http::StatusCode::OK,Json(json!({"result":[]})))}
    }
    async fn write(State(s):State<Arc<Mock>>)->axum::http::StatusCode{s.writes.fetch_add(1,Ordering::SeqCst);axum::http::StatusCode::UNAUTHORIZED}
    #[tokio::test]
    async fn unauthorized_reads_renew_once_but_writes_are_never_replayed(){
        let s=Arc::new(Mock::default());let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let origin=format!("http://{}",listener.local_addr().unwrap());
        let router=Router::new().route("/oauth_token.do",post(issue)).route("/record",get(read).post(write)).with_state(s.clone());let server=tokio::spawn(async move{axum::serve(listener,router).await.unwrap();});
        let c=ConnectionConfig{authentication:"client_credentials".into(),client_id:"read-test".into(),client_secret:"test-secret".into(),..Default::default()};let client=client().unwrap();
        assert_eq!(send_at(&client,&c,"GET","/record",None,&origin).await.unwrap(),json!([]));assert_eq!(s.tokens.load(Ordering::SeqCst),2);assert_eq!(s.reads.load(Ordering::SeqCst),2);
        assert!(send_at(&client,&c,"POST","/record",Some(&json!({})),&origin).await.unwrap_err().to_string().contains("401"));assert_eq!(s.writes.load(Ordering::SeqCst),1);
        send_at(&client,&c,"GET","/record",None,&origin).await.unwrap();assert_eq!(s.tokens.load(Ordering::SeqCst),3);
        server.abort();
    }
    #[test]
    fn oauth_drafts_only_reuse_secrets_for_same_client_and_instance(){
        let saved=ConnectionConfig{authentication:"client_credentials".into(),base_url:"https://dev12345.service-now.com".into(),client_id:"client-one".into(),client_secret:"oauth-private-secret".into(),..Default::default()};
        let mut draft=saved.clone();draft.client_secret.clear();assert_eq!(draft_config(draft.clone(),&saved).unwrap().client_secret,saved.client_secret);
        draft.client_id="different".into();assert!(draft_config(draft.clone(),&saved).is_err());draft=saved.clone();draft.client_secret.clear();draft.base_url="https://dev67890.service-now.com".into();assert!(draft_config(draft,&saved).is_err());
        let mut c=saved.clone();c.client_id="changed".into();c.client_secret.clear();merge(&mut c,&saved);assert!(c.client_secret.is_empty());
        c=saved.clone();c.client_secret="__CLEAR__".into();merge(&mut c,&saved);assert!(!configured(&c));
        let mut settings=Settings::default();settings.connections.insert("servicenow".into(),saved.clone());assert!(!agents::redact(&saved.client_secret,&settings).contains(&saved.client_secret));
        assert!(validate_config(&saved).is_ok());
    }
}
