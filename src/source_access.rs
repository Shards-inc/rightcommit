use crate::{App, model::*};
use anyhow::{Result, ensure};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub fn direct(n: &Node) -> bool {
    n.kind == "Source" && n.source_type == "folder" && n.source_mode == "direct"
}

pub fn paths(context: &BTreeMap<String, String>) -> Vec<PathBuf> {
    context
        .iter()
        .filter(|(k, _)| k.starts_with("direct-source:"))
        .map(|(_, p)| PathBuf::from(p))
        .collect()
}

pub fn source_path(r: &Run, index: usize, n: &Node) -> String {
    r.context
        .get(&format!("direct-source:{}", n.id))
        .or_else(|| r.context.get(&format!("source-path:{}", n.id)))
        .cloned()
        .unwrap_or_else(|| {
            if index == 0 {
                ".".into()
            } else {
                format!(".nodo/sources/{}", n.id)
            }
        })
}

pub fn validate_tree(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    ensure!(
        !metadata.is_symlink(),
        "Link simbolici non supportati: {}",
        path.display()
    );
    if metadata.is_dir() {
        for e in std::fs::read_dir(path)? {
            validate_tree(&e?.path())?;
        }
    } else {
        ensure!(
            metadata.is_file(),
            "File Source non supportato: {}",
            path.display()
        );
    }
    Ok(())
}

pub fn validate_bindings(app: &App, r: &Run) -> Result<()> {
    for (key, path) in r
        .context
        .iter()
        .filter(|(k, _)| k.starts_with("direct-source:"))
    {
        let id = key.trim_start_matches("direct-source:");
        let n = r
            .workflow
            .nodes
            .iter()
            .find(|n| n.id == id && direct(n))
            .ok_or_else(|| anyhow::anyhow!("Source diretta non valida"))?;
        ensure!(
            crate::storage::source_folder(app, "sources", &n.location)? == Path::new(path),
            "Percorso Source modificato"
        );
    }
    Ok(())
}

pub fn available(app: &App, requested: &[PathBuf], excluded: &[String]) -> Result<()> {
    if requested.is_empty() {
        return Ok(());
    }
    let db = app.store.db.lock().unwrap();
    let mut statement = db.prepare("SELECT json FROM runs")?;
    for row in statement.query_map([], |row| row.get::<_, String>(0))? {
        let other: Run = serde_json::from_str(&row?)?;
        if excluded.contains(&other.id)
            || ![
                "starting",
                "running",
                "waiting_child",
                "waiting_external",
                "waiting_review",
                "waiting_user",
            ]
            .contains(&other.status.as_str())
        {
            continue;
        }
        for existing in paths(&other.context) {
            ensure!(
                !requested
                    .iter()
                    .any(|p| p.starts_with(&existing) || existing.starts_with(p)),
                "Una run sta usando questa cartella sources o una cartella sovrapposta. Completa o metti in pausa quella run."
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder_workflow(mode: &str) -> Workflow {
        let mut w = crate::presets::all().remove(0);
        let n = w.nodes.iter_mut().find(|n| n.kind == "Source").unwrap();
        n.source_type = "folder".into();
        n.source_mode = mode.into();
        n.location = "material".into();
        w
    }

    #[tokio::test]
    async fn direct_source_is_the_real_folder_and_survives_workflow_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let original = app.sources.join("material");
        std::fs::create_dir_all(original.join("data")).unwrap();
        std::fs::File::create(original.join("data/large.bin"))
            .unwrap()
            .set_len(11 * 1024 * 1024)
            .unwrap();
        std::fs::write(original.join("notes.txt"), "before").unwrap();
        let w = folder_workflow("direct");
        app.store.save_workflow(1, &w).unwrap();
        let r = crate::engine::create(&app, 1, w.clone(), "test".into(), None)
            .await
            .unwrap();
        assert_eq!(
            PathBuf::from(&r.workspace),
            std::fs::canonicalize(&original).unwrap()
        );
        assert_eq!(
            crate::subworkflows::workspace(&app, 1, &r).unwrap(),
            std::fs::canonicalize(&original).unwrap()
        );
        assert!(
            !original.join(".git").exists(),
            "Do not initialize Git in original material"
        );
        std::fs::write(Path::new(&r.workspace).join("notes.txt"), "after").unwrap();
        assert_eq!(
            std::fs::read_to_string(original.join("notes.txt")).unwrap(),
            "after"
        );
        crate::deletion::perform(&app, 1, &w.id, &[r.id]).unwrap();
        assert_eq!(
            std::fs::read_to_string(original.join("notes.txt")).unwrap(),
            "after"
        );
        assert_eq!(
            std::fs::metadata(original.join("data/large.bin"))
                .unwrap()
                .len(),
            11 * 1024 * 1024
        );
    }

    #[tokio::test]
    async fn legacy_folders_copy_and_total_over_ten_mib_is_not_a_hard_folder_limit() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let original = app.sources.join("material");
        std::fs::create_dir_all(original.join("data")).unwrap();
        for name in ["one.bin", "two.bin"] {
            std::fs::File::create(original.join("data").join(name))
                .unwrap()
                .set_len(6 * 1024 * 1024)
                .unwrap();
        }
        assert_eq!(
            crate::agents::source_copy_size(
                &original,
                Settings::default().source_copy_limit_bytes().unwrap()
            )
            .unwrap(),
            12 * 1024 * 1024
        );
        let r = crate::engine::create(&app, 1, folder_workflow(""), "test".into(), None)
            .await
            .unwrap();
        assert_ne!(PathBuf::from(&r.workspace), original);
        assert_eq!(
            std::fs::metadata(Path::new(&r.workspace).join("data/one.bin"))
                .unwrap()
                .len(),
            6 * 1024 * 1024
        );
        std::fs::write(Path::new(&r.workspace).join("new.txt"), "copy only").unwrap();
        assert!(!original.join("new.txt").exists());
    }

    #[tokio::test]
    async fn direct_source_excludes_overlapping_active_runs_across_users() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let original = app.sources.join("material");
        std::fs::create_dir_all(original.join("nested")).unwrap();
        std::fs::write(original.join("notes.txt"), "test").unwrap();
        let mut first =
            crate::engine::create(&app, 1, folder_workflow("direct"), "test".into(), None)
                .await
                .unwrap();
        first.status = "running".into();
        app.store.save_run(1, &first).unwrap();
        let mut w = folder_workflow("direct");
        w.nodes
            .iter_mut()
            .find(|n| n.kind == "Source")
            .unwrap()
            .location = "material/nested".into();
        assert!(
            crate::engine::create(&app, 2, w.clone(), "test".into(), None)
                .await
                .unwrap_err()
                .to_string()
                .contains("sovrapposta")
        );
        first.status = "paused".into();
        app.store.save_run(1, &first).unwrap();
        let mut second = crate::engine::create(&app, 2, w, "test".into(), None)
            .await
            .unwrap();
        second.status = "starting".into();
        app.store.save_run(2, &second).unwrap();
        assert!(crate::subworkflows::can_launch(&app, 1, &first).is_err());
    }

    #[tokio::test]
    async fn additional_direct_source_uses_its_actual_path() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let original = app.sources.join("material");
        std::fs::create_dir_all(&original).unwrap();
        std::fs::File::create(original.join("large.bin"))
            .unwrap()
            .set_len(11 * 1024 * 1024)
            .unwrap();
        let mut w = crate::presets::all().remove(0);
        let mut n = folder_workflow("direct")
            .nodes
            .into_iter()
            .find(|n| n.kind == "Source")
            .unwrap();
        n.id = "additional".into();
        w.edges.push(Edge {
            from: n.id.clone(),
            to: w
                .nodes
                .iter()
                .find(|n| n.kind == "Agent")
                .unwrap()
                .id
                .clone(),
            kind: "context".into(),
            outcome: String::new(),
        });
        w.nodes.push(n.clone());
        app.store.save_workflow(1, &w).unwrap();
        let r = crate::engine::create(&app, 1, w.clone(), "test".into(), None)
            .await
            .unwrap();
        assert_eq!(
            source_path(&r, 1, &n),
            std::fs::canonicalize(&original).unwrap().to_string_lossy()
        );
        assert!(
            !Path::new(&r.workspace)
                .join(".nodo/sources/additional")
                .exists()
        );
        crate::subworkflows::workspace(&app, 1, &r).unwrap();
        crate::deletion::perform(&app, 1, &w.id, std::slice::from_ref(&r.id)).unwrap();
        assert!(original.join("large.bin").exists());
        assert!(!Path::new(&r.workspace).exists());
    }

    #[tokio::test]
    async fn permanent_primary_keeps_additional_copies_private_and_repeatable() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let original = app.sources.join("material");
        std::fs::create_dir_all(&original).unwrap();
        std::fs::write(original.join("notes.txt"), "original").unwrap();
        let mut w = folder_workflow("direct");
        let n = Node {
            id: "extra".into(),
            kind: "Source".into(),
            title: "Example".into(),
            source_type: "example".into(),
            ..Default::default()
        };
        w.edges.push(Edge {
            from: n.id.clone(),
            to: w
                .nodes
                .iter()
                .find(|n| n.kind == "Agent")
                .unwrap()
                .id
                .clone(),
            kind: "context".into(),
            outcome: String::new(),
        });
        w.nodes.push(n.clone());
        app.store.save_workflow(1, &w).unwrap();
        let first = crate::engine::create(&app, 1, w.clone(), "first".into(), None)
            .await
            .unwrap();
        let second = crate::engine::create(&app, 1, w.clone(), "second".into(), None)
            .await
            .unwrap();
        let extra = PathBuf::from(source_path(&first, 1, &n));
        assert!(extra.join("Cargo.toml").exists());
        assert_ne!(source_path(&first, 1, &n), source_path(&second, 1, &n));
        assert!(!original.join(".nodo/sources").exists());
        crate::deletion::perform(&app, 1, &w.id, &[first.id, second.id]).unwrap();
        assert!(!extra.exists());
        assert_eq!(
            std::fs::read_to_string(original.join("notes.txt")).unwrap(),
            "original"
        );
    }

    #[test]
    fn remote_repositories_never_use_direct_mode() {
        let n = Node {
            kind: "Source".into(),
            source_type: "git".into(),
            source_mode: "direct".into(),
            ..Default::default()
        };
        assert!(!direct(&n));
    }
}
