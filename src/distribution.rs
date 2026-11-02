//! Runtime resources must survive removal of the Cargo build/source directory.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
    process::Command,
};

const BUNDLE: &[u8] = include_bytes!("../distribution/assets.json");

#[derive(Deserialize)]
struct Resource {
    text: String,
    executable: bool,
}

pub fn assets(data: &Path) -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("NODO_ASSETS") {
        return std::fs::canonicalize(path).context("NODO_ASSETS non accessibile");
    }
    extract(data)
}

fn extract(data: &Path) -> Result<PathBuf> {
    let hash = hex::encode(Sha256::digest(BUNDLE));
    let root = data.join("system/assets").join(hash);
    if root.join(".complete").is_file() {
        return Ok(root);
    }
    let files: BTreeMap<String, Resource> = serde_json::from_slice(BUNDLE)?;
    for (name, resource) in files {
        ensure!(
            Path::new(&name)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
            "Invalid bundled path"
        );
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().context("Missing parent")?)?;
        std::fs::write(&path, resource.text)?;
        #[cfg(unix)]
        if resource.executable {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
        }
    }
    std::fs::write(root.join(".complete"), b"ok")?;
    Ok(root)
}

pub fn tools(data: &Path) -> PathBuf {
    data.join("system/tools/bin")
}

pub fn setup(data: &Path, assets: &Path) -> Result<()> {
    let prefix = data.join("system/tools");
    let status = Command::new("npm")
        .args(["install", "--global", "--prefix"])
        .arg(&prefix)
        .args([
            "--no-audit",
            "--no-fund",
            "@openai/codex@0.165.0",
            "@anthropic-ai/claude-code@2.1.312",
            "@earendil-works/pi-coding-agent@1.4.0",
        ])
        .status()
        .context("Installa Node.js 24 (almeno 22.19) e rendi npm disponibile nel PATH")?;
    ensure!(
        status.success(),
        "Installazione degli agenti fallita; ripeti rightcommit setup"
    );
    let status = Command::new("npm")
        .args(["ci", "--omit=dev", "--no-audit", "--no-fund"])
        .current_dir(assets.join("pi-extensions"))
        .status()?;
    ensure!(
        status.success(),
        "Installazione delle estensioni Pi fallita"
    );
    println!("Configurazione completata. Avvia con rightcommit dalla stessa cartella.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn private_tools_are_discovered_by_agent_commands() {
        let data = tempfile::tempdir().unwrap();
        let app = crate::tests::app(data.path());
        let bin = tools(data.path());
        std::fs::create_dir_all(&bin).unwrap();
        let script = bin.join("distribution-test-cli");
        std::fs::write(&script, "#!/bin/sh\nprintf installed-ok").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let home = crate::agents::user_home(&app, 1).unwrap();
        let output = crate::agents::tool(&app, 1, &home, "distribution-test-cli", &[], 5)
            .await
            .unwrap();
        assert_eq!(output, "installed-ok");
    }
    #[test]
    fn installed_resources_are_complete_and_persistent() {
        let data = tempfile::tempdir().unwrap();
        let root = extract(data.path()).unwrap();
        for name in [
            "examples/helpdesk/Cargo.toml",
            "examples/helpdesk/src/lib.rs",
            "examples/helpdesk/.specify/scripts/bash/setup-plan.sh",
            "examples/living-spec/.specify/memory/constitution.md",
            "pi-extensions/package-lock.json",
            "pi-extensions/.npmrc",
        ] {
            assert!(root.join(name).is_file(), "{name}");
        }
        std::fs::write(root.join("pi-extensions/installed-marker"), "keep").unwrap();
        assert_eq!(root, extract(data.path()).unwrap());
        assert!(root.join("pi-extensions/installed-marker").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_ne!(
                std::fs::metadata(
                    root.join("examples/helpdesk/.specify/scripts/bash/setup-plan.sh")
                )
                .unwrap()
                .permissions()
                .mode()
                    & 0o111,
                0
            );
        }
    }
}
