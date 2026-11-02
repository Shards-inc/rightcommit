use crate::{
    App,
    auth::{ApiResult, bad},
    model::User,
};
use axum::{
    Json,
    body::Bytes,
    extract::{Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};
#[derive(Deserialize)]
pub struct Location {
    #[serde(default)]
    path: String,
    #[serde(default)]
    directory: bool,
    #[serde(default = "default_root")]
    root: String,
}
fn default_root() -> String {
    "sources".into()
}
fn root(app: &App, name: &str) -> anyhow::Result<PathBuf> {
    let path = match name {
        "sources" => app.sources.clone(),
        "examples" => app.assets.join("examples"),
        _ => anyhow::bail!("Archivio non valido"),
    };
    Ok(std::fs::canonicalize(path)?)
}
pub(crate) fn source_folder(app: &App, name: &str, location: &str) -> anyhow::Result<PathBuf> {
    let base = root(app, name)?;
    let folder = resolve(&base, if location == "." { "" } else { location })?;
    anyhow::ensure!(folder.is_dir(), "Seleziona una cartella");
    Ok(folder)
}
pub async fn source_size(
    State(app): State<App>,
    user: User,
    Query(q): Query<Location>,
) -> ApiResult<Json<Value>> {
    if q.root != "sources" {
        return Err(bad("Dimensione copia disponibile solo per sources"));
    }
    let folder = source_folder(&app, "sources", &q.path)?;
    let threshold = app.store.settings(user.id)?.source_copy_limit_bytes()?;
    let bytes = crate::agents::source_tree_size(&folder, None)?;
    let copy_error = crate::agents::source_copy_size(&folder, threshold)
        .err()
        .map(|e| e.to_string());
    Ok(Json(
        json!({"bytes":bytes,"threshold_bytes":threshold,"copy_error":copy_error}),
    ))
}

fn relative(s: &str) -> anyhow::Result<&Path> {
    let p = Path::new(s);
    anyhow::ensure!(
        !p.is_absolute() && p.components().all(|c| matches!(c, Component::Normal(_))),
        "Percorso non valido"
    );
    Ok(p)
}
fn resolve(base: &Path, s: &str) -> anyhow::Result<PathBuf> {
    let p = base.join(relative(s)?);
    let mut cursor = base.to_path_buf();
    for c in relative(s)?.components() {
        cursor.push(c);
        anyhow::ensure!(
            !std::fs::symlink_metadata(&cursor)?.file_type().is_symlink(),
            "Link simbolici non supportati"
        );
    }
    Ok(p)
}
const EXPORT_OMIT: &[&str] = &[".git", "target", "node_modules", "__pycache__", ".venv"];
fn workspace_path(app: &App, uid: i64, path: &str) -> anyhow::Result<PathBuf> {
    relative(path)?;
    let (id, rest) = path.split_once('/').unwrap_or((path, ""));
    anyhow::ensure!(!id.is_empty(), "Seleziona un'esecuzione");
    let run = app.store.run(uid, id)?;
    let root = crate::subworkflows::workspace(app, uid, &run)?;
    let base = resolve(&root, "")?;
    anyhow::ensure!(
        std::fs::canonicalize(&base)? == std::fs::canonicalize(&run.workspace)?,
        "Workspace non valido"
    );
    anyhow::ensure!(
        !Path::new(rest)
            .components()
            .any(|c| EXPORT_OMIT.iter().any(|name| c.as_os_str() == *name)),
        "Cartella interna esclusa dall'esportazione"
    );
    resolve(&base, rest)
}
fn storage_path(app: &App, uid: i64, q: &Location) -> anyhow::Result<PathBuf> {
    if q.root == "workspaces" {
        workspace_path(app, uid, &q.path)
    } else {
        resolve(&root(app, &q.root)?, &q.path)
    }
}
fn reports(
    app: &App,
    uid: i64,
    id: &str,
) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
    app.store.run(uid, id)?;
    let mut latest = std::collections::BTreeMap::new();
    let mut artifacts = app.store.artifacts(uid, id)?;
    artifacts.sort_by_key(|a| a.version);
    for a in artifacts {
        if a.path.starts_with(".nodo/reports/") && relative(&a.path).is_ok() {
            latest.insert(a.path, a.content);
        }
    }
    Ok(latest)
}
fn report_view(app: &App, uid: i64, path: &str) -> anyhow::Result<Option<Value>> {
    relative(path)?;
    let (id, rest) = path.split_once('/').unwrap_or((path, ""));
    if rest != ".nodo" && rest != ".nodo/reports" && !rest.starts_with(".nodo/reports/") {
        return Ok(None);
    }
    let reports = reports(app, uid, id)?;
    if let Some(content) = reports.get(rest) {
        let settings=app.store.settings(uid)?;
        let limit=settings.file_preview_limit_bytes()?;
        let preview=(content.len() as u64<=limit && !content.contains('\0')).then_some(content);
        let reason=if preview.is_some(){None}else{Some(if content.len() as u64>limit {"size"}else{"format"})};
        return Ok(Some(json!({"path":path,"file":true,"preview":preview,"html":preview.map(|s|crate::api::render_markdown(s)),"size":content.len(),"preview_reason":reason,"preview_limit_mb":settings.file_preview_limit_mb})));
    }
    if rest == ".nodo/reports" {
        return Ok(Some(
            json!({"path":path,"entries":reports.keys().map(|p|json!({"name":p.trim_start_matches(".nodo/reports/"),"directory":false,"size":reports[p].len()})).collect::<Vec<_>>()}),
        ));
    }
    if rest == ".nodo" && workspace_path(app, uid, path).is_err() {
        return Ok(Some(
            json!({"path":path,"entries":[{"name":"reports","directory":true,"size":0}]}),
        ));
    }
    Ok(None)
}
pub async fn list(
    State(app): State<App>,
    u: User,
    Query(q): Query<Location>,
) -> ApiResult<Json<Value>> {
    if q.root == "workspaces" && q.path.is_empty() {
        let entries: Vec<_> = app.store.runs(u.id)?.iter().map(|r| json!({"name":r.id,"label":format!("{} · {}",r.workflow.name,r.id.chars().take(8).collect::<String>()),"directory":true,"size":0,"status":r.status,"source_mode":if r.context.contains_key("direct-primary") {"direct"} else {"copy"}})).collect();
        return Ok(Json(json!({"path":"","entries":entries})));
    }
    if q.root == "workspaces"
        && let Some(view) = report_view(&app, u.id, &q.path)?
    {
        return Ok(Json(view));
    }
    let p = storage_path(&app, u.id, &q)?;
    if p.is_file() {
        let size = p.metadata()?.len();
        let settings=app.store.settings(u.id)?;
        let limit=settings.file_preview_limit_bytes()?;
        let preview=crate::artifacts::read_text_preview(&p,limit)?;
        let reason=if preview.is_some(){None}else{Some(if size>limit {"size"}else{"format"})};
        return Ok(Json(json!({"path":q.path,"file":true,"html":if q.path.ends_with(".md") { preview.as_ref().map(|text|crate::api::render_markdown(text)) } else { None },"preview":preview,"size":size,"preview_reason":reason,"preview_limit_mb":settings.file_preview_limit_mb})));
    }
    let mut entries = Vec::new();
    for e in std::fs::read_dir(p)?.take(10001) {
        let e = e?;
        let m = e.path().symlink_metadata()?;
        if m.file_type().is_symlink()
            || (q.root == "workspaces"
                && EXPORT_OMIT.contains(&e.file_name().to_string_lossy().as_ref()))
        {
            continue;
        }
        entries.push(
            json!({"name":e.file_name().to_string_lossy(),"directory":m.is_dir(),"size":m.len()}),
        );
    }
    if q.root == "workspaces" {
        let (id, rest) = q.path.split_once('/').unwrap_or((&q.path, ""));
        let name = if rest.is_empty() {
            Some(".nodo")
        } else if rest == ".nodo" {
            Some("reports")
        } else {
            None
        };
        if let Some(name) = name
            && !reports(&app, u.id, id)?.is_empty()
            && !entries.iter().any(|e| e["name"] == name)
        {
            entries.push(json!({"name":name,"directory":true,"size":0}));
        }
    }
    if entries.len() > 10000 {
        return Err(bad("Cartella troppo grande"));
    }
    entries.sort_by_key(|e| {
        (
            !e["directory"].as_bool().unwrap_or(false),
            e["name"].as_str().unwrap_or("").to_lowercase(),
        )
    });
    Ok(Json(json!({"path":q.path,"entries":entries})))
}
pub async fn upload(
    State(app): State<App>,
    _u: User,
    Query(q): Query<Location>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    if ["examples", "workspaces"].contains(&q.root.as_str()) {
        return Err(crate::auth::ApiError(
            StatusCode::FORBIDDEN,
            "Archivio di sola lettura".into(),
        ));
    }
    let base = root(&app, &q.root)?;
    let rel = relative(&q.path)?;
    if q.path.is_empty() {
        return Err(bad("Nome file mancante"));
    }
    let mut parent = base.clone();
    if let Some(dirs) = rel.parent() {
        for c in dirs.components() {
            parent.push(c);
            match std::fs::create_dir(&parent) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(e) => return Err(e.into()),
            }
            let m = parent.symlink_metadata()?;
            if !m.is_dir() || m.file_type().is_symlink() {
                return Err(bad("Percorso cartella non valido"));
            }
        }
    }
    let dest = parent.join(rel.file_name().ok_or_else(|| bad("Nome mancante"))?);
    if q.directory {
        match std::fs::create_dir(&dest) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let m = dest.symlink_metadata()?;
                if !m.is_dir() || m.file_type().is_symlink() {
                    return Err(bad("Conflitto con un file esistente"));
                }
            }
            Err(e) => return Err(e.into()),
        }
        return Ok(Json(json!({"ok":true,"path":q.path,"directory":true})));
    }
    let mut f = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&dest)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(crate::auth::ApiError(
                StatusCode::CONFLICT,
                "Esiste già un file con questo nome".into(),
            ));
        }
        Err(e) => return Err(e.into()),
    };
    use std::io::Write;
    if let Err(e) = f.write_all(&body) {
        drop(f);
        let _ = std::fs::remove_file(dest);
        return Err(e.into());
    }
    Ok(Json(json!({"ok":true,"path":q.path,"size":body.len()})))
}
pub async fn download(
    State(app): State<App>,
    u: User,
    Query(q): Query<Location>,
) -> ApiResult<impl IntoResponse> {
    let mut report_files = std::collections::BTreeMap::new();
    let mut selected = String::new();
    let p = if q.root == "workspaces" {
        relative(&q.path)?;
        let (id, rest) = q.path.split_once('/').unwrap_or((&q.path, ""));
        report_files = reports(&app, u.id, id)?;
        selected = rest.to_string();
        if rest == ".nodo" || rest == ".nodo/reports" || report_files.contains_key(rest) {
            workspace_path(&app, u.id, id)?.join(rest)
        } else {
            workspace_path(&app, u.id, &q.path)?
        }
    } else {
        storage_path(&app, u.id, &q)?
    };
    let omit = q.root == "workspaces";
    let bytes=tokio::task::spawn_blocking(move||->anyhow::Result<Vec<u8>>{
 let mut child=std::process::Command::new("python3").args(["-c",r#"
import sys,os,zipfile,io
from pathlib import Path
p=Path(sys.argv[1]); files=[]
omit={'.git','target','node_modules','__pycache__','.venv'} if sys.argv[2]=='true' else set()
if p.is_dir():
 for parent,dirs,names in os.walk(p,followlinks=False):
  dirs[:]=sorted(d for d in dirs if d not in omit)
  files.extend(Path(parent)/d for d in dirs)
  files.extend(Path(parent)/n for n in sorted(names) if n not in omit)
  if len(files)>10000: raise ValueError('Limite di 10000 elementi superato')
else: files=[p] if p.exists() else []
reports=__import__('json').load(sys.stdin); selected=Path(sys.argv[3]) if sys.argv[3] else None
total=0; out=io.BytesIO()
with zipfile.ZipFile(out,'w',zipfile.ZIP_DEFLATED) as z:
 for f in files:
  if f.is_symlink() or any(a.is_symlink() for a in f.parents): raise ValueError('Link simbolico non supportato')
  total+=f.stat().st_size
  if total>200*1024*1024: raise ValueError('Limite ZIP di 200 MB superato')
  z.write(f, str(Path(p.name)/f.relative_to(p)) if p.is_dir() else f.name)
 for name,content in reports.items():
  rp=Path(name)
  if selected is None: rel=Path(p.name)/rp
  elif rp==selected: rel=Path(rp.name)
  elif selected in rp.parents: rel=Path(p.name)/rp.relative_to(selected)
  else: continue
  total+=len(content.encode())
  if total>200*1024*1024 or len(z.namelist())>=10000: raise ValueError('Limite ZIP superato')
  if str(rel) not in z.namelist(): z.writestr(str(rel),content)
 if not z.namelist() and not p.is_dir(): raise ValueError('File non trovato')
sys.stdout.buffer.write(out.getvalue())
"#]).arg(p).arg(omit.to_string()).arg(selected).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn()?;
 use std::io::Write;
 let mut stdin=child.stdin.take().ok_or_else(||anyhow::anyhow!("Input ZIP non disponibile"))?;
 stdin.write_all(&serde_json::to_vec(&report_files)?)?;drop(stdin);
 let output=child.wait_with_output()?;anyhow::ensure!(output.status.success(),"Download ZIP non disponibile: limite 200 MB / 10000 elementi o link simbolici");Ok(output.stdout)
 }).await.map_err(|_|bad("Download interrotto"))??;
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip"),
            (
                header::CONTENT_DISPOSITION,
                if q.root == "workspaces" {
                    "attachment; filename=\"result.zip\""
                } else if q.root == "examples" {
                    "attachment; filename=\"examples.zip\""
                } else {
                    "attachment; filename=\"sources.zip\""
                },
            ),
        ],
        bytes,
    ))
}
impl From<std::io::Error> for crate::auth::ApiError {
    fn from(e: std::io::Error) -> Self {
        bad(e.to_string())
    }
}

pub(crate) fn rag_source(app: &App, uid: i64, root: &str, path: &str) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        !path.split('/').any(|s| s == ".nodo"),
        "I report virtuali non sono sorgenti indicizzabili"
    );
    storage_path(
        app,
        uid,
        &Location {
            root: root.into(),
            path: path.into(),
            directory: false,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn workspace_access_is_owned_and_excludes_internal_paths() {
        let d = tempfile::tempdir().unwrap();
        let app = crate::tests::app(d.path());
        let run = crate::engine::create(
            &app,
            1,
            crate::presets::all().remove(0),
            "storage test".into(),
            None,
        )
        .await
        .unwrap();
        let path = format!("{}/src/lib.rs", run.id);
        assert!(workspace_path(&app, 1, &path).unwrap().is_file());
        assert!(workspace_path(&app, 2, &path).is_err());
        assert!(workspace_path(&app, 1, &format!("{}/../..", run.id)).is_err());
        assert!(workspace_path(&app, 1, &format!("{}/.git/config", run.id)).is_err());
        assert!(workspace_path(&app, 1, &format!("{}/target/file", run.id)).is_err());
        assert!(workspace_path(&app, 1, "").is_err());
        std::os::unix::fs::symlink("/etc", Path::new(&run.workspace).join("escape")).unwrap();
        assert!(workspace_path(&app, 1, &format!("{}/escape/passwd", run.id)).is_err());
    }
    #[test]
    fn rejects_escape_and_symlinks() {
        let t = tempfile::tempdir().unwrap();
        assert!(relative("../secret").is_err());
        assert!(relative("/etc/passwd").is_err());
        assert!(relative("foo/bar").is_ok());
        std::os::unix::fs::symlink("/etc", t.path().join("link")).unwrap();
        assert!(resolve(t.path(), "link/passwd").is_err());
        assert!(resolve(t.path(), "").is_ok());
    }
}
