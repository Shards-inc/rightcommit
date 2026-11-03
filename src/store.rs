use crate::model::*;
use aes_gcm::{
    Aes256Gcm, KeyInit,
    aead::{Aead, OsRng, rand_core::RngCore},
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

pub fn hash(s: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(s.as_ref()))
}
pub fn private(path: &Path, dir: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
        path,
        std::fs::Permissions::from_mode(if dir { 0o700 } else { 0o600 }),
    )?;
    Ok(())
}
#[derive(Clone)]
pub struct Store {
    pub db: Arc<Mutex<Connection>>,
    key: Arc<[u8; 32]>,
}
impl Store {
    pub fn open(data: &Path) -> Result<Self> {
        let sys = data.join("system");
        std::fs::create_dir_all(&sys)?;
        private(&sys, true)?;
        let kp = sys.join("master.key");
        let key: [u8; 32] = if kp.exists() {
            std::fs::read(&kp)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("Chiave master non valida"))?
        } else {
            let mut k = [0; 32];
            OsRng.fill_bytes(&mut k);
            std::fs::write(&kp, k)?;
            private(&kp, false)?;
            k
        };
        let db = Connection::open(sys.join("nodo.sqlite"))?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
        CREATE TABLE IF NOT EXISTS finops_catalog(id INTEGER PRIMARY KEY CHECK(id=1),json TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS a2a_configs(user_id INTEGER NOT NULL,kind TEXT NOT NULL,id TEXT NOT NULL,blob BLOB NOT NULL,PRIMARY KEY(user_id,kind,id));
        CREATE TABLE IF NOT EXISTS a2a_replies(task TEXT NOT NULL,message TEXT NOT NULL,PRIMARY KEY(task,message));
        CREATE TABLE IF NOT EXISTS a2a_tasks(id TEXT PRIMARY KEY,user_id INTEGER NOT NULL,workflow TEXT NOT NULL,run TEXT NOT NULL,message TEXT NOT NULL,outputs TEXT NOT NULL DEFAULT '[]',UNIQUE(user_id,workflow,message));
        CREATE TABLE IF NOT EXISTS users(id INTEGER PRIMARY KEY AUTOINCREMENT,email TEXT UNIQUE NOT NULL,password TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS tokens(hash TEXT PRIMARY KEY,user_id INTEGER NOT NULL,expires INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS user_access(user_id INTEGER PRIMARY KEY,status TEXT NOT NULL CHECK(status IN ('active','suspended','removed')));
        CREATE TABLE IF NOT EXISTS settings(user_id INTEGER PRIMARY KEY,blob BLOB NOT NULL);
        CREATE TABLE IF NOT EXISTS workflow_organization(user_id INTEGER PRIMARY KEY,json TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS workflows(id TEXT NOT NULL,user_id INTEGER NOT NULL,json TEXT NOT NULL,PRIMARY KEY(id,user_id));
        CREATE TABLE IF NOT EXISTS manager_chats(id TEXT PRIMARY KEY,user_id INTEGER NOT NULL,scope TEXT NOT NULL,json TEXT NOT NULL,busy INTEGER NOT NULL DEFAULT 0,UNIQUE(user_id,scope));
        UPDATE manager_chats SET busy=0;
        CREATE TABLE IF NOT EXISTS schedules(id TEXT PRIMARY KEY,user_id INTEGER NOT NULL,workflow_id TEXT NOT NULL,json TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS schedules_owner ON schedules(user_id,workflow_id);
        CREATE TABLE IF NOT EXISTS schedule_occurrences(schedule_id TEXT NOT NULL,user_id INTEGER NOT NULL,due INTEGER NOT NULL,run_id TEXT UNIQUE,state TEXT NOT NULL,message TEXT NOT NULL,PRIMARY KEY(schedule_id,due));
        CREATE TABLE IF NOT EXISTS runs(id TEXT PRIMARY KEY,user_id INTEGER NOT NULL,json TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS runs_owner ON runs(user_id);
        CREATE INDEX IF NOT EXISTS runs_workflow ON runs(user_id,json_extract(json,'$.workflow.id'));
        CREATE TABLE IF NOT EXISTS artifacts(id TEXT PRIMARY KEY,user_id INTEGER NOT NULL,run TEXT NOT NULL,node TEXT NOT NULL,json TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS artifacts_run ON artifacts(user_id,run);
        CREATE TABLE IF NOT EXISTS artifact_baselines(user_id INTEGER NOT NULL,run TEXT NOT NULL,json TEXT NOT NULL,PRIMARY KEY(user_id,run));
        CREATE TABLE IF NOT EXISTS artifact_recoveries(user_id INTEGER NOT NULL,run TEXT NOT NULL,json TEXT NOT NULL,PRIMARY KEY(user_id,run));
        CREATE TABLE IF NOT EXISTS decisions(id TEXT PRIMARY KEY,user_id INTEGER NOT NULL,run TEXT NOT NULL,json TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS events(seq INTEGER PRIMARY KEY AUTOINCREMENT,user_id INTEGER NOT NULL,run TEXT NOT NULL,json TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS events_run ON events(user_id,run,seq);")?;
        private(&sys.join("nodo.sqlite"), false)?;
        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            key: Arc::new(key),
        })
    }
    pub fn encrypt(&self, value: &[u8]) -> Result<Vec<u8>> {
        let mut nonce = [0; 12];
        OsRng.fill_bytes(&mut nonce);
        let mut out = nonce.to_vec();
        out.extend(
            Aes256Gcm::new_from_slice(self.key.as_ref())
                .unwrap()
                .encrypt((&nonce).into(), value)
                .map_err(|_| anyhow::anyhow!("Cifratura fallita"))?,
        );
        Ok(out)
    }
    pub fn decrypt(&self, value: &[u8]) -> Result<Vec<u8>> {
        anyhow::ensure!(value.len() >= 28, "Segreto cifrato non valido");
        Aes256Gcm::new_from_slice(self.key.as_ref())
            .unwrap()
            .decrypt(value[..12].into(), &value[12..])
            .map_err(|_| anyhow::anyhow!("Decifratura fallita"))
    }
    pub fn settings(&self, uid: i64) -> Result<Settings> {
        let blob: Option<Vec<u8>> = self
            .db
            .lock()
            .unwrap()
            .query_row("SELECT blob FROM settings WHERE user_id=?", [uid], |r| {
                r.get(0)
            })
            .optional()?;
        blob.map(|b| Ok(serde_json::from_slice(&self.decrypt(&b)?)?))
            .unwrap_or_else(|| Ok(Settings::default()))
    }
    pub fn save_settings(&self, uid: i64, s: &Settings) -> Result<()> {
        let blob = self.encrypt(&serde_json::to_vec(s)?)?;
        self.db.lock().unwrap().execute("INSERT INTO settings VALUES(?,?) ON CONFLICT(user_id) DO UPDATE SET blob=excluded.blob",params![uid,blob])?;
        Ok(())
    }
    pub fn update_settings(
        &self,
        uid: i64,
        edit: impl FnOnce(&mut Settings) -> Result<()>,
    ) -> Result<Settings> {
        let db = self.db.lock().unwrap();
        let blob: Option<Vec<u8>> = db
            .query_row("SELECT blob FROM settings WHERE user_id=?", [uid], |r| {
                r.get(0)
            })
            .optional()?;
        let mut settings = blob
            .map(|b| {
                Ok::<_, anyhow::Error>(serde_json::from_slice::<Settings>(&self.decrypt(&b)?)?)
            })
            .transpose()?
            .unwrap_or_default();
        edit(&mut settings)?;
        let blob = self.encrypt(&serde_json::to_vec(&settings)?)?;
        db.execute("INSERT INTO settings VALUES(?,?) ON CONFLICT(user_id) DO UPDATE SET blob=excluded.blob",params![uid,blob])?;
        Ok(settings)
    }
    pub fn workflow(&self, uid: i64, id: &str) -> Result<Workflow> {
        let raw: Option<String> = self
            .db
            .lock()
            .unwrap()
            .query_row(
                "SELECT json FROM workflows WHERE user_id=? AND id=?",
                params![uid, id],
                |r| r.get(0),
            )
            .optional()?;
        serde_json::from_str(&raw.context("Workflow non trovato")?).context("Workflow non valido")
    }
    pub fn workflows(&self, uid: i64) -> Result<Vec<Workflow>> {
        self.json_rows(
            "SELECT json FROM workflows WHERE user_id=? ORDER BY COALESCE(json_extract(json,'$.created'),0) DESC, rowid DESC",
            params![uid],
        )
    }
    pub fn save_workflow(&self, uid: i64, w: &Workflow) -> Result<()> {
        let mut w = w.clone();
        w.created = now();
        self.db.lock().unwrap().execute("INSERT INTO workflows VALUES(?,?,?) ON CONFLICT(id,user_id) DO UPDATE SET json=json_set(excluded.json,'$.created',COALESCE(json_extract(workflows.json,'$.created'),0))",params![w.id,uid,serde_json::to_string(&w)?])?;
        Ok(())
    }
    pub fn rename_workflow(&self, uid: i64, id: &str, name: &str) -> Result<Workflow> {
        let name = name.trim();
        anyhow::ensure!(
            !name.is_empty() && name.chars().count() <= 160 && !name.chars().any(char::is_control),
            "Il nome deve contenere da 1 a 160 caratteri e una sola riga"
        );
        let changed = self.db.lock().unwrap().execute(
            "UPDATE workflows SET json=json_set(json,'$.name',?) WHERE user_id=? AND id=?",
            params![name, uid, id],
        )?;
        anyhow::ensure!(changed == 1, "Workflow non trovato");
        self.workflow(uid, id)
    }
    pub fn run(&self, uid: i64, id: &str) -> Result<Run> {
        let raw: Option<String> = self
            .db
            .lock()
            .unwrap()
            .query_row(
                "SELECT json FROM runs WHERE user_id=? AND id=?",
                params![uid, id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(serde_json::from_str(
            &raw.context("Esecuzione non trovata")?,
        )?)
    }
    pub fn runs(&self, uid: i64) -> Result<Vec<Run>> {
        self.json_rows(
            "SELECT json FROM runs WHERE user_id=? ORDER BY rowid DESC LIMIT 100",
            params![uid],
        )
    }
    pub fn workflow_runs(&self, uid: i64, workflow: &str) -> Result<Vec<Run>> {
        self.json_rows(
            "SELECT json FROM runs WHERE user_id=? AND json_extract(json,'$.workflow.id')=? ORDER BY rowid DESC LIMIT 100",
            params![uid,workflow],
        )
    }
    pub fn save_run(&self, uid: i64, r: &Run) -> Result<()> {
        self.db.lock().unwrap().execute("INSERT INTO runs VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET json=CASE WHEN json_extract(runs.json,'$.pause_requested')=1 AND json_extract(excluded.json,'$.status') IN ('running','waiting_child','waiting_review','waiting_user') THEN json_set(excluded.json,'$.pause_requested',json('true')) ELSE excluded.json END WHERE runs.user_id=excluded.user_id",params![r.id,uid,serde_json::to_string(r)?])?;
        Ok(())
    }
    pub fn artifact(&self, uid: i64, id: &str) -> Result<Artifact> {
        let raw: Option<String> = self
            .db
            .lock()
            .unwrap()
            .query_row(
                "SELECT json FROM artifacts WHERE user_id=? AND id=?",
                params![uid, id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(serde_json::from_str(
            &raw.context("Artefatto non trovato")?,
        )?)
    }
    pub fn artifacts(&self, uid: i64, run: &str) -> Result<Vec<Artifact>> {
        self.json_rows(
            "SELECT json FROM artifacts WHERE user_id=? AND run=? ORDER BY rowid",
            params![uid, run],
        )
    }
    pub fn add_artifact(&self, uid: i64, a: &Artifact) -> Result<()> {
        self.db.lock().unwrap().execute(
            "INSERT INTO artifacts VALUES(?,?,?,?,?)",
            params![a.id, uid, a.run, a.node, serde_json::to_string(a)?],
        )?;
        Ok(())
    }
    pub fn decisions(&self, uid: i64, run: &str) -> Result<Vec<Decision>> {
        self.json_rows(
            "SELECT json FROM decisions WHERE user_id=? AND run=? ORDER BY rowid",
            params![uid, run],
        )
    }
    pub fn add_decision(&self, uid: i64, d: &Decision) -> Result<()> {
        self.db.lock().unwrap().execute(
            "INSERT INTO decisions VALUES(?,?,?,?)",
            params![d.id, uid, d.run, serde_json::to_string(d)?],
        )?;
        Ok(())
    }
    pub fn checkpoint_decision(
        &self,
        uid: i64,
        r: &Run,
        d: &Decision,
        pending_id: &str,
    ) -> Result<()> {
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        let actual: Option<String>=tx.query_row("SELECT CASE WHEN json_type(json,'$.pending')='object' THEN COALESCE(json_extract(json,'$.pending.id'),'') ELSE NULL END FROM runs WHERE user_id=? AND id=?",params![uid,r.id],|row|row.get(0))?;
        anyhow::ensure!(
            actual.as_deref() == Some(pending_id),
            "La richiesta è già stata risolta o è cambiata"
        );
        tx.execute(
            "INSERT INTO decisions VALUES(?,?,?,?) ON CONFLICT(id) DO NOTHING",
            params![d.id, uid, d.run, serde_json::to_string(d)?],
        )?;
        let changed=tx.execute("UPDATE runs SET json=CASE WHEN json_extract(json,'$.pause_requested')=1 THEN json_set(?,'$.pause_requested',json('true')) ELSE ? END WHERE user_id=? AND id=?", params![serde_json::to_string(r)?,serde_json::to_string(r)?,uid,r.id])?;
        anyhow::ensure!(changed == 1, "Run inesistente");
        tx.commit()?;
        Ok(())
    }
    pub fn event(&self, uid: i64, run: &str, kind: &str, node: &str, text: &str) -> Result<Event> {
        let mut e = Event {
            seq: 0,
            run: run.into(),
            kind: kind.into(),
            node: node.into(),
            text: text.chars().take(12000).collect(),
            created: now(),
        };
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO events(user_id,run,json) VALUES(?,?,?)",
            params![uid, run, serde_json::to_string(&e)?],
        )?;
        e.seq = db.last_insert_rowid();
        Ok(e)
    }
    pub fn events(&self, uid: i64, run: &str, after: i64) -> Result<Vec<Event>> {
        let db = self.db.lock().unwrap();
        let mut q=db.prepare("SELECT seq,json FROM events WHERE user_id=? AND run=? AND seq>? ORDER BY seq LIMIT 500")?;
        q.query_map(params![uid, run, after], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?
        .map(|r| {
            let (seq, j) = r?;
            let mut e: Event = serde_json::from_str(&j)?;
            e.seq = seq;
            Ok(e)
        })
        .collect()
    }
    pub fn interrupt_running(&self) -> Result<()> {
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        let rows: Vec<(String, i64, String)> = {
            let mut q = tx.prepare("SELECT id,user_id,json FROM runs")?;
            q.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<Result<_, _>>()?
        };
        for (id, uid, raw) in rows {
            let mut r: Run = serde_json::from_str(&raw)?;
            let active =
                ["running", "starting", "queued", "waiting_child"].contains(&r.status.as_str());
            // Also repair interrupted runs written by older servers with stale node states.
            let stale = r.status == "interrupted"
                && (r.active_edge.is_some()
                    || r.states.values().any(|s| {
                        ["running", "starting", "waiting_child", "waiting_external"]
                            .contains(&s.as_str())
                    }));
            if active || stale {
                r.status = "interrupted".into();
                r.updated = now();
                r.active_edge = None;
                for state in r.states.values_mut() {
                    if ["running", "starting", "waiting_child", "waiting_external"]
                        .contains(&state.as_str())
                    {
                        *state = "interrupted".into();
                    }
                }
                r.error=Some("Server riavviato: verifica gli effetti dell'ultimo turno prima di riprenderlo nella stessa sessione.".into());
                tx.execute(
                    "UPDATE runs SET json=? WHERE id=?",
                    params![serde_json::to_string(&r)?, id],
                )?;
                for (kind, text) in [
                    ("restart", r.error.as_deref().unwrap()),
                    ("state", "interrupted"),
                ] {
                    let event = Event {
                        seq: 0,
                        run: id.clone(),
                        kind: kind.into(),
                        node: r.current.clone(),
                        text: text.into(),
                        created: r.updated,
                    };
                    tx.execute(
                        "INSERT INTO events(user_id,run,json) VALUES(?,?,?)",
                        params![uid, id, serde_json::to_string(&event)?],
                    )?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }
    fn json_rows<T: serde::de::DeserializeOwned>(
        &self,
        sql: &str,
        p: impl rusqlite::Params,
    ) -> Result<Vec<T>> {
        let db = self.db.lock().unwrap();
        let mut q = db.prepare(sql)?;
        q.query_map(p, |r| r.get::<_, String>(0))?
            .map(|r| Ok(serde_json::from_str(&r?)?))
            .collect()
    }
}
