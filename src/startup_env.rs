use anyhow::{Result, anyhow, ensure};
use std::path::Path;

fn read(path: &Path) -> Result<Vec<(String, String)>> {
    let entries = match dotenvy::from_path_iter(path) {
        Ok(entries) => entries,
        Err(error) if error.not_found() => return Ok(Vec::new()),
        // dotenvy parse errors can contain an entire line, including a secret value.
        Err(_) => return Err(anyhow!("Impossibile leggere .env nella cartella di avvio.")),
    };
    let values = entries
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| {
            anyhow!(
                "File .env non valido: controlla la sintassi. I valori non sono stati caricati."
            )
        })?;
    for (name, value) in &values {
        ensure!(
            !name.is_empty() && !name.contains(['=', '\0']) && !value.contains('\0'),
            "File .env non valido: controlla la sintassi. I valori non sono stati caricati."
        );
    }
    Ok(values)
}

/// Load exactly the startup directory's .env; never search a parent directory.
///
/// # Safety
/// Call only from the synchronous entry point, before starting the async runtime or any threads.
/// Process environment mutation is not safe while other threads can read it.
pub(crate) unsafe fn load() -> Result<()> {
    for (name, value) in read(Path::new(".env"))? {
        // Empty entries in .env.example are optional, not overrides of platform defaults
        // (in particular, empty SSL_CERT_FILE/SSL_CERT_DIR would disable native trust roots).
        if !value.is_empty() && std::env::var_os(&name).is_none() {
            // SAFETY: guaranteed by the caller; read() validates names and values first.
            unsafe {
                std::env::set_var(name, value);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_file_and_standard_dotenv_syntax() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read(&dir.path().join("missing.env")).unwrap().is_empty());
        let path = dir.path().join(".env");
        std::fs::write(&path,"# Provider configuration\nexport RIGHTCOMMIT_TEST_KEY='dummy # literal'\nRIGHTCOMMIT_TEST_BLANK=\nRIGHTCOMMIT_TEST_QUOTED=\"hello world\" # comment\nRIGHTCOMMIT_TEST_ROOT=local\nRIGHTCOMMIT_TEST_EXPANDED=${RIGHTCOMMIT_TEST_ROOT}/models\nRIGHTCOMMIT_TEST_LITERAL='$(touch should-not-run)'\n").unwrap();
        let values = read(&path)
            .unwrap()
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(values["RIGHTCOMMIT_TEST_KEY"], "dummy # literal");
        assert_eq!(values["RIGHTCOMMIT_TEST_BLANK"], "");
        assert_eq!(values["RIGHTCOMMIT_TEST_QUOTED"], "hello world");
        assert_eq!(values["RIGHTCOMMIT_TEST_EXPANDED"], "local/models");
        assert_eq!(
            values["RIGHTCOMMIT_TEST_LITERAL"],
            "$(touch should-not-run)"
        );
    }
    #[test]
    fn malformed_files_fail_before_loading_and_errors_do_not_contain_secrets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        std::fs::write(
            &path,
            "OPENAI_API_KEY=valid-dummy-key\nXAI_API_KEY=\"private-key-must-not-appear\n",
        )
        .unwrap();
        let error = read(&path).unwrap_err();
        assert!(error.to_string().contains("File .env non valido"));
        assert!(!format!("{error:?}").contains("private-key"));
        assert!(!format!("{error:?}").contains("valid-dummy-key"));
        std::fs::write(&path, b"OPENAI_API_KEY=bad\0value\n").unwrap();
        assert!(read(&path).is_err());
    }
}
