//! In-memory Client Credentials tokens. Secrets never enter settings responses or agent context.
use crate::model::ConnectionConfig;
use anyhow::{Context,Result,ensure,bail};
use serde_json::Value;
use std::{collections::HashMap,sync::{Arc,OnceLock},time::{Duration,Instant}};
use tokio::sync::Mutex;
struct Token { value:String, usable_until:Instant }
type Entry=Arc<Mutex<Option<Token>>>;
static TOKENS:OnceLock<Mutex<HashMap<String,Entry>>>=OnceLock::new();
fn key(c:&ConnectionConfig,origin:&str)->String {
    crate::store::hash(serde_json::to_vec(&(origin,&c.client_id,&c.client_secret,&c.oauth_scope)).unwrap())
}
async fn entry(c:&ConnectionConfig,origin:&str)->Entry {
    let mut cache=TOKENS.get_or_init(Default::default).lock().await;
    let id=key(c,origin);
    if let Some(entry)=cache.get(&id) {return entry.clone()}
    if cache.len()>=128 {cache.retain(|_,v|Arc::strong_count(v)>1);}
    let entry=Arc::new(Mutex::new(None));
    if cache.len()<128 {cache.insert(id,entry.clone());}
    entry
}
pub(crate) async fn invalidate(c:&ConnectionConfig,origin:&str,rejected:&str) {
    let entry=entry(c,origin).await;
    let mut cached=entry.lock().await;
    if cached.as_ref().is_some_and(|token|token.value==rejected) {*cached=None;}
}
pub(crate) async fn token(client:&reqwest::Client,c:&ConnectionConfig,origin:&str)->Result<String> {
    let entry=entry(c,origin).await;
    let mut cached=entry.lock().await;
    if let Some(token)=cached.as_ref().filter(|t|t.usable_until>Instant::now()) {return Ok(token.value.clone())}
    let mut form=vec![("grant_type","client_credentials"),("client_id",c.client_id.as_str()),("client_secret",c.client_secret.as_str())];
    if !c.oauth_scope.trim().is_empty(){form.push(("scope",c.oauth_scope.trim()));}
    let response=client.post(format!("{origin}/oauth_token.do")).form(&form).header("Accept","application/json").send().await
        .context("Endpoint OAuth ServiceNow non raggiungibile")?;
    let status=response.status();
    // Never expose response bodies: providers may reflect credentials or token material.
    if !status.is_success() {
        let hint=match status.as_u16(){
            400|401=>"controlla Client ID, Client Secret, grant Client Credentials e OAuth application user",
            403=>"controlla utente applicativo, scope e permessi OAuth",
            _=>"controlla disponibilità della PDI e log OAuth",
        };
        bail!("OAuth ServiceNow HTTP {status}: {hint}");
    }
    let response=response.bytes().await.context("Risposta OAuth ServiceNow non leggibile")?;
    ensure!(response.len()<=128_000,"Risposta OAuth ServiceNow troppo grande");
    let value:Value=serde_json::from_slice(&response).map_err(|_|anyhow::anyhow!("Risposta OAuth ServiceNow non valida"))?;
    let token=value["access_token"].as_str().filter(|v|!v.is_empty()&&v.len()<=65_536&&!v.chars().any(|c|c.is_control())).context("Risposta OAuth ServiceNow senza access_token valido")?;
    ensure!(value["token_type"].as_str().unwrap_or("Bearer").eq_ignore_ascii_case("bearer"),"Tipo di token OAuth ServiceNow non supportato");
    let seconds=value["expires_in"].as_u64().or_else(||value["expires_in"].as_str().and_then(|s|s.parse().ok())).filter(|v|*v>0&&*v<=u32::MAX as u64).context("Risposta OAuth ServiceNow senza scadenza valida (expires_in)")?;
    let lead=(seconds/10).clamp(1,60);
    *cached=Some(Token{value:token.to_owned(),usable_until:Instant::now()+Duration::from_secs(seconds.saturating_sub(lead))});
    Ok(token.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router,Json,Form,extract::State,routing::post};
    use std::sync::atomic::{AtomicUsize,Ordering};
    #[derive(Default)]
    struct Mock {calls:AtomicUsize, forms:Mutex<Vec<HashMap<String,String>>>, mode:AtomicUsize}
    async fn issue(State(state):State<Arc<Mock>>,Form(form):Form<HashMap<String,String>>)->(axum::http::StatusCode,Json<Value>){
        state.forms.lock().await.push(form);
        let n=state.calls.fetch_add(1,Ordering::SeqCst)+1;
        match state.mode.load(Ordering::SeqCst) {
            1=>(axum::http::StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"invalid_client","error_description":"reflected-secret"}))),
            2=>(axum::http::StatusCode::OK,Json(serde_json::json!({"access_token":"do-not-print-this","token_type":"Bearer"}))),
            _=>(axum::http::StatusCode::OK,Json(serde_json::json!({"access_token":format!("token-{n}"),"token_type":"Bearer","expires_in":"3600"}))),
        }
    }
    async fn fixture()->(Arc<Mock>,String,reqwest::Client,ConnectionConfig,tokio::task::JoinHandle<()>) {
        let state=Arc::new(Mock::default());
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin=format!("http://{}",listener.local_addr().unwrap());
        let app=Router::new().route("/oauth_token.do",post(issue)).with_state(state.clone());
        let server=tokio::spawn(async move{axum::serve(listener,app).await.unwrap();});
        let config=ConnectionConfig{authentication:"client_credentials".into(),client_id:"test-client".into(),client_secret:"secret + &=value".into(),oauth_scope:"useraccount read".into(),..Default::default()};
        let client=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap();
        (state,origin,client,config,server)
    }
    #[tokio::test]
    async fn concurrent_requests_share_one_token_and_encode_credentials() {
        let(s,url,client,c,server)=fixture().await;
        let results=futures::future::join_all((0..12).map(|_|token(&client,&c,&url))).await;
        assert!(results.into_iter().all(|r|r.unwrap()=="token-1"));
        assert_eq!(s.calls.load(Ordering::SeqCst),1);
        let forms=s.forms.lock().await;assert_eq!(forms[0]["grant_type"],"client_credentials");assert_eq!(forms[0]["client_secret"],c.client_secret);assert_eq!(forms[0]["scope"],c.oauth_scope);
        server.abort();
    }
    #[tokio::test]
    async fn expiry_rotation_and_scope_change_require_fresh_tokens() {
        let(s,url,client,c,server)=fixture().await;
        assert_eq!(token(&client,&c,&url).await.unwrap(),"token-1");
        entry(&c,&url).await.lock().await.as_mut().unwrap().usable_until=Instant::now();
        assert_eq!(token(&client,&c,&url).await.unwrap(),"token-2");
        invalidate(&c,&url,"token-1").await; // A delayed rejection must not evict a newer token.
        assert_eq!(token(&client,&c,&url).await.unwrap(),"token-2");
        let mut rotated=c.clone();rotated.client_secret="rotated-secret".into();assert_eq!(token(&client,&rotated,&url).await.unwrap(),"token-3");
        rotated.oauth_scope="".into();assert_eq!(token(&client,&rotated,&url).await.unwrap(),"token-4");
        assert!(!s.forms.lock().await.last().unwrap().contains_key("scope"));
        server.abort();
    }
    #[tokio::test]
    async fn errors_do_not_echo_secrets_and_missing_expiry_is_rejected() {
        let(s,url,client,c,server)=fixture().await;s.mode.store(1,Ordering::SeqCst);
        let error=token(&client,&c,&url).await.unwrap_err().to_string();assert!(error.contains("400"));assert!(!error.contains("reflected-secret"));assert!(!error.contains(&c.client_secret));
        s.mode.store(2,Ordering::SeqCst);let error=token(&client,&c,&url).await.unwrap_err().to_string();assert!(error.contains("expires_in"));assert!(!error.contains("do-not-print-this"));
        server.abort();
    }
}
