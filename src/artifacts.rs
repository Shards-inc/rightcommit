//! Run outputs are changes against the prepared sources, not a workspace listing.
use crate::{
    App,
    model::{Artifact, ArtifactFile, Run, id, now},
    store::private,
};
use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    fs::File,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

const INLINE_PREVIEW_LIMIT: u64 = 1024 * 1024;

// Disposable, per-process cache: hashes in the database and immutable blobs remain
// authoritative. A restart/eviction simply costs one full read, never loses history.
#[derive(Default)]
pub(crate) struct Tracker {
    runs: Mutex<VecDeque<((i64, String), Arc<ScanState>)>>,
}
impl Tracker {
    fn run(&self, uid: i64, run: &str) -> Arc<ScanState> {
        let mut runs = self.runs.lock().unwrap();
        let key = (uid, run.to_owned());
        let entry = runs
            .iter()
            .position(|(k, _)| k == &key)
            .and_then(|i| runs.remove(i))
            .unwrap_or_else(|| (key, Arc::default()));
        let cache = entry.1.clone();
        runs.push_back(entry);
        // Do not evict an in-flight scan: its lock also serializes snapshots.
        while runs.len() > 16 {
            let Some(i) = runs
                .iter()
                .position(|(_, cache)| Arc::strong_count(cache) == 1)
            else {
                break;
            };
            runs.remove(i);
        }
        cache
    }
}

#[derive(Default)]
struct ScanState {
    inventory: Mutex<InventoryCache>,
    gate: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct FileStamp {
    device: u64,
    inode: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
impl FileStamp {
    fn of(meta: &std::fs::Metadata) -> Self {
        Self {
            device: meta.dev(),
            inode: meta.ino(),
            size: meta.len(),
            modified: (meta.mtime(), meta.mtime_nsec()),
            changed: (meta.ctime(), meta.ctime_nsec()),
        }
    }
    fn settled(&self) -> bool {
        // Racy timestamps on coarse-resolution filesystems must never authorize
        // reuse. ctime also catches same-size writes with a restored mtime.
        let cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(1) as i64;
        self.modified.0 < cutoff && self.changed.0 < cutoff
    }
}
#[derive(Default)]
struct InventoryCache {
    files: BTreeMap<String, (FileStamp, String)>,
    hashed_files: usize,
    monitor: Option<crate::file_watch::Monitor>,
    pending: std::collections::BTreeSet<PathBuf>,
}

fn digest(path: &Path, expected: &FileStamp) -> Result<String> {
    // Never turn a concurrently replaced symlink into cached source content.
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "Artefatto non regolare: {}",
        path.display()
    );
    ensure!(
        FileStamp::of(&file.metadata()?) == *expected,
        "File cambiato durante la scansione: {}",
        path.display()
    );
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    ensure!(
        FileStamp::of(&file.metadata()?) == *expected
            && FileStamp::of(&std::fs::symlink_metadata(path)?) == *expected,
        "File cambiato durante la scansione: {}",
        path.display()
    );
    Ok(hex::encode(hash.finalize()))
}

pub(crate) async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .context("Operazione su file interrotta")?
}

pub async fn baseline_async(app: &App, uid: i64, run: &Run) -> Result<()> {
    let state = app.artifact_tracker.run(uid, &run.id);
    let guard = state.gate.clone().lock_owned().await;
    let (app, run) = (app.clone(), run.clone());
    blocking(move || {
        let (_guard, _state) = (guard, state);
        baseline(&app, uid, &run)
    })
    .await
}

pub async fn snapshot_async(
    app: &App,
    uid: i64,
    run: &Run,
    node: &str,
    binding: &str,
    summary: &str,
) -> Result<()> {
    let state = app.artifact_tracker.run(uid, &run.id);
    // Acquire before dispatch, retain inside the job: cancelling the async caller
    // cannot let a final/Stop snapshot overtake a still-queued disk operation.
    let guard = state.gate.clone().lock_owned().await;
    let (app, run, node, binding, summary) = (
        app.clone(),
        run.clone(),
        node.to_owned(),
        binding.to_owned(),
        summary.to_owned(),
    );
    blocking(move || {
        let (_guard, _state) = (guard, state);
        snapshot(&app, uid, &run, &node, &binding, &summary)
    })
    .await
}

fn working_roots(app: &App, uid: i64, run: &Run) -> Result<Vec<PathBuf>> {
    let lineage = crate::subworkflows::ancestors(app, uid, run)?;
    let root = lineage.last().unwrap_or(run);
    let mut roots = vec![
        PathBuf::from(&run.workspace),
        app.data.join(format!("users/{uid}/workspaces/{}", crate::subworkflows::workspace_owner(root)?)),
    ];
    for (index, node) in root
        .workflow
        .nodes
        .iter()
        .filter(|n| n.kind == "Source")
        .enumerate()
    {
        let path = PathBuf::from(crate::source_access::source_path(root, index, node));
        roots.push(if path.is_absolute() {
            path
        } else {
            Path::new(&root.workspace).join(path)
        });
    }
    // Parents of a registered root are validated too; never traverse a link into
    // another user's files. Git's internal object store is not a workflow output.
    let data = std::fs::canonicalize(&app.data)?;
    let sources = std::fs::canonicalize(&app.sources).unwrap_or_else(|_| app.sources.clone());
    let mut canonical = Vec::new();
    for path in roots {
        if !path.exists() {
            continue;
        }
        ensure!(
            !path
                .ancestors()
                .take_while(|p| p.starts_with(&app.data)
                    || p.starts_with(&app.sources)
                    || p.starts_with(&data)
                    || p.starts_with(&sources))
                .any(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.is_symlink())),
            "Link nel percorso artefatti: {}",
            path.display()
        );
        let path = std::fs::canonicalize(path)?;
        ensure!(
            path.starts_with(&data) || path.starts_with(&sources),
            "Cartella artefatti esterna alle sorgenti della run"
        );
        if !canonical.contains(&path) {
            canonical.push(path);
        }
    }
    // A primary workspace often contains the secondary Source roots. Walking it
    // once is sufficient; otherwise every nested directory is enumerated twice.
    let roots = canonical.clone();
    canonical.retain(|path| {
        !roots
            .iter()
            .any(|parent| parent != path && path.starts_with(parent))
    });
    Ok(canonical)
}

fn inventory(app: &App, uid: i64, run: &Run) -> Result<BTreeMap<String, String>> {
    let cache = app.artifact_tracker.run(uid, &run.id);
    inventory_cached(app, uid, run, &mut cache.inventory.lock().unwrap())
}

fn inventory_cached(
    app: &App,
    uid: i64,
    run: &Run,
    cache: &mut InventoryCache,
) -> Result<BTreeMap<String, String>> {
    fn walk(
        path: &Path,
        files: &mut BTreeMap<String, String>,
        next: &mut InventoryCache,
        old: &InventoryCache,
    ) -> Result<()> {
        let meta = match std::fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        if meta.is_symlink() {
            return Ok(());
        }
        if meta.is_dir() {
            if path.file_name().is_some_and(|n| n == ".git") {
                return Ok(());
            }
            for entry in std::fs::read_dir(path)? {
                walk(&entry?.path(), files, next, old)?;
            }
        } else if meta.is_file() {
            let key = path.to_string_lossy().into_owned();
            let stamp = FileStamp::of(&meta);
            let hash = if let Some((_, hash)) = old.files.get(&key).filter(|(s, _)| s == &stamp) {
                hash.clone()
            } else {
                next.hashed_files += 1;
                digest(path, &stamp)
                    .with_context(|| format!("Lettura artefatto {}", path.display()))?
            };
            if stamp.settled() {
                next.files.insert(key.clone(), (stamp, hash.clone()));
            }
            files.insert(key, hash);
        }
        Ok(())
    }
    let started = std::time::Instant::now();
    let roots = working_roots(app, uid, run)?;
    // Start before the initial scan. Root changes and restarts rebuild the watch.
    if run.parent.is_none() && !cache.monitor.as_ref().is_some_and(|m| m.matches(&roots)) {
        cache.monitor = match crate::file_watch::Monitor::start(roots.clone()) {
            Ok(monitor) => Some(monitor),
            Err(error) => {
                tracing::warn!(run=%run.id, %error, "Notifiche non disponibili; riconciliazione metadati attiva");
                None
            }
        };
    }
    if let Some(monitor) = &cache.monitor {
        let changes = monitor.take();
        if changes.uncertain {
            cache.files.clear();
        } else {
            cache
                .files
                .retain(|path, _| !changes.paths.contains(Path::new(path)));
        }
    }
    let mut files = BTreeMap::new();
    let mut next = InventoryCache::default();
    for root in roots {
        walk(&root, &mut files, &mut next, cache)?;
    }
    next.monitor = cache.monitor.take();
    tracing::debug!(run=%run.id, files=files.len(), hashed=next.hashed_files,
        elapsed_ms=started.elapsed().as_millis(), "Inventario artefatti");
    *cache = next; // Deleted paths are not retained; failed scans never publish a partial cache.
    Ok(files)
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
enum Baseline {
    Metadata {
        version: u8,
        metadata: BTreeMap<String, FileStamp>,
    },
    Legacy(BTreeMap<String, String>),
}

fn metadata_inventory(
    app: &App,
    uid: i64,
    run: &Run,
    cache: &mut InventoryCache,
) -> Result<BTreeMap<String, FileStamp>> {
    fn walk(path: &Path, files: &mut BTreeMap<String, FileStamp>) -> Result<()> {
        let meta = match std::fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        if meta.is_symlink() {
            return Ok(());
        }
        if meta.is_dir() {
            if path.file_name().is_some_and(|n| n == ".git") {
                return Ok(());
            }
            for entry in std::fs::read_dir(path)? {
                walk(&entry?.path(), files)?;
            }
        } else if meta.is_file() {
            files.insert(path.to_string_lossy().into_owned(), FileStamp::of(&meta));
        }
        Ok(())
    }
    let roots = working_roots(app, uid, run)?;
    if run.parent.is_none() && !cache.monitor.as_ref().is_some_and(|m| m.matches(&roots)) {
        cache.monitor = crate::file_watch::Monitor::start(roots.clone()).ok();
    }
    let mut files = BTreeMap::new();
    for root in roots {
        walk(&root, &mut files)?;
    }
    Ok(files)
}

pub fn baseline(app: &App, uid: i64, run: &Run) -> Result<()> {
    let cache = app.artifact_tracker.run(uid, &run.id);
    baseline_cached(app, uid, run, &mut cache.inventory.lock().unwrap())
}
fn baseline_cached(app: &App, uid: i64, run: &Run, cache: &mut InventoryCache) -> Result<()> {
    // Resume must rearm notifications before the next agent starts, even when
    // the immutable baseline is already in the database.
    if run.parent.is_none() && cache.monitor.is_none() {
        cache.monitor = crate::file_watch::Monitor::start(working_roots(app, uid, run)?).ok();
    }
    let exists: bool = app.store.db.lock().unwrap().query_row(
        "SELECT EXISTS(SELECT 1 FROM artifact_baselines WHERE user_id=? AND run=?)",
        params![uid, run.id],
        |r| r.get(0),
    )?;
    if !exists {
        let metadata = metadata_inventory(app, uid, run, cache)?;
        let json = serde_json::to_string(&Baseline::Metadata {
            version: 1,
            metadata: metadata.clone(),
        })?;
        let observed = serde_json::to_string(&metadata)?;
        let mut db = app.store.db.lock().unwrap();
        let tx = db.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO artifact_baselines VALUES(?,?,?)",
            params![uid, run.id, json],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO artifact_observations VALUES(?,?,?)",
            params![uid, run.id, observed],
        )?;
        tx.commit()?;
        cache.hashed_files = 0;
        // Events preceding this metadata baseline describe the initial inputs.
        if let Some(monitor) = &cache.monitor {
            monitor.take();
        }
    }
    Ok(())
}

pub(crate) fn stop_monitor(app: &App, uid: i64, run: &str) {
    let state = app.artifact_tracker.run(uid, run);
    let monitor = state.inventory.lock().unwrap().monitor.take();
    // OS watcher teardown can wait for its event thread. It is read-only and
    // must not delay child completion notifications or hold the scheduler slot.
    if let Some(monitor) = monitor {
        tokio::task::spawn_blocking(move || drop(monitor));
    }
}

pub(crate) fn blob_path(app: &App, uid: i64, hash: &str) -> Result<PathBuf> {
    ensure!(
        hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
        "Impronta artefatto non valida"
    );
    Ok(app
        .data
        .join("system/artifact-files")
        .join(uid.to_string())
        .join(hash))
}

fn capture(app: &App, uid: i64, path: &Path) -> Result<(String, u64, Option<String>)> {
    let folder = app.data.join("system/artifact-files").join(uid.to_string());
    std::fs::create_dir_all(&folder)?;
    private(&folder, true)?;
    let temp = folder.join(format!("{}.tmp", id()));
    let result = (|| {
        let mut input = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)?;
        ensure!(
            input.metadata()?.is_file(),
            "Artefatto non regolare: {}",
            path.display()
        );
        let before = FileStamp::of(&input.metadata()?);
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        private(&temp, false)?;
        let mut hash = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0; 65536];
        loop {
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            output.write_all(&buffer[..n])?;
            hash.update(&buffer[..n]);
            size += n as u64;
        }
        ensure!(
            FileStamp::of(&input.metadata()?) == before
                && FileStamp::of(&std::fs::symlink_metadata(path)?) == before,
            "File cambiato durante la raccolta: {}",
            path.display()
        );
        output.flush()?;
        let hash = hex::encode(hash.finalize());
        let target = blob_path(app, uid, &hash)?;
        std::fs::rename(&temp, &target)?;
        let preview = if size <= INLINE_PREVIEW_LIMIT {
            std::fs::read_to_string(&target)
                .ok()
                .filter(|s| !s.contains('\0'))
        } else {
            None
        };
        Ok((hash, size, preview))
    })();
    if temp.exists() {
        let _ = std::fs::remove_file(temp);
    }
    result
}

fn description(
    app: &App,
    run: &Run,
    location: &str,
    change: &str,
    size: u64,
    preview: bool,
) -> (String, ArtifactFile) {
    let location_buf = std::fs::canonicalize(location).unwrap_or_else(|_| PathBuf::from(location));
    let location = location_buf.as_path();
    let sources = std::fs::canonicalize(&app.sources).unwrap_or_else(|_| app.sources.clone());
    let workspace =
        std::fs::canonicalize(&run.workspace).unwrap_or_else(|_| PathBuf::from(&run.workspace));
    let (root, path) = if let Ok(path) = location.strip_prefix(sources) {
        ("/sources", path.to_path_buf())
    } else {
        (
            "/data",
            location
                .strip_prefix(&workspace)
                .unwrap_or(location)
                .to_path_buf(),
        )
    };
    (
        path.to_string_lossy().into_owned(),
        ArtifactFile {
            modified: std::fs::metadata(location)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_millis() as u64),
            root: root.into(),
            location: location.to_string_lossy().into_owned(),
            change: change.into(),
            size,
            preview,
            blob: true,
        },
    )
}

/// Snapshot all changed files, including binary/large/hidden files, without a
/// catalog-size cap. Preview limits never limit discovery or download.
pub fn snapshot(
    app: &App,
    uid: i64,
    run: &Run,
    node: &str,
    binding: &str,
    summary: &str,
) -> Result<()> {
    let cache = app.artifact_tracker.run(uid, &run.id);
    let mut cache = cache.inventory.lock().unwrap();
    baseline_cached(app, uid, run, &mut cache)?;
    let raw: String = app.store.db.lock().unwrap().query_row(
        "SELECT json FROM artifact_baselines WHERE user_id=? AND run=?",
        params![uid, run.id],
        |r| r.get(0),
    )?;
    let baseline: Baseline = serde_json::from_str(&raw)?;
    let initial: BTreeMap<String, String> = match &baseline {
        Baseline::Legacy(hashes) => hashes.clone(),
        Baseline::Metadata { metadata, .. } => metadata
            .keys()
            .map(|p| (p.clone(), String::new()))
            .collect(),
    };
    let previous = app.store.artifacts(uid, &run.id)?;
    let previous_files: BTreeMap<_, _> = previous
        .iter()
        .filter_map(|artifact| {
            let info = file_info(app, uid, run, artifact);
            (info.change != "report").then_some((info.location, artifact))
        })
        .collect();
    let mut observations = None;
    let files = match &baseline {
        Baseline::Legacy(_) => inventory_cached(app, uid, run, &mut cache)?,
        Baseline::Metadata { metadata, .. } => {
            let raw: Option<String> = app
                .store
                .db
                .lock()
                .unwrap()
                .query_row(
                    "SELECT json FROM artifact_observations WHERE user_id=? AND run=?",
                    params![uid, run.id],
                    |r| r.get(0),
                )
                .optional()?;
            let old: BTreeMap<String, FileStamp> = raw
                .map(|s| serde_json::from_str(&s))
                .transpose()?
                .unwrap_or_else(|| metadata.clone());
            let current = metadata_inventory(app, uid, run, &mut cache)?;
            if let Some(monitor) = &cache.monitor {
                let changes = monitor.take();
                cache.pending.extend(changes.paths);
                // Overflow/errors and silent Docker loss are recovered by the
                // full metadata reconciliation above, without reading all GB.
            }
            let candidates = current
                .iter()
                .filter(|(path, stamp)| {
                    old.get(*path) != Some(*stamp)
                        || cache.pending.contains(Path::new(path))
                        || (stamp.changed.1 == 0 && !stamp.settled())
                        || stamp.changed.0 <= 0
                })
                .map(|(p, _)| (p.clone(), String::new()))
                .collect();
            observations = Some(current);
            cache.hashed_files = 0;
            candidates
        }
    };
    let session = run.sessions.get(binding).cloned().unwrap_or_default();
    for (location, hash) in files {
        let prior = previous_files.get(&location).copied();
        if observations.is_none()
            && prior.is_some_and(|a| a.file.is_none())
            && initial.get(&location) == Some(&hash)
        {
            continue;
        }
        if observations.is_none()
            && prior
                .map(|a| a.hash.as_str())
                .or_else(|| initial.get(&location).map(String::as_str))
                == Some(hash.as_str())
        {
            continue;
        }
        let (hash, size, preview) = capture(app, uid, Path::new(&location))?;
        cache.hashed_files += usize::from(observations.is_some());
        if prior.is_some_and(|a| a.hash == hash) {
            continue;
        }
        let (path, file) = description(
            app,
            run,
            &location,
            if initial.contains_key(&location) {
                "modified"
            } else {
                "created"
            },
            size,
            preview.is_some(),
        );
        app.store.add_artifact(
            uid,
            &Artifact {
                id: id(),
                run: run.id.clone(),
                node: node.into(),
                path,
                version: prior.map_or(1, |a| a.version + 1),
                hash,
                content: preview.unwrap_or_default(),
                session: session.native_id.clone(),
                turn: session.turns,
                created: now(),
                file: Some(file),
            },
        )?;
    }
    if !summary.is_empty() {
        let path = format!(".rightcommit/reports/{node}.md");
        let prior = previous
            .iter()
            .rev()
            .find(|a| a.path == path && a.file.as_ref().is_none_or(|f| f.change == "report"));
        let lineage = crate::subworkflows::ancestors(app, uid, run)?;
        let root = lineage.last().unwrap_or(run);
        let location = app.data.join(format!(
            "users/{uid}/workspaces/{}/.rightcommit/reports/{node}.md",
            root.id
        ));
        app.store.add_artifact(
            uid,
            &Artifact {
                id: id(),
                run: run.id.clone(),
                node: node.into(),
                path,
                version: prior.map_or(1, |a| a.version + 1),
                hash: crate::store::hash(summary),
                content: summary.into(),
                session: session.native_id,
                turn: session.turns,
                created: now(),
                file: Some(ArtifactFile {
                    modified: now() as u64 * 1000,
                    root: "/data".into(),
                    location: location.to_string_lossy().into_owned(),
                    change: "report".into(),
                    size: summary.len() as u64,
                    preview: true,
                    blob: false,
                }),
            },
        )?;
    }
    if let Some(observations) = observations {
        let json = serde_json::to_string(&observations)?;
        app.store.db.lock().unwrap().execute("INSERT INTO artifact_observations VALUES(?,?,?) ON CONFLICT(user_id,run) DO UPDATE SET json=excluded.json", params![uid,run.id,json])?;
        cache.pending.clear();
    }
    Ok(())
}

/// Immutable download: never follow a mutable source path from artifact metadata.
pub fn download_path(app: &App, uid: i64, artifact: &Artifact) -> Result<Option<PathBuf>> {
    if artifact.file.as_ref().is_some_and(|f| f.blob) {
        Ok(Some(blob_path(app, uid, &artifact.hash)?))
    } else {
        Ok(None)
    }
}

pub fn cleanup(app: &App, uid: i64) -> Result<()> {
    let busy = app.busy.lock().unwrap();
    if !busy.is_empty() {
        return Ok(());
    }
    let folder = app.data.join("system/artifact-files").join(uid.to_string());
    if !folder.exists() {
        return Ok(());
    }
    let db = app.store.db.lock().unwrap();
    let mut q = db.prepare("SELECT json FROM artifacts WHERE user_id=?")?;
    let hashes = q
        .query_map([uid], |row| row.get::<_, String>(0))?
        .map(|row| {
            let a: Artifact = serde_json::from_str(&row?)?;
            Ok(a.hash)
        })
        .collect::<Result<std::collections::HashSet<_>>>()?;
    for entry in std::fs::read_dir(folder)? {
        let path = entry?.path();
        if path.extension().is_none()
            && !hashes.contains(&path.file_name().unwrap().to_string_lossy().into_owned())
        {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

/// The saved bytes are immutable. Read previews on demand so a personal limit
/// change also applies to files acquired before that preference existed.
pub(crate) fn read_text_preview(path: &Path, limit: u64) -> Result<Option<String>> {
    let file = File::open(path)?;
    if file.metadata()?.len() > limit {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Ok(None);
    }
    Ok(String::from_utf8(bytes)
        .ok()
        .filter(|text| !text.contains('\0')))
}
pub(crate) fn text_preview(
    app: &App,
    uid: i64,
    artifact: &Artifact,
    limit: u64,
) -> Result<Option<String>> {
    if let Some(path) = download_path(app, uid, artifact)? {
        return read_text_preview(&path, limit);
    }
    Ok(
        (artifact.content.len() as u64 <= limit && !artifact.content.contains('\0'))
            .then(|| artifact.content.clone()),
    )
}

pub fn file_info(app: &App, uid: i64, run: &Run, artifact: &Artifact) -> ArtifactFile {
    if let Some(file) = &artifact.file {
        return file.clone();
    }
    let workspace =
        std::fs::canonicalize(&run.workspace).unwrap_or_else(|_| PathBuf::from(&run.workspace));
    let location = workspace.join(&artifact.path);
    let (_, mut info) = description(
        app,
        run,
        &location.to_string_lossy(),
        if crate::source_access::report_path(&artifact.path) {
            "report"
        } else {
            "legacy"
        },
        artifact.content.len() as u64,
        true,
    );
    info.blob = false;
    info.modified = artifact.created.max(0) as u64 * 1000;
    if info.change == "report" {
        info.root = "/data".into();
        info.location = app
            .data
            .join(format!(
                "users/{uid}/workspaces/{}/{}",
                run.id, artifact.path
            ))
            .to_string_lossy()
            .into_owned();
    }
    info
}

#[cfg(test)]
pub fn rebase(app: &App, uid: i64, old: &Run, new: &Run, artifact: &mut Artifact) {
    let Some(file) = &mut artifact.file else {
        return;
    };
    let location = PathBuf::from(&file.location);
    let mut mappings = vec![
        (
            app.data.join(format!("users/{uid}/workspaces/{}", old.id)),
            app.data.join(format!("users/{uid}/workspaces/{}", new.id)),
        ),
        (PathBuf::from(&old.workspace), PathBuf::from(&new.workspace)),
    ];
    for (index, node) in old
        .workflow
        .nodes
        .iter()
        .filter(|n| n.kind == "Source")
        .enumerate()
    {
        let from = PathBuf::from(crate::source_access::source_path(old, index, node));
        let from = if from.is_absolute() {
            from
        } else {
            Path::new(&old.workspace).join(from)
        };
        if let Some(next) = new.workflow.nodes.iter().find(|n| n.id == node.id) {
            let to = PathBuf::from(crate::source_access::source_path(new, index, next));
            mappings.push((
                from,
                if to.is_absolute() {
                    to
                } else {
                    Path::new(&new.workspace).join(to)
                },
            ));
        }
    }
    mappings.sort_by_key(|(from, _)| std::cmp::Reverse(from.components().count()));
    for (from, to) in mappings {
        let from = std::fs::canonicalize(&from).unwrap_or(from);
        let to = std::fs::canonicalize(&to).unwrap_or(to);
        if let Ok(relative) = location.strip_prefix(from) {
            let (path, mut info) = description(
                app,
                new,
                &to.join(relative).to_string_lossy(),
                &file.change,
                file.size,
                file.preview,
            );
            info.blob = file.blob;
            info.modified = file.modified;
            *file = info;
            artifact.path = path;
            break;
        }
    }
}

/// Backfill old runs once. Existing versions remain immutable; the current
/// workspace is explicitly recorded as recovered, with no invented node author.
pub fn recover_legacy_run(app: &App, uid: i64, run: &Run) -> Result<usize> {
    let recovered: bool = app.store.db.lock().unwrap().query_row(
        "SELECT EXISTS(SELECT 1 FROM artifact_recoveries WHERE user_id=? AND run=?)",
        params![uid, run.id],
        |r| r.get(0),
    )?;
    if recovered {
        return Ok(0);
    }
    ensure!(
        Path::new(&run.workspace).is_dir(),
        "Workspace storico non disponibile"
    );
    let previous = app.store.artifacts(uid, &run.id)?;
    let has_baseline: bool = app.store.db.lock().unwrap().query_row(
        "SELECT EXISTS(SELECT 1 FROM artifact_baselines WHERE user_id=? AND run=?)",
        params![uid, run.id],
        |r| r.get(0),
    )?;
    if !previous.iter().any(|a| a.file.is_none())
        && (has_baseline
            || !["completed", "failed", "interrupted", "stopped"].contains(&run.status.as_str()))
    {
        return Ok(0);
    }
    let latest: BTreeMap<_, _> = previous
        .iter()
        .filter_map(|a| {
            let info = file_info(app, uid, run, a);
            (info.change != "report").then_some((info.location, a))
        })
        .collect();
    let mut count = 0;
    for (location, hash) in inventory(app, uid, run)? {
        let prior = latest.get(&location).copied();
        if prior.is_some_and(|a| a.hash == hash) {
            continue;
        }
        let modified = std::fs::metadata(&location)?
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as u64;
        // Missing snapshots may be inputs omitted by the former 1000-file cap.
        // Without an initial inventory, only discover files dated since this run
        // began. They are labelled recovered, never claimed as created by a node.
        if prior.is_none() && modified < run.created.max(0) as u64 * 1000 {
            continue;
        }
        let (hash, size, preview) = capture(app, uid, Path::new(&location))?;
        if prior.is_some_and(|a| a.hash == hash) {
            continue;
        }
        let (path, mut info) =
            description(app, run, &location, "recovered", size, preview.is_some());
        info.modified = modified;
        app.store.add_artifact(
            uid,
            &Artifact {
                id: id(),
                run: run.id.clone(),
                node: "artifact-recovery".into(),
                path,
                version: prior.map_or(1, |a| a.version + 1),
                hash,
                content: preview.unwrap_or_default(),
                session: String::new(),
                turn: 0,
                created: now(),
                file: Some(info),
            },
        )?;
        count += 1;
    }
    app.store.db.lock().unwrap().execute(
        "INSERT OR IGNORE INTO artifact_recoveries VALUES(?,?,?)",
        params![
            uid,
            run.id,
            serde_json::json!({"recovered":count,"at":now()}).to_string()
        ],
    )?;
    Ok(count)
}

pub fn recover_legacy_runs(app: &App) -> Result<(usize, usize)> {
    let runs: Vec<(i64, Run)> = {
        let db = app.store.db.lock().unwrap();
        let mut q=db.prepare("SELECT r.user_id,r.json FROM runs r WHERE (EXISTS(SELECT 1 FROM artifacts a WHERE a.user_id=r.user_id AND a.run=r.id AND json_extract(a.json,'$.file') IS NULL) OR (json_extract(r.json,'$.status') IN ('completed','failed','interrupted','stopped') AND NOT EXISTS(SELECT 1 FROM artifact_baselines b WHERE b.user_id=r.user_id AND b.run=r.id))) AND NOT EXISTS(SELECT 1 FROM artifact_recoveries b WHERE b.user_id=r.user_id AND b.run=r.id) ORDER BY r.rowid")?;
        q.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .map(|row| {
            let (uid, json) = row?;
            Ok((uid, serde_json::from_str(&json)?))
        })
        .collect::<Result<_>>()?
    };
    let mut completed = 0;
    let mut files = 0;
    for (uid, run) in runs {
        match recover_legacy_run(app, uid, &run) {
            Ok(count) => {
                completed += 1;
                files += count;
                tracing::info!(run=%run.id,recovered=count,"Artefatti storici recuperati");
            }
            Err(error) => {
                tracing::warn!(run=%run.id,%error,"Recupero artefatti storici non completato; verrà ritentato al riavvio")
            }
        }
    }
    Ok((completed, files))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn fixture(direct: bool) -> (tempfile::TempDir, App, Run, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let rid = id();
        let run_folder = app.data.join(format!("users/1/workspaces/{rid}"));
        let workspace = if direct {
            app.sources.join("main")
        } else {
            run_folder.clone()
        };
        let second = app.sources.join("second");
        let copied = run_folder.join(".rightcommit/sources/copied");
        for p in [&workspace, &second, &copied] {
            std::fs::create_dir_all(p).unwrap();
            std::fs::write(p.join("original.txt"), "original").unwrap();
        }
        let mut context = BTreeMap::from([(
            "direct-source:second".into(),
            second.to_string_lossy().into_owned(),
        )]);
        if direct {
            context.insert("direct-primary".into(), "source".into());
            context.insert(
                "direct-source:source".into(),
                workspace.to_string_lossy().into_owned(),
            );
            context.insert(
                "source-path:copied".into(),
                copied.to_string_lossy().into_owned(),
            );
        }
        let workflow=serde_json::from_value(serde_json::json!({"id":"workflow","name":"Outputs","description":"","entry":"task","edges":[],"nodes":[
            {"id":"source","kind":"Source","source_type":"folder","source_mode":if direct {"direct"} else {"copy"},"location":"main"},
            {"id":"second","kind":"Source","source_type":"folder","source_mode":"direct","location":"second"},
            {"id":"copied","kind":"Source","source_type":"examples"},
            {"id":"task","kind":"Task"}]})).unwrap();
        let run = Run {
            id: rid,
            workflow,
            status: "paused".into(),
            current: "task".into(),
            created: now(),
            updated: now(),
            workspace: workspace.to_string_lossy().into_owned(),
            states: BTreeMap::new(),
            sessions: BTreeMap::new(),
            outputs: BTreeMap::new(),
            pending: None,
            active_edge: None,
            pause_requested: false,
            attempts: BTreeMap::new(),
            reworks: BTreeMap::new(),
            context,
            last_task: None,
            last_node: None,
            error: None,
            input: String::new(),
            execution: Default::default(),
            parent: None,
            children: vec![],
        };
        app.store.save_run(1, &run).unwrap();
        (dir, app, run, second, copied)
    }

    #[test]
    fn inventory_reuses_hashes_but_tracks_restored_times_replacements_and_new_files() {
        let (_dir, app, run, _, _) = fixture(true);
        let root = Path::new(&run.workspace);
        for i in 0..1500 {
            std::fs::write(root.join(format!("input-{i}.txt")), vec![b'x'; 16 * 1024]).unwrap();
        }
        // Let the conservative coarse-timestamp safety window expire.
        std::thread::sleep(std::time::Duration::from_millis(2100));
        let started = std::time::Instant::now();
        baseline(&app, 1, &run).unwrap();
        let cold = started.elapsed();
        let cache = app.artifact_tracker.run(1, &run.id);
        assert_eq!(cache.inventory.lock().unwrap().hashed_files, 0);
        let started = std::time::Instant::now();
        snapshot(&app, 1, &run, "unchanged", "", "").unwrap();
        let warm = started.elapsed();
        assert_eq!(
            cache.inventory.lock().unwrap().hashed_files,
            0,
            "unchanged contents must not be read again"
        );
        assert!(app.store.artifacts(1, &run.id).unwrap().is_empty());
        println!("1500-file inventory: cold={cold:?}, warm={warm:?}; warm content reads=0");

        let original = root.join("original.txt");
        let modified = std::fs::metadata(&original).unwrap().modified().unwrap();
        // Same size, deliberately restored mtime: ctime must invalidate the cache.
        std::fs::write(&original, "modified").unwrap();
        File::options()
            .write(true)
            .open(&original)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        // Atomic replacement, also preserving size and mtime, must be discovered.
        let replaced = root.join("input-0.txt");
        let modified = std::fs::metadata(&replaced).unwrap().modified().unwrap();
        let temp = root.join("replacement.tmp");
        std::fs::write(&temp, vec![b'y'; 16 * 1024]).unwrap();
        File::options()
            .write(true)
            .open(&temp)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        std::fs::rename(temp, replaced).unwrap();
        // Generated, hidden and large/binary outputs still belong in the inventory.
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        std::fs::write(root.join("target/debug/output.bin"), b"generated\0binary").unwrap();
        std::fs::write(root.join(".hidden.txt"), "new hidden output").unwrap();
        std::fs::remove_file(root.join("input-1.txt")).unwrap();
        snapshot(&app, 1, &run, "changed", "", "").unwrap();
        let artifacts = app.store.artifacts(1, &run.id).unwrap();
        assert_eq!(artifacts.len(), 4);
        assert_eq!(cache.inventory.lock().unwrap().hashed_files, 4);
        assert!(artifacts.iter().any(|a| a.content == "modified"));
        assert!(
            artifacts
                .iter()
                .any(|a| a.path.ends_with("target/debug/output.bin"))
        );
        // A new process distrusts all cached metadata but preserves version history.
        let reopened = crate::tests::app(&app.data);
        snapshot(&reopened, 1, &run, "restarted", "", "").unwrap();
        assert_eq!(reopened.store.artifacts(1, &run.id).unwrap().len(), 4);
        assert_eq!(
            reopened
                .artifact_tracker
                .run(1, &run.id)
                .inventory
                .lock()
                .unwrap()
                .hashed_files,
            0
        );
    }

    #[test]
    fn metadata_start_and_restart_do_not_open_unchanged_source_contents() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, app, run, _, _) = fixture(true);
        let path = Path::new(&run.workspace).join("unreadable.txt");
        std::fs::write(&path, "must not be opened").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0)).unwrap();
        if unsafe { libc::geteuid() } != 0 {
            assert!(File::open(&path).is_err());
        }
        baseline(&app, 1, &run).unwrap();
        snapshot(&app, 1, &run, "task", "", "").unwrap();
        let reopened = crate::tests::app(&app.data);
        snapshot(&reopened, 1, &run, "next", "", "").unwrap();
        assert!(reopened.store.artifacts(1, &run.id).unwrap().is_empty());
        assert!(!app.data.join("system/artifact-files").exists());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn legacy_hash_baselines_still_capture_changes() {
        let (_dir, app, run, _, _) = fixture(false);
        let initial = inventory(&app, 1, &run).unwrap();
        app.store
            .db
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO artifact_baselines VALUES(?,?,?)",
                params![1, run.id, serde_json::to_string(&initial).unwrap()],
            )
            .unwrap();
        std::fs::write(
            Path::new(&run.workspace).join("original.txt"),
            "legacy changed",
        )
        .unwrap();
        snapshot(&app, 1, &run, "task", "", "").unwrap();
        assert_eq!(
            app.store.artifacts(1, &run.id).unwrap()[0].content,
            "legacy changed"
        );
    }

    #[test]
    fn metadata_recovers_changes_while_notifications_are_unavailable() {
        let (_dir, app, run, _, _) = fixture(true);
        baseline(&app, 1, &run).unwrap();
        let state = app.artifact_tracker.run(1, &run.id);
        state.inventory.lock().unwrap().monitor.take();
        let root = Path::new(&run.workspace);
        std::fs::write(root.join("original.txt"), "changed while not watched").unwrap();
        std::fs::create_dir_all(root.join("new-folder")).unwrap();
        std::fs::write(root.join("new-folder/new.txt"), "new file").unwrap();
        snapshot(&app, 1, &run, "task", "", "").unwrap();
        let artifacts = app.store.artifacts(1, &run.id).unwrap();
        assert_eq!(artifacts.len(), 2);
        assert!(
            artifacts
                .iter()
                .any(|a| a.content == "changed while not watched")
        );
        assert!(artifacts.iter().any(|a| a.content == "new file"));
    }

    #[test]
    fn concurrent_snapshots_do_not_create_duplicate_file_versions() {
        let (_dir, app, run, _, _) = fixture(false);
        baseline(&app, 1, &run).unwrap();
        std::fs::write(Path::new(&run.workspace).join("output.txt"), "output").unwrap();
        std::thread::scope(|scope| {
            for node in ["first", "second"] {
                let (app, run) = (&app, &run);
                scope.spawn(move || snapshot(app, 1, run, node, "", node).unwrap());
            }
        });
        let artifacts = app.store.artifacts(1, &run.id).unwrap();
        assert_eq!(
            artifacts.iter().filter(|a| a.path == "output.txt").count(),
            1
        );
        assert_eq!(artifacts.len(), 3, "both reports are preserved");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancelled_snapshot_is_joined_before_final_snapshot_without_blocking_runtime() {
        let (_dir, app, run, _, _) = fixture(false);
        let cache = app.artifact_tracker.run(1, &run.id);
        let (held, ready) = std::sync::mpsc::channel();
        let (release, released) = std::sync::mpsc::channel();
        let holder = std::thread::spawn(move || {
            let _guard = cache.inventory.lock().unwrap();
            held.send(()).unwrap();
            released
                .recv_timeout(std::time::Duration::from_secs(3))
                .unwrap();
        });
        ready.recv().unwrap();
        let (app2, run2) = (app.clone(), run.clone());
        let scan =
            tokio::spawn(
                async move { snapshot_async(&app2, 1, &run2, "task", "", "report").await },
            );
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        assert!(!scan.is_finished());
        // Stop cancels the caller, not a disk job already submitted. The final
        // snapshot must join it even if that job is still waiting to start.
        let state = app.artifact_tracker.run(1, &run.id);
        assert!(state.gate.try_lock().is_err());
        scan.abort();
        assert!(scan.await.unwrap_err().is_cancelled());
        let (app2, run2) = (app.clone(), run.clone());
        let final_scan = tokio::spawn(async move {
            snapshot_async(&app2, 1, &run2, "final", "", "final report").await
        });
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        assert!(!final_scan.is_finished());
        release.send(()).unwrap();
        final_scan.await.unwrap().unwrap();
        holder.join().unwrap();
        assert_eq!(app.store.artifacts(1, &run.id).unwrap().len(), 2);
    }

    #[test]
    fn artifacts_only_changes_across_direct_copy_and_additional_sources() {
        for direct in [true, false] {
            let (_dir, app, run, second, copied) = fixture(direct);
            baseline(&app, 1, &run).unwrap();
            snapshot(&app, 1, &run, "task", "", "report").unwrap();
            assert_eq!(
                app.store.artifacts(1, &run.id).unwrap().len(),
                1,
                "Unchanged inputs are not artifacts"
            );
            let main = Path::new(&run.workspace);
            std::fs::write(main.join("original.txt"), "updated").unwrap();
            std::fs::write(main.join("new.txt"), "created").unwrap();
            std::fs::create_dir_all(main.join(".hidden")).unwrap();
            std::fs::write(main.join(".hidden/result.md"), "hidden result").unwrap();
            std::fs::write(second.join("original.txt"), "updated second").unwrap();
            std::fs::write(copied.join("binary.pdf"), b"%PDF\0\xffbinary").unwrap();
            let data_only = app
                .data
                .join(format!("users/1/workspaces/{}/output.zip", run.id));
            std::fs::write(&data_only, b"PK\0zip").unwrap();
            snapshot(&app, 1, &run, "task", "", "report 2").unwrap();
            let artifacts = app.store.artifacts(1, &run.id).unwrap();
            let files: Vec<_> = artifacts
                .iter()
                .filter(|a| a.file.as_ref().unwrap().change != "report")
                .collect();
            assert_eq!(files.len(), 6);
            assert!(files.iter().any(|a| a.path.contains(".hidden/result.md")));
            let main_change = files
                .iter()
                .find(|a| {
                    a.file.as_ref().unwrap().location
                        == std::fs::canonicalize(main.join("original.txt"))
                            .unwrap()
                            .to_string_lossy()
                })
                .unwrap();
            assert_eq!(
                main_change.file.as_ref().unwrap().root,
                if direct { "/sources" } else { "/data" }
            );
            assert_eq!(main_change.file.as_ref().unwrap().change, "modified");
            let pdf = files
                .iter()
                .find(|a| a.path.ends_with("binary.pdf"))
                .unwrap();
            assert!(!pdf.file.as_ref().unwrap().preview);
            assert_eq!(pdf.file.as_ref().unwrap().root, "/data");
            assert_eq!(
                std::fs::read(download_path(&app, 1, pdf).unwrap().unwrap()).unwrap(),
                b"%PDF\0\xffbinary"
            );
            std::fs::write(copied.join("binary.pdf"), "changed later").unwrap();
            // A stored version survives modifications and deletion of its working file.
            assert_eq!(
                std::fs::read(download_path(&app, 1, pdf).unwrap().unwrap()).unwrap(),
                b"%PDF\0\xffbinary"
            );
            std::fs::write(main.join("original.txt"), "original").unwrap();
            snapshot(&app, 1, &run, "next", "", "report").unwrap();
            let after = app.store.artifacts(1, &run.id).unwrap();
            let reverted = after
                .iter()
                .rev()
                .find(|a| a.path == main_change.path && a.file.as_ref().unwrap().change != "report")
                .unwrap();
            assert_eq!(reverted.version, 2);
            assert_eq!(reverted.content, "original");
            assert!(app.store.artifact(2, &pdf.id).is_err());
            assert!(blob_path(&app, 1, "../../outside").is_err());
        }
    }

    #[test]
    fn artifacts_large_binary_and_more_than_a_thousand_outputs_survive_restart() {
        let (_dir, app, run, _, _) = fixture(false);
        let root = Path::new(&run.workspace);
        for i in 0..1005 {
            std::fs::write(root.join(format!("input-{i}.txt")), "unchanged").unwrap();
        }
        baseline(&app, 1, &run).unwrap();
        for i in 0..1005 {
            std::fs::write(root.join(format!("output-{i}.txt")), "output").unwrap();
        }
        let large = vec![b'x'; 1024 * 1024 + 1];
        std::fs::write(root.join("large.txt"), &large).unwrap();
        let reopened = crate::tests::app(&app.data);
        snapshot(&reopened, 1, &run, "task", "", "").unwrap();
        let artifacts = reopened.store.artifacts(1, &run.id).unwrap();
        assert_eq!(artifacts.len(), 1006);
        let large_artifact = artifacts.iter().find(|a| a.path == "large.txt").unwrap();
        assert!(!large_artifact.file.as_ref().unwrap().preview);
        assert_eq!(
            std::fs::read(
                download_path(&reopened, 1, large_artifact)
                    .unwrap()
                    .unwrap()
            )
            .unwrap(),
            large
        );
        snapshot(&reopened, 1, &run, "next", "", "").unwrap();
        assert_eq!(reopened.store.artifacts(1, &run.id).unwrap().len(), 1006);
    }

    #[test]
    fn artifacts_legacy_resume_keeps_history_and_filters_only_changes_after_resume() {
        let (_dir, app, run, _, _) = fixture(false);
        let original = Path::new(&run.workspace).join("original.txt");
        let legacy = Artifact {
            id: id(),
            run: run.id.clone(),
            node: "old".into(),
            path: "original.txt".into(),
            version: 4,
            hash: crate::store::hash("old snapshot"),
            content: "old snapshot".into(),
            session: String::new(),
            turn: 1,
            created: 1,
            file: None,
        };
        app.store.add_artifact(1, &legacy).unwrap();
        baseline(&app, 1, &run).unwrap();
        snapshot(&app, 1, &run, "task", "", "").unwrap();
        assert_eq!(app.store.artifacts(1, &run.id).unwrap().len(), 1);
        let info = file_info(&app, 1, &run, &legacy);
        assert_eq!(info.change, "legacy");
        assert_eq!(info.modified, 1000);
        std::fs::write(original, "after resume").unwrap();
        snapshot(&app, 1, &run, "task", "", "").unwrap();
        let artifacts = app.store.artifacts(1, &run.id).unwrap();
        assert_eq!(artifacts.len(), 2);
        assert_eq!(artifacts[1].version, 5);
        assert_eq!(artifacts[1].content, "after resume");
    }

    #[test]
    fn artifacts_inherited_from_direct_run_point_to_new_copy_but_keep_downloads() {
        let (_dir, app, run, _, _) = fixture(true);
        baseline(&app, 1, &run).unwrap();
        std::fs::write(Path::new(&run.workspace).join("output.txt"), "output").unwrap();
        snapshot(&app, 1, &run, "task", "", "").unwrap();
        let mut artifact = app.store.artifacts(1, &run.id).unwrap().remove(0);
        let mut new = run.clone();
        new.id = id();
        let dest = app.data.join(format!("users/1/workspaces/{}", new.id));
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("output.txt"), "output").unwrap();
        new.workspace = dest.to_string_lossy().into_owned();
        new.context.clear();
        let old_modified = artifact.file.as_ref().unwrap().modified;
        rebase(&app, 1, &run, &new, &mut artifact);
        let info = artifact.file.as_ref().unwrap();
        assert_eq!(info.root, "/data");
        assert_eq!(artifact.path, "output.txt");
        assert_eq!(info.modified, old_modified);
        assert_eq!(
            std::fs::read(download_path(&app, 1, &artifact).unwrap().unwrap()).unwrap(),
            b"output"
        );
    }

    #[test]
    fn artifacts_legacy_recovery_preserves_versions_discovers_outputs_and_is_idempotent() {
        for direct in [true, false] {
            let (_dir, app, mut run, _, copied) = fixture(direct);
            run.created = now() - 60;
            run.updated = now();
            app.store.save_run(1, &run).unwrap();
            let root = Path::new(&run.workspace);
            let old =
                std::time::UNIX_EPOCH + std::time::Duration::from_secs((run.created - 10) as u64);
            for location in inventory(&app, 1, &run).unwrap().keys() {
                File::options()
                    .write(true)
                    .open(location)
                    .unwrap()
                    .set_times(std::fs::FileTimes::new().set_modified(old))
                    .unwrap();
            }
            let original = Artifact {
                id: id(),
                run: run.id.clone(),
                node: "task".into(),
                path: "original.txt".into(),
                version: 3,
                hash: crate::store::hash("original"),
                content: "original".into(),
                session: "original-session".into(),
                turn: 1,
                created: run.created,
                file: None,
            };
            app.store.add_artifact(1, &original).unwrap();
            std::fs::write(root.join("original.txt"), "updated document").unwrap();
            std::fs::write(root.join("missing-report.md"), "# Recovered report").unwrap();
            std::fs::write(copied.join("missing-binary.pdf"), b"%PDF\0\xff").unwrap();
            let expected_modified = std::fs::metadata(root.join("original.txt"))
                .unwrap()
                .modified()
                .unwrap()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let (runs, files) = recover_legacy_runs(&app).unwrap();
            assert_eq!(runs, 1);
            assert_eq!(files, 3);
            let artifacts = app.store.artifacts(1, &run.id).unwrap();
            assert_eq!(artifacts.len(), 4);
            assert_eq!(
                app.store.artifact(1, &original.id).unwrap().content,
                "original"
            );
            assert!(app.store.artifact(1, &original.id).unwrap().file.is_none());
            let updated = artifacts.iter().find(|a| a.version == 4).unwrap();
            assert_eq!(updated.content, "updated document");
            let info = updated.file.as_ref().unwrap();
            assert_eq!(info.change, "recovered");
            assert_eq!(info.modified, expected_modified);
            assert_eq!(info.root, if direct { "/sources" } else { "/data" });
            let pdf = artifacts
                .iter()
                .find(|a| a.path.ends_with("missing-binary.pdf"))
                .unwrap();
            assert_eq!(
                std::fs::read(download_path(&app, 1, pdf).unwrap().unwrap()).unwrap(),
                b"%PDF\0\xff"
            );
            assert!(
                !artifacts
                    .iter()
                    .any(|a| a.path.ends_with("copied/original.txt"))
            );
            assert_eq!(recover_legacy_runs(&app).unwrap(), (0, 0));
            let reopened = crate::tests::app(&app.data);
            assert_eq!(recover_legacy_runs(&reopened).unwrap(), (0, 0));
            assert_eq!(app.store.artifacts(1, &run.id).unwrap().len(), 4);
        }
    }

    #[test]
    fn artifacts_recovery_includes_old_failed_runs_that_never_saved_a_snapshot() {
        let (_dir, app, mut run, _, _) = fixture(false);
        run.created = now() - 60;
        run.status = "failed".into();
        app.store.save_run(1, &run).unwrap();
        let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs((run.created - 10) as u64);
        for location in inventory(&app, 1, &run).unwrap().keys() {
            File::options()
                .write(true)
                .open(location)
                .unwrap()
                .set_times(std::fs::FileTimes::new().set_modified(old))
                .unwrap();
        }
        std::fs::write(
            Path::new(&run.workspace).join("partial-result.txt"),
            "partial output",
        )
        .unwrap();
        assert_eq!(recover_legacy_runs(&app).unwrap(), (1, 1));
        let files = app.store.artifacts(1, &run.id).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "partial-result.txt");
    }

    #[test]
    fn artifacts_recovery_skips_new_runs_and_unavailable_workspaces_without_losing_history() {
        let (_dir, app, run, _, _) = fixture(false);
        baseline(&app, 1, &run).unwrap();
        std::fs::write(Path::new(&run.workspace).join("new.txt"), "new").unwrap();
        snapshot(&app, 1, &run, "task", "", "").unwrap();
        assert_eq!(recover_legacy_runs(&app).unwrap(), (0, 0));
        let original = Artifact {
            id: id(),
            run: run.id.clone(),
            node: "task".into(),
            path: "original.txt".into(),
            version: 1,
            hash: crate::store::hash("original"),
            content: "original".into(),
            session: String::new(),
            turn: 1,
            created: 1,
            file: None,
        };
        app.store.add_artifact(1, &original).unwrap();
        std::fs::remove_dir_all(&run.workspace).unwrap();
        assert_eq!(recover_legacy_runs(&app).unwrap(), (0, 0));
        assert_eq!(
            app.store.artifact(1, &original.id).unwrap().content,
            "original"
        );
    }

    #[test]
    fn artifact_preview_limit_changes_apply_to_existing_immutable_files() {
        let (_dir, app, run, _, _) = fixture(false);
        baseline(&app, 1, &run).unwrap();
        let path = Path::new(&run.workspace).join("large.txt");
        std::fs::write(&path, vec![b'x'; 2_000_000]).unwrap();
        snapshot(&app, 1, &run, "task", "", "").unwrap();
        let artifact = app.store.artifacts(1, &run.id).unwrap().remove(0);
        assert!(!artifact.file.as_ref().unwrap().preview);
        assert_eq!(
            text_preview(
                &app,
                1,
                &artifact,
                app.store
                    .settings(1)
                    .unwrap()
                    .file_preview_limit_bytes()
                    .unwrap()
            )
            .unwrap()
            .unwrap()
            .len(),
            2_000_000
        );
        let mut settings = app.store.settings(1).unwrap();
        settings.file_preview_limit_mb = 1;
        app.store.save_settings(1, &settings).unwrap();
        assert!(
            text_preview(
                &app,
                1,
                &artifact,
                settings.file_preview_limit_bytes().unwrap()
            )
            .unwrap()
            .is_none()
        );
        settings.file_preview_limit_mb = 3;
        app.store.save_settings(1, &settings).unwrap();
        std::fs::write(path, "changed working file").unwrap();
        assert_eq!(
            text_preview(
                &app,
                1,
                &artifact,
                settings.file_preview_limit_bytes().unwrap()
            )
            .unwrap()
            .unwrap()
            .len(),
            2_000_000
        );
        assert_eq!(
            app.store.artifact(1, &artifact.id).unwrap().hash,
            artifact.hash
        );
    }

    #[test]
    fn artifacts_ignore_symlink_escapes_and_git_metadata() {
        let (dir, app, run, _, _) = fixture(false);
        baseline(&app, 1, &run).unwrap();
        let root = Path::new(&run.workspace);
        let secret = dir.path().join("private-secret");
        std::fs::write(&secret, "secret").unwrap();
        std::os::unix::fs::symlink(&secret, root.join("escape.txt")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".git/internal"), "git").unwrap();
        snapshot(&app, 1, &run, "task", "", "").unwrap();
        assert!(app.store.artifacts(1, &run.id).unwrap().is_empty());
    }
}
