//! Ticket-level idempotency shared across runs; credentials never enter the plan.
use crate::{App, model::*, servicenow::{self, Effect}};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{collections::BTreeSet, future::Future, io::Write, path::{Path, PathBuf}};

pub(crate) fn namespace(value: &str) -> Result<()> {
    ensure!(!value.is_empty() && value.len() <= 80 && value.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)), "Namespace ITSM non valido: usa 1..80 lettere, numeri, _ o -");
    Ok(())
}
pub(crate) fn identity(instance: &str, ns: &str, request: &str) -> String {
    crate::store::hash(format!("{instance}\0{ns}\0{request}").as_bytes())
}
pub(crate) fn prepare(r: &Run, n: &Node, c: &ConnectionConfig, plan: &Value) -> Result<Effect> {
    ensure!(plan["schema"] == "rightcommit.itsm.batch.v1", "Schema lotto ITSM non valido");
    let ns=servicenow::text(plan,"namespace")?; namespace(ns)?;
    let file_config=if n.target.ends_with(".json") {
        let config=crate::servicenow_config::read_json(r,&n.target,"configurazione",128000)?;
        ensure!(config["schema"]=="rightcommit.servicenow.catalog.v1"&&config["namespace"]==ns,"Namespace o schema del file di configurazione diverso dal lotto");
        crate::servicenow_config::no_credentials(&config)?;Some(config)
    }else {ensure!(n.target.is_empty()||n.target==ns,"Namespace del lotto diverso da quello fissato nel nodo Connection");None};
    let tickets=plan["tickets"].as_array().context("Il lotto deve contenere tickets")?;
    ensure!(!tickets.is_empty() && tickets.len()<=100,"Un lotto ITSM deve contenere 1..100 ticket");
    let mut seen=BTreeSet::new();let mut items=Vec::new();
    for ticket in tickets {
        let request=servicenow::text(ticket,"request_id")?;namespace(request)?;
        ensure!(seen.insert(request),"request_id duplicato nel lotto: {request}");
        let action=servicenow::text(ticket,"action")?;
        let category=servicenow::text(ticket,"category")?;
        ensure!(matches!((action,category),("create_incident","Incident")|("order_catalog","Request")|("answer_only","Other")),"Categoria/azione non valida per {request}");
        let mut effective=c.clone();let mut configuration_error=None;
        if action=="order_catalog" {
            if let Some(config)=&file_config {
                match crate::servicenow_config::catalog(config,ticket) {
                    Ok((item,values))=>{effective.catalog_item=item;effective.catalog_variables=values.to_string();},
                    Err(error)=>configuration_error=Some(error.to_string()),
                }
            }
        }
        let input=json!({"request_id":request,"category":category,"action":action,
            "description":servicenow::text(ticket,"description")?,"short_description":servicenow::text(ticket,"short_description")?,
            "catalog_item":if action=="order_catalog" {effective.catalog_item.as_str()} else {""},
            "variables":if action=="order_catalog" {servicenow::variables(&effective)?} else {Value::Null}});
        let digest=crate::store::hash(serde_json::to_vec(&input)?);
        let mut item=json!({"request_id":request,"category":category,"action":action,"input_digest":digest,"status":"ready","needs_human":ticket["status"]=="needs_human"});
        if action=="order_catalog" && (configuration_error.is_some() || effective.catalog_item.is_empty()) {
            item["status"]=json!("blocked");item["message"]=json!(configuration_error.unwrap_or_else(||"Configura articolo e variabili in un file della Source e indica il file nel target del nodo Connection; gli altri ticket verranno elaborati.".into()));
        } else {
            let mut single=n.clone();single.operation="apply_ticket_plan".into();
            let mut effect=servicenow::prepare_plan(&single,r,&effective,ticket)?;
            if action=="create_incident" {
                let correlation=format!("rightcommit:itsm:{}",&identity(&effect.instance,ns,request)[..40]);
                effect.correlation=correlation.clone();effect.body["correlation_id"]=json!(correlation);
            }
            item["effect"]=serde_json::to_value(effect)?;
        }
        items.push(item);
    }
    Ok(Effect {instance:servicenow::base(c)?,method:"BATCH".into(),path:String::new(),body:json!({"namespace":ns,"items":items}),correlation:String::new()})
}
fn receipt_path(app: &App, uid:i64, instance:&str, ns:&str, request:&str)->PathBuf {
    app.data.join("system/itsm-items").join(uid.to_string()).join(format!("{}.json.enc",identity(instance,ns,request)))
}
fn read_receipt(app:&App,path:&Path)->Result<Value>{Ok(serde_json::from_slice(&app.store.decrypt(&std::fs::read(path)?)?)?)}
fn save_receipt(app:&App,path:&Path,value:&Value)->Result<()> {
    let tmp=path.with_extension(format!("{}.tmp",id()));
    let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    crate::store::private(&tmp,false)?;file.write_all(&app.store.encrypt(&serde_json::to_vec(value)?)?)?;file.sync_all()?;
    std::fs::rename(tmp,path)?;Ok(())
}
fn status_error(item:&Value,error:impl std::fmt::Display)->Value {
    json!({"request_id":item["request_id"],"category":item["category"],"action":item["action"],"status":"attention_required","message":error.to_string()})
}
async fn incident_lookup<F,Fut>(effect:&Effect,transport:&F)->Result<Option<Value>>
where F:Fn(String,String,Option<Value>)->Fut,Fut:Future<Output=Result<Value>> {
    let mut url=reqwest::Url::parse("https://example.invalid")?;
    url.query_pairs_mut().append_pair("sysparm_query",&format!("correlation_id={}",effect.correlation)).append_pair("sysparm_limit","2");
    let query=url.query().unwrap();
    let values=transport("GET".into(),format!("/api/now/table/incident?{query}"),None).await?;
    let records=values.as_array().context("Ricerca incident non valida")?;
    ensure!(records.len()<=1,"Più incident con la stessa identità: riconciliare prima di procedere");
    if let Some(result)=records.first(){
        ensure!(result["correlation_id"]==effect.correlation && result["description"]==effect.body["description"],"L’incidente esistente ha contenuti diversi: nessun duplicato creato");
        servicenow::sys_id(servicenow::text(result,"sys_id")?)?;
        ensure!(result["number"].as_str().is_some_and(|v|v.starts_with("INC")),"Record recuperato non valido");
        return Ok(Some(result.clone()));
    }
    Ok(None)
}
async fn apply_item<F,Fut>(app:&App,uid:i64,r:&Run,n:&Node,ns:&str,item:&Value,transport:&F)->Result<Value>
where F:Fn(String,String,Option<Value>)->Fut,Fut:Future<Output=Result<Value>> {
    let request=servicenow::text(item,"request_id")?;
    let effect:Effect=serde_json::from_value(item["effect"].clone())?;
    if effect.method=="NONE" {return Ok(json!({"request_id":request,"category":item["category"],"action":item["action"],"status":if item["needs_human"]==true {"needs_human"} else {"answered"},"result":effect.body}));}
    ensure!(effect.method=="POST","Metodo non ammesso nel lotto");
    let path=receipt_path(app,uid,&effect.instance,ns,request);
    let dir=path.parent().unwrap();std::fs::create_dir_all(dir)?;crate::store::private(dir,true)?;
    let pending=json!({"schema":1,"namespace":ns,"request_id":request,"instance":effect.instance,"input_digest":item["input_digest"],"effect":effect,"state":"pending_reconciliation","run":r.id,"node":n.id,"created":now()});
    let fresh=match std::fs::OpenOptions::new().write(true).create_new(true).open(&path){
        Ok(mut file)=>{crate::store::private(&path,false)?;file.write_all(&app.store.encrypt(&serde_json::to_vec(&pending)?)?)?;file.sync_all()?;true},
        Err(error) if error.kind()==std::io::ErrorKind::AlreadyExists=>false,
        Err(error)=>return Err(error.into()),
    };
    let outcome=|status:&str,result:Value|json!({"request_id":request,"category":item["category"],"action":item["action"],"status":status,"result":result});
    if !fresh {
        let receipt=read_receipt(app,&path).context("Ricevuta in uso o non leggibile; nessun reinvio eseguito")?;
        ensure!(receipt["input_digest"]==item["input_digest"],"Ticket {request} già noto con contenuti o configurazione diversi: usa un aggiornamento esplicito, non una nuova creazione");
        if receipt["state"]=="applied" {
            let result=if item["action"]=="create_incident" {
                let previous=&receipt["result"];let incident=servicenow::sys_id(servicenow::text(previous,"sys_id")?)?;
                let live=transport("GET".into(),format!("/api/now/table/incident/{incident}"),None).await?;
                ensure!(live["sys_id"]==previous["sys_id"] && live["number"]==previous["number"],"Ricevuta e incidente remoto non corrispondono; nessuna creazione ripetuta");
                live
            } else { receipt["result"].clone() };
            return Ok(outcome("reused",result));
        }
        if !effect.correlation.is_empty() {
            if let Some(result)=incident_lookup(&effect,transport).await? {
                let mut recovered=receipt;recovered["state"]=json!("applied");recovered["result"]=result.clone();save_receipt(app,&path,&recovered)?;return Ok(outcome("reused",result));
            }
        }
        anyhow::bail!("Esito precedente incerto per {request}: riconciliare ServiceNow; la richiesta non viene reinviata automaticamente");
    }
    if !effect.correlation.is_empty() {
        if let Some(result)=incident_lookup(&effect,transport).await? {
            let mut recovered=pending;recovered["state"]=json!("applied");recovered["result"]=result.clone();save_receipt(app,&path,&recovered)?;return Ok(outcome("reused",result));
        }
    }
    let result=transport(effect.method.clone(),effect.path.clone(),Some(effect.body.clone())).await?;
    ensure!(result.get("sys_id").is_some()||result.get("request_id").is_some()||result.get("request_number").is_some(),"ServiceNow non ha restituito un identificativo: riconciliare prima di ritentare");
    if item["action"]=="create_incident" {ensure!(result["correlation_id"]==effect.correlation,"Correlazione non confermata; riconciliare la ricevuta");}
    let mut applied=pending;applied["state"]=json!("applied");applied["result"]=result.clone();save_receipt(app,&path,&applied)?;
    Ok(outcome("applied",result))
}
pub(crate) async fn execute(app:&App,uid:i64,r:&Run,n:&Node,c:&ConnectionConfig,effect:&Effect,client:&reqwest::Client)->Result<Value>{
    execute_with(app,uid,r,n,effect,|method,path,body|async move {servicenow::send(client,c,&method,&path,body.as_ref()).await}).await
}
async fn execute_with<F,Fut>(app:&App,uid:i64,r:&Run,n:&Node,effect:&Effect,transport:F)->Result<Value>
where F:Fn(String,String,Option<Value>)->Fut,Fut:Future<Output=Result<Value>> {
    ensure!(effect.method=="BATCH","Anteprima lotto non valida");let ns=servicenow::text(&effect.body,"namespace")?;namespace(ns)?;
    if n.target.ends_with(".json") {
        let config=crate::servicenow_config::read_json(r,&n.target,"configurazione",128000)?;
        ensure!(config["namespace"]==ns,"Namespace diverso dal file di configurazione approvato");
    } else {ensure!(n.target.is_empty()||n.target==ns,"Namespace diverso dal nodo approvato");}
    let items=effect.body["items"].as_array().context("Lotto senza elementi")?;
    ensure!(!items.is_empty()&&items.len()<=100,"Dimensione lotto non valida");
    let mut seen=BTreeSet::new();
    for item in items {
        let request=servicenow::text(item,"request_id")?;namespace(request)?;
        ensure!(seen.insert(request),"Ticket duplicato nell’anteprima");
        ensure!(item["input_digest"].as_str().is_some_and(|v|v.len()==64&&v.bytes().all(|b|b.is_ascii_hexdigit())),"Impronta ticket non valida");
        if item["status"]=="blocked" {continue;}
        let child:Effect=serde_json::from_value(item["effect"].clone())?;
        ensure!(child.instance==effect.instance,"Istanza del ticket diversa dal lotto approvato");
        ensure!(matches!((item["action"].as_str(),child.method.as_str()),(Some("create_incident"|"order_catalog"),"POST")|(Some("answer_only"),"NONE")),"Azione ticket non valida");
        if item["action"]=="order_catalog" {
            let catalog=child.path.strip_prefix("/api/sn_sc/servicecatalog/items/").and_then(|v|v.strip_suffix("/order_now")).context("Endpoint catalogo non valido")?;
            servicenow::sys_id(catalog)?;ensure!(child.correlation.is_empty(),"Correlazione catalogo non valida");
        }
        if item["action"]=="create_incident" {
            ensure!(child.path=="/api/now/table/incident" && child.correlation==format!("rightcommit:itsm:{}",&identity(&effect.instance,ns,request)[..40]) && child.body["correlation_id"]==child.correlation,"Identità incident non valida");
        }
    }
    let mut results=Vec::new();
    for item in items {
        let request=servicenow::text(item,"request_id")?;
        if item["status"]=="blocked" {results.push(item.clone());continue;}
        let result=apply_item(app,uid,r,n,ns,item,&transport).await.unwrap_or_else(|e|status_error(item,e));
        crate::agents::emit(app,uid,&r.id,"output",&n.id,&format!("Ticket {}: {}",request,result["status"].as_str().unwrap_or("attention_required")));
        results.push(result);
    }
    let attention=results.iter().filter(|v|matches!(v["status"].as_str(),Some("attention_required"|"blocked"|"needs_human"))).count();
    Ok(json!({"namespace":ns,"status":if attention==0 {"completed"} else {"attention_required"},"total":results.len(),"attention_required":attention,"items":results}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::{Arc,Mutex},future::{Ready,ready}};
    #[derive(Default)] struct Server { records:Vec<Value>, posts:usize, orders:usize, lose_incident_response:bool, lose_order_response:bool }
    fn transport(server:Arc<Mutex<Server>>)->impl Fn(String,String,Option<Value>)->Ready<Result<Value>> {
        move |method,path,body| {
            let result=(||->Result<Value>{
                let mut s=server.lock().unwrap();
                if method=="GET" {
                    if path.contains('?') {
                        let url=reqwest::Url::parse(&format!("https://example.invalid{path}"))?;
                        let query=url.query_pairs().find(|(k,_)|k=="sysparm_query").unwrap().1.into_owned();
                        let correlation=query.strip_prefix("correlation_id=").unwrap();
                        return Ok(json!(s.records.iter().filter(|v|v["correlation_id"]==correlation).collect::<Vec<_>>()));
                    }
                    return s.records.iter().find(|v|path.ends_with(v["sys_id"].as_str().unwrap())).cloned().context("Record not found");
                }
                ensure!(method=="POST","Unexpected request");
                if path=="/api/now/table/incident" {
                    s.posts+=1;let mut value=body.unwrap();value["sys_id"]=json!(format!("{:032x}",s.posts));value["number"]=json!(format!("INC{:07}",s.posts));value["state"]=json!("1");s.records.push(value.clone());
                    if s.lose_incident_response {s.lose_incident_response=false;anyhow::bail!("Simulated lost incident response")};return Ok(value);
                }
                ensure!(path.ends_with("/order_now"),"Unexpected endpoint");s.orders+=1;
                if s.lose_order_response {s.lose_order_response=false;anyhow::bail!("Simulated lost catalog response")};
                Ok(json!({"request_id":format!("{:032x}",s.orders+100),"request_number":format!("REQ{:07}",s.orders)}))
            })();ready(result)
        }
    }
    fn config()->ConnectionConfig {ConnectionConfig{base_url:"https://dev12345.service-now.com".into(),catalog_item:"a".repeat(32),catalog_variables:"{}".into(),..Default::default()}}
    fn ticket(id:&str,category:&str)->Value {json!({"request_id":id,"category":category,"action":match category {"Incident"=>"create_incident","Request"=>"order_catalog",_=>"answer_only"},"description":format!("Ticket {id}"),"short_description":format!("Ticket {id}"),"answer":"KB answer","status":if id=="unknown" {"needs_human"} else {"resolution_proposed"}})}
    fn plan()->Value {json!({"schema":"rightcommit.itsm.batch.v1","namespace":"test-customer","tickets":[ticket("incident","Incident"),ticket("request","Request"),ticket("faq","Other"),ticket("unknown","Other")]})}
    async fn fixture()->(tempfile::TempDir,App,Run,Node){let dir=tempfile::tempdir().unwrap();let app=crate::tests::app(dir.path());let r=crate::engine::create(&app,1,crate::presets::all().remove(0),"batch".into(),None).await.unwrap();let node=Node{id:"apply-plan".into(),operation:"apply_ticket_batch".into(),..Default::default()};(dir,app,r,node)}
    #[tokio::test]
    async fn repeated_runs_and_kb_changes_do_not_duplicate_incidents_or_orders(){
        let (_d,app,mut r,mut n)=fixture().await;let server=Arc::new(Mutex::new(Server::default()));let c=config();let p=plan();
        let e=prepare(&r,&n,&c,&p).unwrap();let first=execute_with(&app,1,&r,&n,&e,transport(server.clone())).await.unwrap();
        assert_eq!(first["items"][0]["status"],"applied");assert_eq!(first["items"][1]["status"],"applied");assert_eq!(first["items"][2]["status"],"answered");assert_eq!(first["items"][3]["status"],"needs_human");
        r.id="another-run".into();n.id="another-node".into();let mut changed=p;changed["tickets"][0]["answer"]=json!("Updated KB wording");let next=prepare(&r,&n,&c,&changed).unwrap();
        assert_eq!(e.body["items"][0]["effect"]["correlation"],next.body["items"][0]["effect"]["correlation"]);
        let second=execute_with(&app,1,&r,&n,&next,transport(server.clone())).await.unwrap();assert_eq!(second["items"][0]["status"],"reused");assert_eq!(second["items"][1]["status"],"reused");
        assert_eq!(server.lock().unwrap().posts,1);assert_eq!(server.lock().unwrap().orders,1);
    }
    #[tokio::test]
    async fn missing_catalog_only_blocks_request_and_can_be_configured_later(){
        let (_d,app,r,n)=fixture().await;let server=Arc::new(Mutex::new(Server::default()));let mut c=config();c.catalog_item.clear();
        let first=execute_with(&app,1,&r,&n,&prepare(&r,&n,&c,&plan()).unwrap(),transport(server.clone())).await.unwrap();assert_eq!(first["items"][1]["status"],"blocked");assert_eq!(first["items"][0]["status"],"applied");assert_eq!(server.lock().unwrap().orders,0);
        let second=execute_with(&app,1,&r,&n,&prepare(&r,&n,&config(),&plan()).unwrap(),transport(server.clone())).await.unwrap();assert_eq!(second["items"][1]["status"],"applied");assert_eq!(second["items"][0]["status"],"reused");assert_eq!(server.lock().unwrap().posts,1);
    }
    #[tokio::test]
    async fn lost_incident_response_is_reconciled_but_unknown_catalog_is_never_reposted(){
        let (_d,app,r,n)=fixture().await;let server=Arc::new(Mutex::new(Server{lose_incident_response:true,lose_order_response:true,..Default::default()}));let e=prepare(&r,&n,&config(),&plan()).unwrap();
        let first=execute_with(&app,1,&r,&n,&e,transport(server.clone())).await.unwrap();assert_eq!(first["items"][0]["status"],"attention_required");assert_eq!(first["items"][1]["status"],"attention_required");assert_eq!(first["items"][2]["status"],"answered");
        let second=execute_with(&app,1,&r,&n,&e,transport(server.clone())).await.unwrap();assert_eq!(second["items"][0]["status"],"reused");assert_eq!(second["items"][1]["status"],"attention_required");assert_eq!(server.lock().unwrap().posts,1);assert_eq!(server.lock().unwrap().orders,1);
    }
    #[tokio::test]
    async fn duplicate_ids_invalid_batches_and_changed_ticket_content_fail_without_duplicate_writes(){
        let (_d,app,r,n)=fixture().await;let c=config();let mut p=plan();p["tickets"][1]=p["tickets"][0].clone();assert!(prepare(&r,&n,&c,&p).is_err());
        p=plan();p["namespace"]=json!("../escape");assert!(prepare(&r,&n,&c,&p).is_err());
        p=plan();p["tickets"]=json!([]);assert!(prepare(&r,&n,&c,&p).is_err());
        p=plan();p["tickets"][0]["category"]=json!("Other");assert!(prepare(&r,&n,&c,&p).is_err());
        let server=Arc::new(Mutex::new(Server::default()));execute_with(&app,1,&r,&n,&prepare(&r,&n,&c,&plan()).unwrap(),transport(server.clone())).await.unwrap();
        p=plan();p["tickets"][0]["description"]=json!("Changed original ticket");let result=execute_with(&app,1,&r,&n,&prepare(&r,&n,&c,&p).unwrap(),transport(server.clone())).await.unwrap();assert_eq!(result["items"][0]["status"],"attention_required");assert_eq!(server.lock().unwrap().posts,1);
    }
    #[tokio::test]
    async fn concurrent_runs_claim_the_same_ticket_only_once(){
        let (_d,app,r,n)=fixture().await;let server=Arc::new(Mutex::new(Server::default()));let mut p=plan();p["tickets"]=json!([ticket("concurrent","Incident")]);let e=prepare(&r,&n,&config(),&p).unwrap();
        let network=|method,path,body|{let shared=server.clone();async move {tokio::time::sleep(std::time::Duration::from_millis(5)).await;transport(shared)(method,path,body).await}};
        let (a,b)=tokio::join!(execute_with(&app,1,&r,&n,&e,&network),execute_with(&app,1,&r,&n,&e,&network));
        assert!(a.is_ok()&&b.is_ok());assert_eq!(server.lock().unwrap().posts,1);
        let again=execute_with(&app,1,&r,&n,&e,transport(server.clone())).await.unwrap();assert_eq!(again["items"][0]["status"],"reused");assert_eq!(server.lock().unwrap().posts,1);
    }

    #[tokio::test]
    async fn file_configuration_routes_requests_without_personal_catalog_settings(){
        let (_d,app,r,mut n)=fixture().await;n.target="catalog.json".into();
        let config=json!({"schema":"rightcommit.servicenow.catalog.v1","namespace":"test-customer","catalog":{"software":{"item_sys_id":"b".repeat(32),"variables":{"product":{"from":"/fields/product"}},"required_variables":["product"]}}});
        std::fs::write(std::path::Path::new(&r.workspace).join("catalog.json"),config.to_string()).unwrap();
        let mut p=plan();p["tickets"][1]["catalog_route"]=json!("software");p["tickets"][1]["request_fields"]=json!({"product":"Python"});
        let mut c=config_for_file_test();c.catalog_item.clear();let e=prepare(&r,&n,&c,&p).unwrap();assert_eq!(e.body["items"][1]["status"],"ready");assert_eq!(e.body["items"][1]["effect"]["body"]["variables"]["product"],"Python");
        let server=Arc::new(Mutex::new(Server::default()));let result=execute_with(&app,1,&r,&n,&e,transport(server.clone())).await.unwrap();assert_eq!(result["items"][1]["status"],"applied");assert_eq!(server.lock().unwrap().orders,1);
        p["tickets"][1]["request_fields"]=json!({});let blocked=prepare(&r,&n,&c,&p).unwrap();assert_eq!(blocked.body["items"][1]["status"],"blocked");assert_eq!(blocked.body["items"][0]["status"],"ready");
    }
    fn config_for_file_test()->ConnectionConfig{config()}

}
