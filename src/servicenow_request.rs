//! Read-only acquisition of a software/service requirement, configured by a Source file.
use crate::{model::*,servicenow,servicenow_config};
use anyhow::{Result,Context,ensure};
use serde_json::{Value,json};
use std::{future::Future,path::Path};
fn identifier(value:&str)->Result<()> {ensure!(!value.is_empty()&&value.len()<=80&&value.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'_'),"Nome tabella/campo ServiceNow non valido");Ok(())}
fn raw(value:&Value)->Option<&str>{value.as_str().or_else(||value["value"].as_str())}
fn query(path:&str,pairs:&[(&str,String)])->Result<String>{let mut u=reqwest::Url::parse("https://example.invalid")?;for(k,v)in pairs{u.query_pairs_mut().append_pair(k,v);}Ok(format!("{path}?{}",u.query().unwrap_or_default()))}
async fn acquire<F,Fut>(r:&Run,n:&Node,config:&Value,instance:Option<String>,transport:F)->Result<Value>
where F:Fn(String)->Fut,Fut:Future<Output=Result<Value>> {
    ensure!(config["schema"]=="rightcommit.servicenow.requirement.v1","Schema configurazione acquisizione non valido");servicenow_config::no_credentials(config)?;
    let source=&config["source"];let table=source["table"].as_str().unwrap_or("sc_req_item");identifier(table)?;
    let mode=source["mode"].as_str().context("source.mode deve essere fixture o servicenow")?;
    let (record,variables,variable_status)=if mode=="fixture" {
        let file=source["fixture"].as_str().context("Fixture non configurata")?;
        let fixture=servicenow_config::read_json(r,file,"fixture",1_000_000)?;
        ensure!(fixture["record"].is_object()&&fixture["variables"].is_object(),"Fixture deve contenere record e variables");
        (fixture["record"].clone(),fixture["variables"].clone(),"fixture".to_string())
    }else {
        ensure!(mode=="servicenow"&&instance.is_some(),"Modalità acquisizione non valida o connessione mancante");
        let sid=source["sys_id"].as_str().unwrap_or("");let number=source["number"].as_str().unwrap_or("");
        ensure!(sid.is_empty()^number.is_empty(),"Configura esattamente uno tra source.sys_id e source.number");
        let fields=source["fields"].as_array().context("Configura source.fields")?;ensure!(!fields.is_empty()&&fields.len()<=64,"Numero campi non valido");
        let mut selected=vec!["sys_id".to_string(),"number".to_string()];
        for f in fields{let f=f.as_str().context("Nome campo non testuale")?;for part in f.split('.') {identifier(part)?;}if !selected.iter().any(|v|v==f){selected.push(f.into());}}
        let common=[("sysparm_fields",selected.join(",")),("sysparm_display_value","all".into())];
        let record=if !sid.is_empty(){servicenow::sys_id(sid)?;transport(query(&format!("/api/now/table/{table}/{sid}"),&common)?).await?}
        else {ensure!(number.len()<=80&&number.bytes().all(|b|b.is_ascii_alphanumeric()||b"_-".contains(&b)),"Numero richiesta non valido");
            let mut pairs=common.to_vec();pairs.push(("sysparm_query",format!("number={number}")));pairs.push(("sysparm_limit","2".into()));
            let records=transport(query(&format!("/api/now/table/{table}"),&pairs)?).await?;
            let records=records.as_array().context("Elenco richieste non valido")?;ensure!(records.len()==1,"Richiesta non trovata o numero non univoco");records[0].clone()};
        let actual=raw(&record["sys_id"]).context("Risposta senza sys_id")?;servicenow::sys_id(actual)?;
        if !sid.is_empty(){ensure!(actual==sid,"ServiceNow ha restituito un record diverso");}
        if !number.is_empty(){ensure!(raw(&record["number"])==Some(number),"ServiceNow ha restituito un numero diverso");}
        if source["include_variables"]==true {
            ensure!(table=="sc_req_item","La lettura variabili standard è disponibile per sc_req_item; per altre tabelle usare source.fields");
            let response=transport(query("/api/now/table/sc_item_option_mtom",&[("sysparm_query",format!("request_item={actual}")),("sysparm_fields","sc_item_option.item_option_new.name,sc_item_option.value".into()),("sysparm_limit","201".into())])?).await;
            match response {
                Ok(rows)=>{let rows=rows.as_array().context("Variabili RITM non valide")?;ensure!(rows.len()<=200,"Troppe variabili: risultato incompleto");let mut vars=serde_json::Map::new();
                    for row in rows{let name=raw(&row["sc_item_option.item_option_new.name"]).context("Variabile senza nome interno")?;ensure!(!vars.contains_key(name),"Variabile ripetuta {name}: serve un adapter per MRVS");vars.insert(name.into(),json!(raw(&row["sc_item_option.value"]).context("Valore variabile non leggibile o campo escluso da ACL")?));}
                    (record,Value::Object(vars),"read".into())},
                Err(error)=>{ensure!(source["variables_required"]==false,"Lettura variabili necessaria ma non riuscita: {error}");(record,json!({}),"unavailable".into())}
            }
        }else{(record,json!({}),"not_requested".into())}
    };
    Ok(json!({"schema":"rightcommit.servicenow.snapshot.v1","source_run":r.id,"node":n.id,
        "provenance":{"mode":mode,"instance":instance,"table":table,"sys_id":raw(&record["sys_id"]),"number":raw(&record["number"]),"retrieved_at":now(),"configuration":n.location,"variables_status":variable_status},
        "record":record,"variables":variables}))
}
pub(crate) async fn execute(app:&crate::App,uid:i64,r:&Run,n:&Node,c:Option<&ConnectionConfig>)->Result<AgentResult>{
    let config=servicenow_config::read_json(r,&n.location,"configurazione",128000)?;
    let fixture=config["source"]["mode"]=="fixture";
    let instance=if fixture {None}else{Some(servicenow::base(c.context("Configura la connessione ServiceNow nelle impostazioni personali")?)?)};
    let client=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(30)).build()?;
    let snapshot=acquire(r,n,&config,instance,|path|{let client=&client;async move {servicenow::send(client,c.context("Connessione ServiceNow mancante")?,"GET",&path,None).await}}).await?;
    ensure!(!n.id.is_empty()&&n.id.bytes().all(|b|b.is_ascii_alphanumeric()||b"_-".contains(&b)),"ID nodo non valido");
    let root=std::fs::canonicalize(&r.workspace)?;let output=root.join("output");std::fs::create_dir_all(&output)?;ensure!(std::fs::canonicalize(&output)?.starts_with(&root),"Output fuori dal workspace");
    let relative=format!("output/servicenow-{}.json",n.id);let file=Path::new(&r.workspace).join(&relative);ensure!(!file.symlink_metadata().is_ok_and(|m|m.file_type().is_symlink()),"Snapshot non può essere un link");std::fs::write(&file,serde_json::to_vec_pretty(&snapshot)?)?;
    // Native nodes run as the server; subsequent agents use the isolated user.
    crate::agents::own(app,uid,&output)?;crate::agents::own(app,uid,&file)?;
    Ok(AgentResult{status:"completed".into(),decision:"pass".into(),summary:serde_json::to_string_pretty(&snapshot)?,artifacts:vec![relative],..Default::default()})
}

#[cfg(test)]
mod tests {
 use super::*;
 use std::sync::{Arc,Mutex};
 async fn fixture()->(tempfile::TempDir,Run,Node){let dir=tempfile::tempdir().unwrap();let app=crate::tests::app(dir.path());let r=crate::engine::create(&app,1,crate::presets::all().remove(0),"read".into(),None).await.unwrap();let n=Node{id:"read-request".into(),kind:"Connection".into(),provider:"servicenow".into(),operation:"read_request".into(),location:"config.json".into(),..Default::default()};(dir,r,n)}
 #[tokio::test] async fn fixture_is_explicit_and_never_calls_the_network(){
  let(_d,r,n)=fixture().await;std::fs::write(Path::new(&r.workspace).join("fixture.json"),json!({"record":{"sys_id":"a".repeat(32),"number":"RITM-DEMO"},"variables":{"goal":"Example"}}).to_string()).unwrap();
  let config=json!({"schema":"rightcommit.servicenow.requirement.v1","source":{"mode":"fixture","fixture":"fixture.json","table":"sc_req_item"}});
  std::fs::write(Path::new(&r.workspace).join("config.json"),config.to_string()).unwrap();
  let snapshot=acquire(&r,&n,&config,None,|_|async{anyhow::bail!("Network must not be called")}).await.unwrap();assert_eq!(snapshot["provenance"]["mode"],"fixture");assert!(snapshot["provenance"]["instance"].is_null());
  let app=crate::tests::app(_d.path());let result=execute(&app,1,&r,&n,None).await.unwrap();assert_eq!(result.artifacts,vec!["output/servicenow-read-request.json"]);
 }
 #[tokio::test] async fn live_reads_exact_record_and_standard_ritm_variables(){
  let(_d,r,n)=fixture().await;let paths=Arc::new(Mutex::new(Vec::new()));let captured=paths.clone();
  let config=json!({"schema":"rightcommit.servicenow.requirement.v1","source":{"mode":"servicenow","table":"sc_req_item","number":"RITM0010001","fields":["short_description","description"],"include_variables":true,"variables_required":true}});
  let snapshot=acquire(&r,&n,&config,Some("https://dev12345.service-now.com".into()),move |path|{
   captured.lock().unwrap().push(path.clone());async move {if path.contains("sc_item_option_mtom"){Ok(json!([{"sc_item_option.item_option_new.name":"goal","sc_item_option.value":"Approval portal"}]))}else{Ok(json!([{"sys_id":{"value":"a".repeat(32),"display_value":"a".repeat(32)},"number":{"value":"RITM0010001","display_value":"RITM0010001"},"short_description":"Software approval"}]))}}
  }).await.unwrap();assert_eq!(snapshot["variables"]["goal"],"Approval portal");assert_eq!(snapshot["provenance"]["mode"],"servicenow");assert_eq!(paths.lock().unwrap().len(),2);
  let invalid=json!({"schema":"rightcommit.servicenow.requirement.v1","source":{"mode":"servicenow","table":"sc_req_item","sys_id":"a".repeat(32),"number":"RITM0010001","fields":["number"]}});
  assert!(acquire(&r,&n,&invalid,Some("instance".into()),|_|async{anyhow::bail!("No request expected")}).await.is_err());
 }
 #[tokio::test] async fn required_variable_acl_failure_is_not_silently_dropped(){
  let(_d,r,n)=fixture().await;let config=json!({"schema":"rightcommit.servicenow.requirement.v1","source":{"mode":"servicenow","table":"sc_req_item","sys_id":"a".repeat(32),"fields":["number"],"include_variables":true,"variables_required":true}});
  let transport=|path:String|async move{if path.contains("mtom"){anyhow::bail!("ServiceNow HTTP 403 Forbidden")};Ok(json!({"sys_id":"a".repeat(32),"number":"RITM001"}))};
  assert!(acquire(&r,&n,&config,Some("instance".into()),transport).await.unwrap_err().to_string().contains("403"));
 }
}
