mod chunking;
mod embedding;
pub mod model;
mod vector;
use crate::{
    App,
    auth::{ApiResult, bad},
    model::{User, now},
};
use anyhow::{Context, Result, ensure};
use axum::{
    Json,
    extract::{Path, State},
};
use model::*;
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Default)]
pub struct Hub {
    jobs: Mutex<HashMap<(i64, String), Arc<AtomicBool>>>,
    serial: tokio::sync::Mutex<()>,
    encoder: Mutex<Option<(String, fastembed::TextEmbedding)>>,
}
fn init(app: &App) -> Result<()> {
    app.store.db.lock().unwrap().execute_batch("CREATE TABLE IF NOT EXISTS rag_indices(user_id INTEGER NOT NULL,id TEXT NOT NULL,json TEXT NOT NULL,PRIMARY KEY(user_id,id));")?;
    Ok(())
}
fn save(app: &App, uid: i64, i: &Index) -> Result<()> {
    init(app)?;
    app.store.db.lock().unwrap().execute("INSERT INTO rag_indices(user_id,id,json) VALUES(?,?,?) ON CONFLICT(user_id,id) DO UPDATE SET json=excluded.json",params![uid,i.id,serde_json::to_string(i)?])?;
    Ok(())
}
pub fn get(app: &App, uid: i64, id: &str) -> Result<Index> {
    init(app)?;
    let s: Option<String> = app
        .store
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT json FROM rag_indices WHERE user_id=? AND id=?",
            params![uid, id],
            |r| r.get(0),
        )
        .optional()?;
    serde_json::from_str(&s.context("Indice RAG non trovato")?).map_err(Into::into)
}
pub fn indices(app: &App, uid: i64) -> Result<Vec<Index>> {
    init(app)?;
    let db = app.store.db.lock().unwrap();
    let mut stmt = db.prepare("SELECT json FROM rag_indices WHERE user_id=?")?;
    let mut out = stmt
        .query_map([uid], |r| r.get::<_, String>(0))?
        .map(|s| Ok(serde_json::from_str::<Index>(&s?)?))
        .collect::<Result<Vec<_>>>()?;
    out.sort_by_key(|i| (i.name.to_lowercase(), i.root.clone(), i.path.clone()));
    Ok(out)
}
fn base(app: &App, uid: i64, id: &str) -> Result<PathBuf> {
    uuid::Uuid::parse_str(id)?;
    let p = app.data.join("system/rag").join(uid.to_string()).join(id);
    std::fs::create_dir_all(&p)?;
    crate::store::private(&p, true)?;
    Ok(p)
}
fn generation(app: &App, uid: i64, i: &Index) -> Result<PathBuf> {
    let g = i
        .generation
        .as_ref()
        .context("Indice non ancora disponibile")?;
    uuid::Uuid::parse_str(g)?;
    Ok(base(app, uid, &i.id)?.join(g))
}
fn progress(
    app: &App,
    uid: i64,
    i: &mut Index,
    phase: &str,
    done: usize,
    total: Option<usize>,
    message: &str,
) -> Result<()> {
    i.progress.phase = phase.into();
    i.progress.done = done;
    i.progress.total = total;
    i.progress.message = message.into();
    i.progress.updated = now();
    save(app, uid, i)
}
fn check(cancel: &AtomicBool) -> Result<()> {
    ensure!(!cancel.load(Ordering::Relaxed), "Indicizzazione annullata");
    Ok(())
}
#[derive(Deserialize)]
pub struct Start {
    root: String,
    path: String,
    #[serde(default)]
    config: Config,
    #[serde(default)]
    rebuild: bool,
}
pub async fn list(State(app): State<App>, u: User) -> ApiResult<Json<Value>> {
    let mut out = indices(&app, u.id)?;
    for i in &mut out {
        if ["queued", "running", "cancelling"].contains(&i.progress.state.as_str())
            && !app
                .rag
                .jobs
                .lock()
                .unwrap()
                .contains_key(&(u.id, i.id.clone()))
        {
            i.progress.state = "interrupted".into();
            i.progress.message = "Indicizzazione interrotta dal riavvio; puoi riprenderla".into();
            save(&app, u.id, i)?;
        }
    }
    Ok(Json(
        json!({"indices":out,"languages":chunking::languages()}),
    ))
}
pub async fn detail(
    State(app): State<App>,
    u: User,
    Path(id): Path<String>,
) -> ApiResult<Json<Index>> {
    let mut i = get(&app, u.id, &id)?;
    if ["queued", "running", "cancelling"].contains(&i.progress.state.as_str())
        && !app.rag.jobs.lock().unwrap().contains_key(&(u.id, id))
    {
        i.progress.state = "interrupted".into();
        i.progress.message = "Indicizzazione interrotta dal riavvio".into();
        save(&app, u.id, &i)?;
    }
    Ok(Json(i))
}
pub async fn start(
    State(app): State<App>,
    u: User,
    Json(req): Json<Start>,
) -> ApiResult<Json<Index>> {
    req.config.validate()?;
    let source = crate::storage::rag_source(&app, u.id, &req.root, &req.path)?;
    ensure_source(&source)?;
    let mut jobs = app.rag.jobs.lock().unwrap();
    let mut all = indices(&app, u.id)?;
    let mut i = if let Some(n) = all
        .iter()
        .position(|i| i.root == req.root && i.path == req.path)
    {
        all.remove(n)
    } else {
        if all.len() >= 100 {
            return Err(bad("Massimo 100 indici per utente"));
        }
        Index {
            id: uuid::Uuid::new_v4().to_string(),
            root: req.root,
            path: req.path,
            name: source
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            directory: source.is_dir(),
            config: req.config.clone(),
            created: now(),
            updated: now(),
            generation: None,
            generation_config: None,
            progress: Progress::default(),
            stats: Stats::default(),
            warnings: vec![],
        }
    };
    let cancel = Arc::new(AtomicBool::new(false));
    if jobs.contains_key(&(u.id, i.id.clone())) {
        return Err(bad("Indicizzazione già in corso"));
    }
    jobs.insert((u.id, i.id.clone()), cancel.clone());
    i.config = req.config;
    i.progress = Progress {
        state: "queued".into(),
        phase: "queued".into(),
        message: "In attesa del motore locale".into(),
        ..Progress::default()
    };
    if let Err(e) = save(&app, u.id, &i) {
        jobs.remove(&(u.id, i.id.clone()));
        return Err(e.into());
    }
    drop(jobs);
    let result = i.clone();
    tokio::spawn(async move {
        let acquire = app.rag.serial.lock();
        tokio::pin!(acquire);
        let _serial = loop {
            tokio::select! {guard=&mut acquire=>break Some(guard),_=tokio::time::sleep(std::time::Duration::from_millis(200))=>{if cancel.load(Ordering::Relaxed){break None}}}
        };
        let staging = base(&app, u.id, &i.id).map(|p| p.join(uuid::Uuid::new_v4().to_string()));
        let outcome = match staging.as_ref() {
            Ok(dir) => run(&app, u.id, &mut i, &source, dir, req.rebuild, &cancel).await,
            Err(e) => Err(anyhow::anyhow!(e.to_string())),
        };
        if let Err(e) = outcome {
            if let Ok(dir) = staging {
                let _ = std::fs::remove_dir_all(dir);
            }
            i.progress.state = if cancel.load(Ordering::Relaxed) {
                "cancelled"
            } else {
                "failed"
            }
            .into();
            i.progress.message = format!("{e:#}");
            i.progress.updated = now();
            let _ = save(&app, u.id, &i);
        }
        app.rag.jobs.lock().unwrap().remove(&(u.id, i.id.clone()));
    });
    Ok(Json(result))
}
fn ensure_source(p: &std::path::Path) -> Result<()> {
    ensure!(p.is_file() || p.is_dir(), "Sorgente non disponibile");
    Ok(())
}
pub async fn cancel(
    State(app): State<App>,
    u: User,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let _ = get(&app, u.id, &id)?;
    if let Some(c) = app.rag.jobs.lock().unwrap().get(&(u.id, id)) {
        c.store(true, Ordering::Relaxed);
    }
    Ok(Json(json!({"ok":true})))
}
pub async fn delete(
    State(app): State<App>,
    u: User,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let _ = get(&app, u.id, &id)?;
    let jobs = app.rag.jobs.lock().unwrap();
    if jobs.contains_key(&(u.id, id.clone())) {
        return Err(bad(
            "Annulla prima l’indicizzazione e attendi la fine della fase corrente",
        ));
    }
    let p = base(&app, u.id, &id)?;
    std::fs::remove_dir_all(p)?;
    app.store
        .db
        .lock()
        .unwrap()
        .execute(
            "DELETE FROM rag_indices WHERE user_id=? AND id=?",
            params![u.id, id],
        )
        .map_err(anyhow::Error::from)?;
    drop(jobs);
    Ok(Json(json!({"ok":true})))
}
fn scan(
    source: &std::path::Path,
    cfg: &Config,
    cancel: &AtomicBool,
) -> Result<(Manifest, HashMap<String, String>, usize)> {
    let single_file = source.is_file();
    let mut manifest = Manifest {
        content_key: cfg.content_key(),
        ..Manifest::default()
    };
    let mut texts = HashMap::new();
    let mut skipped = 0;
    let mut pending = vec![source.to_path_buf()];
    let mut bytes = 0;
    while let Some(p) = pending.pop() {
        check(cancel)?;
        let m = std::fs::symlink_metadata(&p)?;
        if m.file_type().is_symlink() {
            skipped += 1;
            continue;
        }
        let name = p.file_name().unwrap_or_default().to_string_lossy();
        if p != source
            && ([
                ".git",
                "target",
                "node_modules",
                ".venv",
                ".nodo",
                ".rightcommit",
                "__pycache__",
            ]
            .contains(&name.as_ref())
                || (!cfg.hidden && name.starts_with('.')))
        {
            skipped += 1;
            continue;
        }
        if m.is_dir() {
            if p == source || cfg.recursive {
                for entry in std::fs::read_dir(p)? {
                    pending.push(entry?.path());
                }
            }
            continue;
        }
        if !m.is_file()
            || m.len() > cfg.max_file_mb as u64 * 1024 * 1024
            || (!single_file
                && !cfg.extensions.iter().any(|x| {
                    p.extension()
                        .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case(x))
                }))
        {
            skipped += 1;
            continue;
        }
        let data = std::fs::read(&p)?;
        let Ok(text) = String::from_utf8(data.clone()) else {
            skipped += 1;
            continue;
        };
        if text.contains('\0') {
            skipped += 1;
            continue;
        }
        bytes += data.len();
        ensure!(
            manifest.files.len() < 5000 && bytes <= 128 * 1024 * 1024,
            "Limite indice: 5000 file / 128 MB"
        );
        let rel = if single_file {
            name.to_string()
        } else {
            p.strip_prefix(source)?.to_string_lossy().into()
        };
        manifest.files.insert(
            rel.clone(),
            Stamp {
                hash: crate::store::hash(&data),
                size: m.len(),
                modified: m
                    .modified()?
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_millis(),
            },
        );
        texts.insert(rel, text);
    }
    Ok((manifest, texts, skipped))
}
async fn run(
    app: &App,
    uid: i64,
    i: &mut Index,
    source: &std::path::Path,
    dir: &std::path::Path,
    rebuild: bool,
    cancel: &Arc<AtomicBool>,
) -> Result<()> {
    let started = std::time::Instant::now();
    check(cancel)?;
    i.progress.state = "running".into();
    progress(
        app,
        uid,
        i,
        "scanning",
        0,
        None,
        "Lettura file e confronto degli hash",
    )?;
    let p = source.to_path_buf();
    let cfg = i.config.clone();
    let c = cancel.clone();
    let (manifest, texts, skipped) =
        tokio::task::spawn_blocking(move || scan(&p, &cfg, &c)).await??;
    check(cancel)?;
    let old_dir = generation(app, uid, i).ok();
    let old_manifest = old_dir
        .as_ref()
        .and_then(|p| std::fs::read(p.join("manifest.json")).ok())
        .and_then(|s| serde_json::from_slice::<Manifest>(&s).ok())
        .unwrap_or_default();
    let reusable = !rebuild && old_manifest.content_key == manifest.content_key;
    let changed = manifest
        .files
        .iter()
        .filter(|(p, s)| !reusable || old_manifest.files.get(*p).is_none_or(|o| o.hash != s.hash))
        .map(|(p, _)| p.clone())
        .collect::<Vec<_>>();
    let mut stats = Stats {
        files: manifest.files.len(),
        skipped_files: skipped,
        scan_ms: started.elapsed().as_millis() as u64,
        size_bytes: manifest.files.values().map(|s| s.size).sum(),
        new_files: manifest
            .files
            .keys()
            .filter(|p| !old_manifest.files.contains_key(*p))
            .count(),
        modified_files: manifest
            .files
            .iter()
            .filter(|(p, stamp)| {
                old_manifest
                    .files
                    .get(*p)
                    .is_some_and(|old| old.hash != stamp.hash)
            })
            .count(),
        deleted_files: old_manifest
            .files
            .keys()
            .filter(|p| !manifest.files.contains_key(*p))
            .count(),
        reused_files: manifest.files.len() - changed.len(),
        ..Stats::default()
    };
    if reusable && changed.is_empty() && stats.deleted_files == 0 && i.generation.is_some() {
        stats.chunks = i.stats.chunks;
        stats.reused_chunks = i.stats.chunks;
        stats.total_ms = started.elapsed().as_millis() as u64;
        i.stats = stats;
        i.progress.state = "completed".into();
        progress(
            app,
            uid,
            i,
            "completed",
            1,
            Some(1),
            "Nessuna modifica: indice già aggiornato",
        )?;
        return Ok(());
    }
    i.progress.files_total = changed.len();
    i.progress.files_done = 0;
    let chunk_start = std::time::Instant::now();
    let mut chunks = vec![];
    if reusable && let Some(p) = old_dir.as_ref() {
        chunks = vector::all(p)
            .await?
            .into_iter()
            .filter(|c| {
                manifest
                    .files
                    .get(&c.path)
                    .is_some_and(|s| s.hash == c.source_hash)
            })
            .collect();
    }
    stats.reused_chunks = chunks.len();
    let mut fresh = vec![];
    let mut warnings = vec![];
    for (n, p) in changed.iter().enumerate() {
        check(cancel)?;
        progress(app, uid, i, "chunking", n, Some(changed.len()), p)?;
        let (part, w) = chunking::chunks(p, &texts[p], &manifest.files[p].hash, &i.config).await?;
        fresh.extend(part);
        warnings.extend(w);
        ensure!(
            chunks.len() + fresh.len() <= 50000,
            "Massimo 50000 chunk per indice"
        );
        i.progress.files_done = n + 1;
        i.progress.chunks = chunks.len() + fresh.len();
        progress(app, uid, i, "chunking", n + 1, Some(changed.len()), p)?;
    }
    stats.chunk_ms = chunk_start.elapsed().as_millis() as u64;
    let vectors = i.config.retrieval != "keyword";
    if vectors && !fresh.is_empty() {
        progress(
            app,
            uid,
            i,
            "loading_model",
            0,
            None,
            "Preparazione del modello; al primo utilizzo viene scaricato il modello",
        )?;
        let hub = app.rag.clone();
        let cfg = i.config.clone();
        let cache = app.data.join("system/rag-models");
        let key = format!("{}:{}", cfg.model, cfg.encoder_tokens);
        let load_key = key.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut guard = hub.encoder.lock().unwrap();
            if guard.as_ref().is_none_or(|(k, _)| *k != load_key) {
                *guard = Some((load_key, embedding::load(&cfg, &cache)?));
            }
            Ok(())
        })
        .await??;
        check(cancel)?;
        let hub = app.rag.clone();
        let cfg = i.config.clone();
        let (f, splits) = tokio::task::spawn_blocking(move || {
            let guard = hub.encoder.lock().unwrap();
            embedding::fit(&guard.as_ref().unwrap().1, fresh, &cfg)
        })
        .await??;
        fresh = f;
        stats.encoder_splits = splits;
        ensure!(
            chunks.len() + fresh.len() <= 50000,
            "Massimo 50000 chunk per indice"
        );
        let embedding_start = std::time::Instant::now();
        let total = fresh.len();
        let mut done = 0;
        for batch in fresh.chunks_mut(i.config.batch_size) {
            check(cancel)?;
            progress(
                app,
                uid,
                i,
                "embedding",
                done,
                Some(total),
                "Calcolo locale dei vettori",
            )?;
            let hub = app.rag.clone();
            let texts = batch.iter().map(|c| c.text.clone()).collect::<Vec<_>>();
            let vectors = tokio::task::spawn_blocking(move || {
                hub.encoder
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .1
                    .embed(texts, None)
            })
            .await??;
            for (c, v) in batch.iter_mut().zip(vectors) {
                c.vector = v;
            }
            done += batch.len();
            i.progress.chunks = chunks.len() + total;
            progress(
                app,
                uid,
                i,
                "embedding",
                done,
                Some(total),
                "Calcolo locale dei vettori",
            )?;
        }
        stats.embedded_chunks = total;
        stats.embedding_ms = embedding_start.elapsed().as_millis() as u64;
    }
    chunks.extend(fresh);
    check(cancel)?;
    ensure!(
        !chunks.is_empty(),
        "Nessun contenuto indicizzabile con i filtri selezionati"
    );
    stats.chunks = chunks.len();
    progress(
        app,
        uid,
        i,
        "saving",
        0,
        None,
        "Salvataggio indice e preparazione della ricerca testuale",
    )?;
    let storage_start = std::time::Instant::now();
    std::fs::create_dir_all(dir)?;
    vector::write(dir, &chunks, vectors).await?;
    std::fs::write(dir.join("manifest.json"), serde_json::to_vec(&manifest)?)?;
    check(cancel)?;
    stats.storage_ms = storage_start.elapsed().as_millis() as u64;
    stats.total_ms = started.elapsed().as_millis() as u64;
    let mut published = i.clone();
    published.stats = stats;
    published.warnings = warnings.into_iter().take(100).collect();
    published.generation = Some(dir.file_name().unwrap().to_string_lossy().into());
    published.generation_config = Some(i.config.clone());
    published.updated = now();
    published.progress.state = "completed".into();
    progress(
        app,
        uid,
        &mut published,
        "completed",
        1,
        Some(1),
        "Indice disponibile",
    )?;
    *i = published;
    Ok(())
}
pub async fn search_index(app: &App, uid: i64, id: &str, mut q: Search) -> Result<Value> {
    let i = get(app, uid, id)?;
    let cfg = i
        .generation_config
        .as_ref()
        .context("Indice non disponibile")?;
    ensure!(
        !q.query.trim().is_empty() && q.query.len() <= 4096,
        "Query: 1–4096 caratteri"
    );
    ensure!((1..=30).contains(&q.top_k), "Risultati: 1–30");
    if q.mode.is_empty() {
        q.mode = cfg.retrieval.clone()
    }
    ensure!(
        ["keyword", "semantic", "hybrid"].contains(&q.mode.as_str()),
        "Modalità non valida"
    );
    ensure!(
        cfg.retrieval != "keyword" || q.mode == "keyword",
        "Indice keyword: rigenera con embedding per la ricerca semantica"
    );
    let vector = if q.mode != "keyword" {
        let hub = app.rag.clone();
        let cfg = cfg.clone();
        let text = q.query.clone();
        let cache = app.data.join("system/rag-models");
        Some(
            tokio::task::spawn_blocking(move || -> Result<Vec<f32>> {
                let mut g = hub.encoder.lock().unwrap();
                let key = format!("{}:{}", cfg.model, cfg.encoder_tokens);
                if g.as_ref().is_none_or(|(k, _)| *k != key) {
                    *g = Some((key, embedding::load(&cfg, &cache)?));
                }
                embedding::query(&g.as_ref().unwrap().1, text, &cfg)
            })
            .await??,
        )
    } else {
        None
    };
    let results = vector::search(&generation(app, uid, &i)?, &q, &q.mode, vector).await?;
    Ok(
        json!({"index":i.id,"root":i.root,"path":i.path,"generation":i.generation,"indexed_at":i.updated,"mode":q.mode,"results":results,"notice":"L’indice descrive la sorgente indicizzata, non le modifiche successive del workspace."}),
    )
}
pub async fn search(
    State(app): State<App>,
    u: User,
    Path(id): Path<String>,
    Json(q): Json<Search>,
) -> ApiResult<Json<Value>> {
    Ok(Json(search_index(&app, u.id, &id, q).await?))
}
pub fn tools(node: &crate::model::Node) -> Result<Vec<rmcp::model::Tool>> {
    ["rag_search","rag_status","rag_list_documents","rag_read_chunk"].into_iter().map(|name|Ok(serde_json::from_value(json!({"name":format!("rag_{}_{}",&crate::store::hash(&node.id)[..10],name),"description":format!("{name} — {}. Read-only indexed source; may differ from current run workspace. Source and index generation are returned.",node.title),"inputSchema":{"type":"object","properties":{"query":{"type":"string"},"path_prefix":{"type":"string"},"chunk_id":{"type":"string"},"offset":{"type":"integer","minimum":0}},"additionalProperties":false}}))?)).collect()
}
pub async fn call(
    app: &App,
    uid: i64,
    node: &crate::model::Node,
    name: &str,
    args: Value,
) -> Result<Option<Value>> {
    let prefix = format!("rag_{}_", &crate::store::hash(&node.id)[..10]);
    let Some(method) = name.strip_prefix(&prefix) else {
        return Ok(None);
    };
    let i = get(app, uid, &node.rag_index)?;
    let result = match method {
        "rag_search" => {
            search_index(
                app,
                uid,
                &i.id,
                Search {
                    query: args["query"].as_str().unwrap_or_default().into(),
                    mode: node.rag_mode.clone(),
                    top_k: if node.rag_top_k == 0 {
                        8
                    } else {
                        node.rag_top_k
                    },
                    path_prefix: args["path_prefix"].as_str().unwrap_or_default().into(),
                    ..Search::default()
                },
            )
            .await?
        }
        "rag_status" => {
            json!({"root":i.root,"path":i.path,"generation":i.generation,"indexed_at":i.updated,"stats":i.stats,"progress":i.progress,"config":i.generation_config})
        }
        "rag_list_documents" => {
            let data: Manifest = serde_json::from_slice(&std::fs::read(
                generation(app, uid, &i)?.join("manifest.json"),
            )?)?;
            let offset = args["offset"].as_u64().unwrap_or(0).min(5000) as usize;
            json!({"total":data.files.len(),"offset":offset,"documents":data.files.into_iter().skip(offset).take(100).collect::<Vec<_>>()})
        }
        "rag_read_chunk" => {
            let id = args["chunk_id"].as_str().context("chunk_id richiesto")?;
            let c = vector::chunk(&generation(app, uid, &i)?, id).await?;
            json!({"root":i.root,"source":i.path,"generation":i.generation,"chunk":c})
        }
        _ => return Ok(None),
    };
    Ok(Some(
        json!({"content":[{"type":"text","text":serde_json::to_string(&result)?}],"isError":false}),
    ))
}

#[cfg(test)]
mod tests;
