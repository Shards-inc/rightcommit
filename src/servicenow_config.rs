//! File-based ServiceNow business configuration. Credentials remain in personal settings.
use crate::model::Run;
use anyhow::{Context,Result,ensure};
use serde_json::{Value,json};
use std::path::{Path,Component};

pub(crate) fn read_json(r:&Run,relative:&str,label:&str,limit:u64)->Result<Value>{
    ensure!(!relative.is_empty()&&!Path::new(relative).is_absolute()&&!Path::new(relative).components().any(|p|matches!(p,Component::ParentDir)),"Usa un file {label} relativo al workspace");
    let root=std::fs::canonicalize(&r.workspace)?;
    let path=std::fs::canonicalize(root.join(relative)).with_context(||format!("File {label} mancante o non accessibile: {relative}"))?;
    ensure!(path.starts_with(&root)&&path.is_file()&&path.metadata()?.len()<=limit,"File {label} fuori dal workspace o troppo grande: {relative}");
    serde_json::from_slice(&std::fs::read(path)?).with_context(||format!("JSON non valido nel file {label}: {relative}"))
}
pub(crate) fn no_credentials(value:&Value)->Result<()> {
    match value {
        Value::Object(values)=>for (key,v) in values {
            ensure!(!["password","token","api_key","client_secret","authorization","access_token"].contains(&key.to_ascii_lowercase().as_str()),"Le credenziali non vanno nel file di configurazione; usa Personal Settings");
            no_credentials(v)?;
        },
        Value::Array(values)=>for v in values {no_credentials(v)?;},
        _=>{}
    }
    Ok(())
}
pub(crate) fn catalog(config:&Value,plan:&Value)->Result<(String,Value)> {
    ensure!(config["schema"]=="rightcommit.servicenow.catalog.v1","Schema configurazione catalogo non valido");
    no_credentials(config)?;
    let key=plan["catalog_route"].as_str().filter(|v|!v.is_empty()).context("Request senza catalog_route: seleziona una regola nel file ticket")?;
    let route=config["catalog"].get(key).with_context(||format!("Regola catalogo {key} assente nel file di configurazione"))?;
    let item=route["item_sys_id"].as_str().filter(|v|!v.is_empty()).with_context(||format!("Configura item_sys_id per la regola {key} nel file di configurazione"))?;
    crate::servicenow::sys_id(item)?;
    let context=json!({"request_id":plan["request_id"],"fields":plan.get("request_fields").cloned().unwrap_or(json!({}))});
    let mapping=route["variables"].as_object().context("La regola deve definire variables come oggetto")?;
    ensure!(mapping.len()<=100,"Troppe variabili nella regola catalogo");
    let mut values=serde_json::Map::new();
    for (name,spec) in mapping {
        ensure!(!name.is_empty()&&name.len()<=100&&name.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'_'),"Nome interno variabile non valido");
        ensure!(spec.is_object()&&(spec.get("from").is_some()^spec.get("literal").is_some()),"Ogni variabile deve definire from oppure literal");
        let value=if let Some(pointer)=spec.get("from") {
            let pointer=pointer.as_str().context("from deve essere un JSON pointer")?;
            ensure!(pointer=="/request_id"||pointer.starts_with("/fields/"),"from deve leggere /fields/... o /request_id");
            context.pointer(pointer).cloned()
        }else {spec.get("literal").cloned()};
        if let Some(value)=value {values.insert(name.clone(),value);}
    }
    ensure!(route.get("required_variables").is_none_or(|v|v.is_array()),"required_variables deve essere un array");
    for required in route["required_variables"].as_array().unwrap_or(&Vec::new()) {
        let name=required.as_str().context("required_variables deve contenere nomi")?;
        ensure!(values.get(name).is_some_and(|v|!v.is_null()&&!v.as_str().is_some_and(|s|s.trim().is_empty())),"Variabile obbligatoria {name} mancante per la regola {key}");
    }
    let values=Value::Object(values);ensure!(serde_json::to_vec(&values)?.len()<=64000,"Variabili catalogo troppo grandi");
    Ok((item.to_string(),values))
}

#[cfg(test)]
mod tests {
 use super::*;
 #[test] fn routes_use_each_requests_fields_and_reject_missing_values_or_secrets(){
  let config=json!({"schema":"rightcommit.servicenow.catalog.v1","namespace":"demo","catalog":{
   "software":{"item_sys_id":"a".repeat(32),"variables":{"product":{"from":"/fields/product"},"quantity":{"literal":1}},"required_variables":["product"]},
   "license":{"item_sys_id":"b".repeat(32),"variables":{"sku":{"from":"/fields/sku"}},"required_variables":["sku"]}}});
  let first=json!({"request_id":"r1","catalog_route":"software","request_fields":{"product":"Python"}});
  let second=json!({"request_id":"r2","catalog_route":"license","request_fields":{"sku":"Editor Pro"}});
  let(a,av)=catalog(&config,&first).unwrap();let(b,bv)=catalog(&config,&second).unwrap();assert_ne!(a,b);assert_eq!(av["product"],"Python");assert_eq!(av["quantity"],1);assert_eq!(bv["sku"],"Editor Pro");
  assert!(catalog(&config,&json!({"catalog_route":"software","request_fields":{}})).is_err());
  assert!(catalog(&config,&json!({"catalog_route":"unknown"})).is_err());
  let mut unsafe_config=config.clone();unsafe_config["password"]=json!("not-allowed");assert!(catalog(&unsafe_config,&first).is_err());
  let mut invalid=config;invalid["catalog"]["software"]["variables"]["product"]=json!({"from":"/env/API_KEY"});assert!(catalog(&invalid,&first).is_err());
 }
}
