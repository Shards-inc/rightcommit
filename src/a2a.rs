//! A2A 1.0 JSON-RPC adapter. Published workflows keep a private, frozen definition.
use crate::{
    App,
    auth::{ApiError, ApiResult},
    engine,
    model::*,
};
use anyhow::{Context, Result, bail, ensure};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{sync::LazyLock, time::Duration};
static SEND_LOCK: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Connection {
    pub id: String,
    pub name: String,
    pub url: String,
    pub token: String,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Publication {
    pub enabled: bool,
    pub name: String,
    pub description: String,
    pub base_url: String,
    pub outputs: Vec<String>,
    pub token_hash: String,
    pub workflow: Option<Workflow>,
}
fn load<T: serde::de::DeserializeOwned + Default>(
    a: &App,
    uid: i64,
    kind: &str,
    id: &str,
) -> Result<T> {
    let blob: Option<Vec<u8>> = a
        .store
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT blob FROM a2a_configs WHERE user_id=? AND kind=? AND id=?",
            params![uid, kind, id],
            |r| r.get(0),
        )
        .optional()?;
    match blob {
        Some(b) => Ok(serde_json::from_slice(&a.store.decrypt(&b)?)?),
        None => Ok(T::default()),
    }
}
fn save<T: Serialize>(a: &App, uid: i64, kind: &str, id: &str, v: &T) -> Result<()> {
    let blob = a.store.encrypt(&serde_json::to_vec(v)?)?;
    a.store.db.lock().unwrap().execute("INSERT INTO a2a_configs VALUES(?,?,?,?) ON CONFLICT(user_id,kind,id) DO UPDATE SET blob=excluded.blob",params![uid,kind,id,blob])?;
    Ok(())
}
fn url(s: &str) -> Result<reqwest::Url> {
    let u = reqwest::Url::parse(s)?;
    ensure!(
        ["http", "https"].contains(&u.scheme())
            && u.host_str().is_some()
            && u.username().is_empty()
            && u.password().is_none()
            && u.fragment().is_none()
            && u.query().is_none(),
        "URL HTTP(S) senza credenziali o parametri richiesto"
    );
    Ok(u)
}
pub fn validate_connection(a: &App, uid: i64, id: &str) -> Result<()> {
    let c: Connection = load(a, uid, "connection", id)?;
    ensure!(!c.id.is_empty(), "Connessione A2A personale inesistente");
    url(&c.url)?;
    Ok(())
}
fn http() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()?)
}
async fn bounded(res: reqwest::Response) -> Result<Value> {
    ensure!(
        res.status().is_success(),
        "Servizio A2A: HTTP {}",
        res.status()
    );
    ensure!(
        res.content_length().unwrap_or(0) <= 2 * 1024 * 1024,
        "Risposta A2A troppo grande"
    );
    let mut res = res;
    let mut bytes = Vec::new();
    while let Some(chunk) = res.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= 2 * 1024 * 1024,
            "Risposta A2A troppo grande"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(serde_json::from_slice(&bytes)?)
}
pub async fn connections(State(a): State<App>, u: User) -> ApiResult<Json<Value>> {
    let result: Result<Json<Value>> = async {
        let ids: Vec<String> = {
            let db = a.store.db.lock().unwrap();
            let mut q =
                db.prepare("SELECT id FROM a2a_configs WHERE user_id=? AND kind='connection'")?;
            q.query_map([u.id], |r| r.get(0))?
                .collect::<std::result::Result<_, _>>()?
        };
        let mut result = Vec::new();
        for id in ids {
            let c: Connection = load(&a, u.id, "connection", &id)?;
            result
                .push(json!({"id":c.id,"name":c.name,"url":c.url,"has_token":!c.token.is_empty()}));
        }
        Ok(Json(json!(result)))
    }
    .await;
    Ok(result?)
}
pub async fn save_connection(
    State(a): State<App>,
    u: User,
    Json(mut c): Json<Connection>,
) -> ApiResult<Json<Value>> {
    let result: Result<Json<Value>> = async {
        url(&c.url)?;
        ensure!(
            c.name.len() <= 160 && c.token.len() <= 8192,
            "Configurazione troppo grande"
        );
        if c.id.is_empty() {
            c.id = id()
        };
        ensure!(c.id.len() <= 80, "ID non valido");
        if c.token.is_empty() {
            c.token = load::<Connection>(&a, u.id, "connection", &c.id)?.token;
        }
        save(&a, u.id, "connection", &c.id, &c)?;
        Ok(Json(json!({"id":c.id})))
    }
    .await;
    Ok(result?)
}
async fn discover(c: &Connection) -> Result<Value> {
    url(&c.url)?;
    let card = bounded(http()?.get(&c.url).bearer_auth(&c.token).send().await?).await?;
    let interfaces = card["supportedInterfaces"]
        .as_array()
        .context("Agent Card A2A 1.0 richiesta")?;
    let interface = interfaces
        .iter()
        .find(|v| {
            v["protocolBinding"] == "JSONRPC"
                && v["protocolVersion"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("1.0"))
        })
        .context("Binding JSONRPC A2A 1.0 non disponibile")?;
    let endpoint = interface["url"].as_str().context("Endpoint assente")?;
    let endpoint = url(endpoint)?;
    let source = url(&c.url)?;
    ensure!(
        endpoint.origin() == source.origin(),
        "La Agent Card rimanda a un'origine diversa: configura un endpoint della stessa origine"
    );
    Ok(card)
}
pub async fn probe(
    State(a): State<App>,
    u: User,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(discover(&load(&a, u.id, "connection", &id)?).await?))
}
pub async fn publication(
    State(a): State<App>,
    u: User,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    a.store.workflow(u.id, &id)?;
    let p: Publication = load(&a, u.id, "publication", &id)?;
    Ok(Json(
        json!({"enabled":p.enabled,"name":p.name,"description":p.description,"base_url":p.base_url,"outputs":p.outputs,"has_token":!p.token_hash.is_empty(),"path":format!("/a2a/{}/{}",u.id,id)}),
    ))
}
pub async fn publish(
    State(a): State<App>,
    u: User,
    Path(wid): Path<String>,
    Json(v): Json<Value>,
) -> ApiResult<Json<Value>> {
    let result: Result<Json<Value>> = async {
        let _lock = SEND_LOCK.lock().await;
        let mut p: Publication = serde_json::from_value(v.clone())?;
        let old: Publication = load(&a, u.id, "publication", &wid)?;
        // Never accept hashes or definitions supplied by the browser.
        p.token_hash = old.token_hash;
        let mut token = None;
        if p.enabled {
            url(&p.base_url)?;
            let w = a.store.workflow(u.id, &wid)?;
            crate::subworkflows::validate_tree(&a, u.id, &w)?;
            ensure!(
                p.outputs
                    .iter()
                    .all(|id| w.nodes.iter().any(|n| &n.id == id)),
                "Output inesistente"
            );
            ensure!(
                !p.name.trim().is_empty() && p.name.len() <= 160 && p.description.len() <= 4096,
                "Nome o descrizione non validi"
            );
            if p.token_hash.is_empty() || v["rotate"] == true {
                let t = format!("{}{}", id(), id());
                p.token_hash = crate::store::hash(&t);
                token = Some(t);
            }
            p.workflow = Some(w);
        } else {
            p.workflow = None;
            p.token_hash.clear();
        }
        save(&a, u.id, "publication", &wid, &p)?;
        Ok(Json(json!({"saved":true,"token":token})))
    }
    .await;
    Ok(result?)
}
fn authorized(a: &App, uid: i64, wid: &str, h: &HeaderMap) -> Result<Publication> {
    crate::auth::ensure_active(a, uid)?;
    let p: Publication = load(a, uid, "publication", wid)?;
    ensure!(
        p.enabled && a.store.workflow(uid, wid).is_ok(),
        "Agente non disponibile"
    );
    let token = h
        .get("authorization")
        .and_then(|x| x.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .context("Accesso A2A richiesto")?;
    ensure!(
        !p.token_hash.is_empty() && crate::store::hash(token) == p.token_hash,
        "Accesso A2A negato"
    );
    Ok(p)
}
fn card_value(p: &Publication, uid: i64, wid: &str) -> Value {
    json!({"name":p.name,"description":p.description,"version":"1.0.0","supportedInterfaces":[{"url":format!("{}/a2a/{uid}/{wid}",p.base_url.trim_end_matches('/')),"protocolBinding":"JSONRPC","protocolVersion":"1.0"}],"capabilities":{"streaming":false,"pushNotifications":false},"defaultInputModes":["text/plain"],"defaultOutputModes":["text/plain","application/json"],"securitySchemes":{"bearer":{"httpAuthSecurityScheme":{"scheme":"bearer"}}},"securityRequirements":[{"schemes":{"bearer":{"list":[]}}}],"skills":[{"id":wid,"name":p.name,"description":p.description,"tags":["workflow"]}]})
}
pub async fn card(
    State(a): State<App>,
    Path((uid, wid)): Path<(i64, String)>,
    h: HeaderMap,
) -> ApiResult<Json<Value>> {
    let p = authorized(&a, uid, &wid, &h)
        .map_err(|_| ApiError(StatusCode::UNAUTHORIZED, "Accesso A2A negato".into()))?;
    Ok(Json(card_value(&p, uid, &wid)))
}
fn text(message: &Value) -> Result<String> {
    ensure!(message["role"] == "ROLE_USER", "Ruolo ROLE_USER richiesto");
    let parts = message["parts"]
        .as_array()
        .context("Parti del messaggio richieste")?;
    ensure!(
        !parts.is_empty() && parts.iter().all(|p| p["text"].is_string()),
        "Sono accettati messaggi di testo"
    );
    let s = parts
        .iter()
        .map(|p| p["text"].as_str().unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    ensure!(
        !s.trim().is_empty() && s.len() <= 64000,
        "Messaggio vuoto o troppo grande"
    );
    Ok(s)
}
fn task_run(a: &App, uid: i64, wid: &str, tid: &str) -> Result<String> {
    a.store
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT run FROM a2a_tasks WHERE id=? AND user_id=? AND workflow=?",
            params![tid, uid, wid],
            |r| r.get(0),
        )
        .optional()?
        .context("Task non trovato")
}
fn attention(a: &App, uid: i64, mut r: Run) -> Result<Run> {
    for _ in 0..crate::subworkflows::MAX_DEPTH {
        if r.pending.is_some() {
            break;
        }
        let Some(call) = r
            .children
            .iter()
            .rev()
            .find(|c| c.node == r.current && Some(&c.attempt) == r.attempts.get(&c.node))
        else {
            break;
        };
        r = a.store.run(uid, &call.run)?;
    }
    Ok(r)
}
fn task_value(a: &App, uid: i64, tid: &str, rid: &str, _p: &Publication) -> Result<Value> {
    let r = a.store.run(uid, rid)?;
    let active = attention(a, uid, r.clone())?;
    let selected: String = a.store.db.lock().unwrap().query_row(
        "SELECT outputs FROM a2a_tasks WHERE id=? AND user_id=?",
        params![tid, uid],
        |r| r.get(0),
    )?;
    let outputs: Vec<String> = serde_json::from_str(&selected)?;
    let state = match r.status.as_str() {
        "completed" => "TASK_STATE_COMPLETED",
        "failed" | "interrupted" => "TASK_STATE_FAILED",
        _ if active
            .pending
            .as_ref()
            .is_some_and(|x| x.kind == "question") =>
        {
            "TASK_STATE_INPUT_REQUIRED"
        }
        _ => "TASK_STATE_WORKING",
    };
    let pending = active
        .pending
        .as_ref()
        .filter(|x| x.kind == "question")
        .map(|x| x.questions.join("\n"))
        .unwrap_or_else(|| {
            if active.pending.is_some() {
                "In attesa di approvazione locale".into()
            } else {
                String::new()
            }
        });
    let mut out = json!({"id":tid,"contextId":tid,"status":{"state":state}});
    if !pending.is_empty() {
        out["status"]["message"] =
            json!({"messageId":id(),"role":"ROLE_AGENT","parts":[{"text":pending}]});
    }
    if r.status == "completed" {
        out["artifacts"]=json!(outputs.iter().filter_map(|n|r.outputs.get(n).map(|v|json!({"artifactId":n,"name":r.workflow.node(n).map(|n|n.title.clone()).unwrap_or_default(),"parts":[{"text":v.report()}]}))).collect::<Vec<_>>());
    }
    Ok(out)
}
pub async fn rpc(
    State(a): State<App>,
    Path((uid, wid)): Path<(i64, String)>,
    h: HeaderMap,
    Json(v): Json<Value>,
) -> ApiResult<Json<Value>> {
    let p = authorized(&a, uid, &wid, &h)
        .map_err(|_| ApiError(StatusCode::UNAUTHORIZED, "Accesso A2A negato".into()))?;
    let request_id = v["id"].clone();
    let result:Result<Value>=async {
        ensure!(v["jsonrpc"]=="2.0" && !request_id.is_null(),"Richiesta JSON-RPC non valida");
        let method=v["method"].as_str().unwrap_or("");let params=&v["params"];
        match method {
            "SendMessage"=>{
                let _guard=SEND_LOCK.lock().await;
                let msg=&params["message"];let input=text(msg)?;
                let mid=msg["messageId"].as_str().filter(|s|!s.is_empty()&&s.len()<=160).context("messageId richiesto")?;
                if let Some(tid)=msg["taskId"].as_str() {
                    let rid=task_run(&a,uid,&wid,tid)?;let r=attention(&a,uid,a.store.run(uid,&rid)?)?;

                    let user=User{id:uid,email:format!("a2a:{wid}")};let already:bool=a.store.db.lock().unwrap().query_row("SELECT EXISTS(SELECT 1 FROM a2a_replies WHERE task=? AND message=?)",params![tid,mid],|r|r.get(0))?;
                    if !already {
                        ensure!(r.pending.as_ref().is_some_and(|x|x.kind=="question"),"Il task non attende chiarimenti dal client");
                        a.store.db.lock().unwrap().execute("INSERT INTO a2a_replies VALUES(?,?)",params![tid,mid])?;
                        engine::decide(&a,&user,&r.id,"answer",&input)?;engine::launch(a.clone(),uid,r.id.clone())?;
                    }
                    return Ok(json!({"task":task_value(&a,uid,tid,&rid,&p)?}));
                }
                ensure!(msg["contextId"].is_null(),"Per continuare un task usa taskId");
                let previous:Option<(String,String)>=a.store.db.lock().unwrap().query_row("SELECT id,run FROM a2a_tasks WHERE user_id=? AND workflow=? AND message=?",params![uid,wid,mid],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
                let (tid,rid)=if let Some(pair)=previous {ensure!(!pair.1.is_empty(),"Invio precedente interrotto: verificare le run locali");pair} else {
                    let tid=id();
                    a.store.db.lock().unwrap().execute("INSERT INTO a2a_tasks(id,user_id,workflow,run,message,outputs) VALUES(?,?,?,?,?,?)",params![tid,uid,wid,"",mid,serde_json::to_string(&p.outputs)?])?;
                    let r=engine::create(&a,uid,p.workflow.clone().context("Workflow non pubblicato")?,input,None).await?;
                    a.store.db.lock().unwrap().execute("UPDATE a2a_tasks SET run=? WHERE id=?",params![r.id,tid])?;
                    engine::launch(a.clone(),uid,r.id.clone())?;(tid,r.id)
                };
                Ok(json!({"task":task_value(&a,uid,&tid,&rid,&p)?}))
            },
            "GetTask"=>{let tid=params["id"].as_str().context("ID richiesto")?;let rid=task_run(&a,uid,&wid,tid)?;task_value(&a,uid,tid,&rid,&p)},
            "GetExtendedAgentCard"=>Ok(card_value(&p,uid,&wid)),
            _=>bail!("Metodo non supportato")
        }
    }.await;
    let result = if v["method"] == "SendMessage"
        && v["params"]["configuration"]["returnImmediately"] != true
    {
        match result {
            Ok(mut value) => {
                while value["task"]["status"]["state"] == "TASK_STATE_WORKING" {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    let tid = value["task"]["id"].as_str().unwrap_or("");
                    match task_run(&a, uid, &wid, tid)
                        .and_then(|rid| task_value(&a, uid, tid, &rid, &p))
                    {
                        Ok(t) => value["task"] = t,
                        Err(e) => {
                            return Ok(Json(
                                json!({"jsonrpc":"2.0","id":request_id,"error":{"code":-32603,"message":e.to_string()}}),
                            ));
                        }
                    }
                }
                Ok(value)
            }
            Err(e) => Err(e),
        }
    } else {
        result
    };
    Ok(Json(match result {
        Ok(result) => json!({"jsonrpc":"2.0","id":request_id,"result":result}),
        Err(e) => {
            json!({"jsonrpc":"2.0","id":request_id,"error":{"code":if v["method"]=="CancelTask"{-32002}else if !["SendMessage","GetTask","GetExtendedAgentCard"].contains(&v["method"].as_str().unwrap_or("")){-32601}else{-32602},"message":e.to_string()}})
        }
    }))
}
async fn remote(c: &Connection, endpoint: &str, method: &str, params: Value) -> Result<Value> {
    let v = bounded(
        http()?
            .post(endpoint)
            .bearer_auth(&c.token)
            .header("A2A-Version", "1.0")
            .json(&json!({"jsonrpc":"2.0","id":id(),"method":method,"params":params}))
            .send()
            .await?,
    )
    .await?;
    ensure!(
        v.get("error").is_none(),
        "A2A: {}",
        v["error"]["message"].as_str().unwrap_or("errore remoto")
    );
    v.get("result")
        .cloned()
        .context("Risposta JSON-RPC non valida")
}
fn complete(r: &mut Run, n: &Node, summary: String) -> AgentResult {
    let key = format!("a2a:{}", n.id);
    if let Some(tid) = r.context.remove(&key) {
        r.context.insert(
            format!("{key}:history:{}", r.attempts.get(&n.id).unwrap_or(&0)),
            tid,
        );
    }
    r.context.remove(&format!("{key}:sent"));
    r.context.remove(&format!("{key}:endpoint"));
    AgentResult {
        status: "completed".into(),
        summary,
        ..Default::default()
    }
}
pub async fn execute(a: &App, uid: i64, r: &mut Run, n: &Node) -> Result<AgentResult> {
    let c: Connection = load(a, uid, "connection", &n.a2a_id)?;
    let key = format!("a2a:{}", n.id);
    let endpoint_key = format!("{key}:endpoint");
    let endpoint = if let Some(s) = r.context.get(&endpoint_key) {
        ensure!(
            url(s)?.origin() == url(&c.url)?.origin(),
            "Connessione A2A modificata: ripristina la connessione originale per riprendere il task"
        );
        s.clone()
    } else {
        let card = discover(&c).await?;
        let endpoint = card["supportedInterfaces"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| {
                x["protocolBinding"] == "JSONRPC"
                    && x["protocolVersion"]
                        .as_str()
                        .is_some_and(|s| s.starts_with("1.0"))
            })
            .unwrap()["url"]
            .as_str()
            .unwrap()
            .to_string();
        r.context.insert(endpoint_key, endpoint.clone());
        endpoint
    };
    let answer = r.context.remove(&format!("answer:{}", n.id));
    let mut task = if let Some(tid) = r.context.get(&key).cloned() {
        if let Some(answer) = answer {
            let answer = serde_json::from_str::<Value>(&answer)
                .ok()
                .and_then(|v| v["answer"].as_str().map(str::to_owned))
                .unwrap_or(answer);
            a.store.save_run(uid, r)?;
            remote(&c,&endpoint,"SendMessage",json!({"message":{"messageId":id(),"taskId":tid,"role":"ROLE_USER","parts":[{"text":answer}]},"configuration":{"returnImmediately":true}})).await?["task"].clone()
        } else {
            remote(&c, &endpoint, "GetTask", json!({"id":tid})).await?
        }
    } else {
        ensure!(
            !r.context.contains_key(&format!("{key}:sent")),
            "Invio A2A precedente senza conferma. Verifica il servizio remoto prima di creare una nuova run."
        );
        let mid = id();
        r.context.insert(format!("{key}:sent"), mid.clone());
        a.store.save_run(uid, r)?;
        let response=remote(&c,&endpoint,"SendMessage",json!({"message":{"messageId":mid,"role":"ROLE_USER","parts":[{"text":format!("{}\n{}\n{}",n.prompt,r.input,r.last_node.as_ref().and_then(|id|r.outputs.get(id)).map(|o|o.report()).unwrap_or_default())}]},"configuration":{"returnImmediately":true}})).await?;
        if let Some(message) = response.get("message") {
            return Ok(complete(r, n, serde_json::to_string(message)?));
        }
        let task = response["task"].clone();
        let tid = task["id"].as_str().context("Task remoto senza ID")?;
        r.context.insert(key.clone(), tid.into());
        a.store.save_run(uid, r)?;
        task
    };
    let until = tokio::time::Instant::now()
        + Duration::from_secs(a.store.settings(uid)?.task_timeout_secs.max(30));
    loop {
        match task["status"]["state"].as_str().unwrap_or("") {
            "TASK_STATE_COMPLETED" => {
                return Ok(complete(r, n, serde_json::to_string_pretty(&task)?));
            }
            "TASK_STATE_INPUT_REQUIRED" => {
                return Ok(AgentResult {
                    status: "needs_input".into(),
                    questions: vec![
                        task["status"]["message"]["parts"]
                            .as_array()
                            .map(|ps| {
                                ps.iter()
                                    .filter_map(|p| p["text"].as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            })
                            .unwrap_or_else(|| "Il servizio A2A richiede un chiarimento".into()),
                    ],
                    ..Default::default()
                });
            }
            "TASK_STATE_FAILED"
            | "TASK_STATE_CANCELED"
            | "TASK_STATE_REJECTED"
            | "TASK_STATE_AUTH_REQUIRED" => bail!(
                "Task A2A terminato o accesso richiesto: {}",
                task["status"]["state"]
            ),
            "TASK_STATE_SUBMITTED" | "TASK_STATE_WORKING" => {}
            _ => bail!("Stato A2A non riconosciuto"),
        }
        ensure!(
            tokio::time::Instant::now() < until,
            "Timeout A2A: il task remoto resta disponibile al retry"
        );
        if a.store.run(uid, &r.id)?.pause_requested {
            return Ok(AgentResult {
                status: "paused".into(),
                ..Default::default()
            });
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
        task = remote(&c, &endpoint, "GetTask", json!({"id":r.context[&key]})).await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, App, Workflow) {
        let dir = tempfile::tempdir().unwrap();
        let a = crate::tests::app(&std::fs::canonicalize(dir.path()).unwrap());
        let mut w = crate::presets::all().remove(0);
        w.id = "a2a-test".into();
        w.entry = "read".into();
        w.nodes = vec![
            Node {
                id: "source".into(),
                kind: "Source".into(),
                title: "Source".into(),
                source_type: "example".into(),
                ..Default::default()
            },
            Node {
                id: "read".into(),
                kind: "Connection".into(),
                title: "Read".into(),
                provider: "local".into(),
                operation: "read_ticket".into(),
                location: "Cargo.toml".into(),
                ..Default::default()
            },
        ];
        w.edges = vec![Edge {
            from: "source".into(),
            to: "read".into(),
            kind: "context".into(),
            outcome: String::new(),
        }];
        a.store.save_workflow(1, &w).unwrap();
        (dir, a, w)
    }
    fn headers(token: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("authorization", format!("Bearer {token}").parse().unwrap());
        h
    }
    async fn published(a: &App, w: &Workflow) -> String {
        publish(State(a.clone()),User{id:1,email:"owner".into()},Path(w.id.clone()),Json(json!({"enabled":true,"name":"Read agent","base_url":"http://localhost:8080","outputs":["read"]}))).await.unwrap().0["token"].as_str().unwrap().into()
    }
    #[tokio::test]
    async fn publication_auth_rotation_and_private_credentials() {
        let (_dir, a, w) = fixture();
        let token = published(&a, &w).await;
        assert!(authorized(&a, 1, &w.id, &HeaderMap::new()).is_err());
        assert!(authorized(&a, 2, &w.id, &headers(&token)).is_err());
        assert!(authorized(&a, 1, &w.id, &headers(&token)).is_ok());
        let p = publication(
            State(a.clone()),
            User {
                id: 1,
                email: "owner".into(),
            },
            Path(w.id.clone()),
        )
        .await
        .unwrap()
        .0;
        assert!(!p.to_string().contains(&token));
        assert!(p.get("workflow").is_none());
        let new=publish(State(a.clone()),User{id:1,email:"owner".into()},Path(w.id.clone()),Json(json!({"enabled":true,"name":"Read agent","base_url":"http://localhost:8080","outputs":["read"],"rotate":true}))).await.unwrap().0;
        assert!(authorized(&a, 1, &w.id, &headers(&token)).is_err());
        assert!(authorized(&a, 1, &w.id, &headers(new["token"].as_str().unwrap())).is_ok());
        let _ = publish(
            State(a.clone()),
            User {
                id: 1,
                email: "owner".into(),
            },
            Path(w.id.clone()),
            Json(json!({"enabled":false})),
        )
        .await
        .unwrap();
        assert!(authorized(&a, 1, &w.id, &headers(new["token"].as_str().unwrap())).is_err());
    }
    #[tokio::test]
    async fn server_idempotency_frozen_outputs_and_tenant_isolation() {
        let (_dir, a, w) = fixture();
        let token = published(&a, &w).await;
        let req = json!({"jsonrpc":"2.0","id":1,"method":"SendMessage","params":{"message":{"messageId":"same-message","role":"ROLE_USER","parts":[{"text":"Read package"}]},"configuration":{"returnImmediately":true}}});
        let first = rpc(
            State(a.clone()),
            Path((1, w.id.clone())),
            headers(&token),
            Json(req.clone()),
        )
        .await
        .unwrap()
        .0;
        assert!(first.get("error").is_none(), "{first}");
        let tid = first["result"]["task"]["id"].as_str().unwrap();
        let second = rpc(
            State(a.clone()),
            Path((1, w.id.clone())),
            headers(&token),
            Json(req),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(
            first["result"]["task"]["id"],
            second["result"]["task"]["id"]
        );
        assert!(task_run(&a, 2, &w.id, tid).is_err());
        assert!(task_run(&a, 1, "other", tid).is_err());
        let rid = task_run(&a, 1, &w.id, tid).unwrap();
        for _ in 0..100 {
            if !a.busy.lock().unwrap().contains(&rid) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let r = a.store.run(1, &rid).unwrap();
        assert_eq!(r.status, "completed", "{:?}", r.error);
        let mut p: Publication = load(&a, 1, "publication", &w.id).unwrap();
        p.outputs.clear();
        let t = task_value(&a, 1, tid, &rid, &p).unwrap();
        assert_eq!(t["artifacts"].as_array().unwrap().len(), 1);
        assert!(t.to_string().contains("[package]"));
        let req = json!({"jsonrpc":"2.0","id":2,"method":"SendMessage","params":{"message":{"messageId":"continue","taskId":tid,"role":"ROLE_USER","parts":[{"text":"approve"}]}}});
        let result = rpc(State(a), Path((1, w.id)), headers(&token), Json(req))
            .await
            .unwrap()
            .0;
        assert!(result.get("error").is_some());
    }
    #[tokio::test]
    async fn real_http_client_calls_published_workflow() {
        let (_dir, a, w) = fixture();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let result = publish(
            State(a.clone()),
            User {
                id: 1,
                email: "owner".into(),
            },
            Path(w.id.clone()),
            Json(json!({"enabled":true,"name":"Read agent","base_url":base,"outputs":["read"]})),
        )
        .await
        .unwrap()
        .0;
        let token = result["token"].as_str().unwrap();
        let server = tokio::spawn({
            let a = a.clone();
            async move { axum::serve(listener, crate::api::router(a)).await.unwrap() }
        });
        let c = Connection {
            id: "remote".into(),
            name: "Remote".into(),
            url: format!("{base}/a2a/1/{}/.well-known/agent-card.json", w.id),
            token: token.into(),
        };
        save(&a, 1, "connection", "remote", &c).unwrap();
        let mut local = engine::create(&a, 1, w.clone(), "Input".into(), None)
            .await
            .unwrap();
        let node = Node {
            id: "remote-node".into(),
            kind: "A2A".into(),
            a2a_id: "remote".into(),
            prompt: "Read".into(),
            ..Default::default()
        };
        let output = execute(&a, 1, &mut local, &node).await.unwrap();
        assert_eq!(output.status, "completed");
        assert!(output.summary.contains("[package]"));
        assert!(
            local
                .context
                .keys()
                .any(|k| k.starts_with("a2a:remote-node:history:"))
        );
        assert!(!local.context.contains_key("a2a:remote-node"));
        let no_auth = http()
            .unwrap()
            .post(format!("{base}/a2a/1/{}", w.id))
            .json(&json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(no_auth.status(), 401);
        server.abort();
    }
    #[tokio::test]
    async fn client_clarification_resumes_remote_task_without_resubmission() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let (_dir, a, w) = fixture();
        let count = Arc::new(AtomicUsize::new(0));
        let responses = Arc::new(AtomicUsize::new(0));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let endpoint = format!("{base}/rpc");
        let card = json!({"supportedInterfaces":[{"url":endpoint,"protocolBinding":"JSONRPC","protocolVersion":"1.0"}]});
        let route=axum::Router::new().route("/card",axum::routing::get(move||{let card=card.clone();async move{Json(card)}})).route("/rpc",axum::routing::post({let count=count.clone();let responses=responses.clone();move|Json(v):Json<Value>|{let count=count.clone();let responses=responses.clone();async move{
            let continued=v["params"]["message"]["taskId"]=="remote-task";
            if v["method"]=="SendMessage" {if continued{responses.fetch_add(1,Ordering::SeqCst);}else{count.fetch_add(1,Ordering::SeqCst);}}
            let task=json!({"id":"remote-task","contextId":"remote-context","status":{"state":if continued{"TASK_STATE_COMPLETED"}else{"TASK_STATE_INPUT_REQUIRED"},"message":{"parts":[{"text":"Quale progetto?"}]}}});
            Json(json!({"jsonrpc":"2.0","id":v["id"],"result":if v["method"]=="GetTask"{task}else{json!({"task":task})}}))
        }}}));
        let server = tokio::spawn(async move { axum::serve(listener, route).await.unwrap() });
        save(
            &a,
            1,
            "connection",
            "mock",
            &Connection {
                id: "mock".into(),
                url: format!("{base}/card"),
                ..Default::default()
            },
        )
        .unwrap();
        let mut r = engine::create(&a, 1, w, "Input".into(), None)
            .await
            .unwrap();
        let n = Node {
            id: "a2a".into(),
            a2a_id: "mock".into(),
            kind: "A2A".into(),
            ..Default::default()
        };
        let first = execute(&a, 1, &mut r, &n).await.unwrap();
        assert_eq!(first.status, "needs_input");
        assert_eq!(first.questions, vec!["Quale progetto?"]);
        // Load persisted state as after a server restart, then supply the clarification.
        let mut resumed = a.store.run(1, &r.id).unwrap();
        resumed
            .context
            .insert("answer:a2a".into(), "Progetto alpha".into());
        a.store.save_run(1, &resumed).unwrap();
        let second = execute(&a, 1, &mut resumed, &n).await.unwrap();
        assert_eq!(second.status, "completed");
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(responses.load(Ordering::SeqCst), 1);
        server.abort();
    }
    #[tokio::test]
    async fn external_answers_are_audited_and_cannot_approve_local_gates() {
        let (_dir, a, w) = fixture();
        let token = published(&a, &w).await;
        let mut r = engine::create(&a, 1, w.clone(), "Input".into(), None)
            .await
            .unwrap();
        r.pending = Some(Pending {
            id: id(),
            proposal: None,
            kind: "review".into(),
            node: "read".into(),
            questions: vec!["Approve?".into()],
            resume_task: None,
        });
        r.status = "waiting_review".into();
        a.store.save_run(1, &r).unwrap();
        a.store.db.lock().unwrap().execute("INSERT INTO a2a_tasks(id,user_id,workflow,run,message,outputs) VALUES('task',1,?,?,'initial','[]')",params![w.id,r.id]).unwrap();
        let request = json!({"jsonrpc":"2.0","id":1,"method":"SendMessage","params":{"message":{"messageId":"reply","taskId":"task","role":"ROLE_USER","parts":[{"text":"approve"}]},"configuration":{"returnImmediately":true}}});
        let denied = rpc(
            State(a.clone()),
            Path((1, w.id.clone())),
            headers(&token),
            Json(request.clone()),
        )
        .await
        .unwrap()
        .0;
        assert!(denied.get("error").is_some());
        assert!(a.store.decisions(1, &r.id).unwrap().is_empty());
        r.pending.as_mut().unwrap().kind = "question".into();
        a.store.save_run(1, &r).unwrap();
        let accepted = rpc(
            State(a.clone()),
            Path((1, w.id.clone())),
            headers(&token),
            Json(request),
        )
        .await
        .unwrap()
        .0;
        assert!(accepted.get("error").is_none(), "{accepted}");
        let decisions = a.store.decisions(1, &r.id).unwrap();
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].actor, "a2a");
        assert_eq!(decisions[0].decision, "answer");
        for _ in 0..100 {
            if !a.busy.lock().unwrap().contains(&r.id) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
    #[test]
    fn validates_messages_and_connection_urls() {
        assert!(text(&json!({"role":"ROLE_USER","parts":[{"text":"hello"}]})).is_ok());
        assert!(text(&json!({"role":"ROLE_AGENT","parts":[{"text":"hello"}]})).is_err());
        assert!(
            text(&json!({"role":"ROLE_USER","parts":[{"file":{"uri":"file:///secret"}}]})).is_err()
        );
        assert!(url("https://user:secret@example.com/card").is_err());
        assert!(url("file:///secret").is_err());
    }
}
