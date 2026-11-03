//! Persistent local schedules. Occurrences reserve their run id before preparing any files.
use crate::{
    App,
    api::StartRun,
    auth::{ApiResult, bad},
    model::*,
};
use anyhow::{Context, Result, ensure};
use axum::{
    Json,
    extract::{Path, State},
};
use chrono::{Datelike, Duration, LocalResult, NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub workflow_id: String,
    #[serde(default)]
    pub locale: Option<String>,
    pub name: String,
    pub kind: String,
    pub start: String, // local YYYY-MM-DDTHH:MM
    pub end: Option<String>,
    pub timezone: String,
    #[serde(default = "one")]
    pub every: u32,
    #[serde(default)]
    pub weekdays: Vec<u32>, // Monday=0
    #[serde(default)]
    pub input: String,
    pub execution: Option<ExecutionPolicy>,
    pub base_run: Option<String>,
    #[serde(default)]
    pub allow_large_source_copy: bool,
    #[serde(default = "yes")]
    pub active: bool,
}
fn one() -> u32 {
    1
}
fn yes() -> bool {
    true
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Schedule {
    pub id: String,
    pub config: Config,
    pub next: Option<i64>,
    pub created: i64,
}
fn local(value: &str) -> Result<NaiveDateTime> {
    Ok(NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M").context("Data o ora non valida")?)
}
fn instant(tz: Tz, value: NaiveDateTime) -> Option<i64> {
    match tz.from_local_datetime(&value) {
        LocalResult::Single(t) => Some(t.timestamp()),
        LocalResult::Ambiguous(a, b) => Some(a.timestamp().min(b.timestamp())),
        LocalResult::None => None,
    }
}
impl Config {
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.name.trim().is_empty() && self.name.len() <= 160,
            "Inserisci un nome valido (massimo 160 caratteri)"
        );
        ensure!(
            ["once", "minutes", "hours", "daily", "weekly"].contains(&self.kind.as_str()),
            "Frequenza non valida"
        );
        ensure!((1..=10000).contains(&self.every), "Intervallo non valido");
        ensure!(
            self.weekdays.iter().all(|d| *d < 7)
                && (self.kind != "weekly" || !self.weekdays.is_empty()),
            "Seleziona almeno un giorno valido"
        );
        let tz: Tz = self.timezone.parse().context("Fuso orario non valido")?;
        let start = local(&self.start)?;
        ensure!(
            instant(tz, start).is_some(),
            "L'orario iniziale non esiste nel fuso selezionato"
        );
        if let Some(end) = &self.end {
            let end = local(end)?;
            ensure!(
                end >= start && instant(tz, end).is_some(),
                "Fine periodo non valida"
            );
        }
        if let Some(policy) = &self.execution {
            crate::supervision::validate(policy)?;
        }
        Ok(())
    }
    pub fn next_after(&self, after: i64) -> Result<Option<i64>> {
        let tz: Tz = self.timezone.parse().context("Fuso orario non valido")?;
        let start = local(&self.start)?;
        let first = instant(tz, start).context("Orario iniziale non valido")?;
        let end = self
            .end
            .as_ref()
            .map(|v| {
                Ok::<_, anyhow::Error>(instant(tz, local(v)?).context("Fine periodo non valida")?)
            })
            .transpose()?;
        let candidate = match self.kind.as_str() {
            "once" => (first > after).then_some(first),
            "minutes" | "hours" => {
                let step = i64::from(self.every) * if self.kind == "hours" { 3600 } else { 60 };
                ensure!(step > 0, "Intervallo non valido");
                Some(if after < first {
                    first
                } else {
                    first + ((after - first) / step + 1) * step
                })
            }
            "daily" | "weekly" => {
                let date = chrono::DateTime::from_timestamp(after, 0)
                    .context("Data fuori intervallo")?
                    .with_timezone(&tz)
                    .date_naive()
                    .max(start.date());
                let mut found = None;
                // Two weeks cover a weekly occurrence skipped by a DST gap.
                for n in 0..15 {
                    let date = date + Duration::days(n);
                    if self.kind == "weekly"
                        && !self
                            .weekdays
                            .contains(&date.weekday().num_days_from_monday())
                    {
                        continue;
                    }
                    if let Some(at) = instant(tz, date.and_time(start.time())) {
                        if at >= first && at > after {
                            found = Some(at);
                            break;
                        }
                    }
                }
                found
            }
            _ => anyhow::bail!("Frequenza non valida"),
        };
        Ok(candidate.filter(|t| end.is_none_or(|e| *t <= e)))
    }
}
fn all(app: &App, uid: Option<i64>) -> Result<Vec<(i64, Schedule)>> {
    let db = app.store.db.lock().unwrap();
    let mut q = db.prepare(
        "SELECT user_id,json FROM schedules WHERE (?1 IS NULL OR user_id=?1) ORDER BY rowid DESC",
    )?;
    q.query_map([uid], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .map(|r| {
            let (uid, s) = r?;
            Ok((uid, serde_json::from_str(&s)?))
        })
        .collect()
}
fn get(app: &App, uid: i64, id: &str) -> Result<Schedule> {
    let raw: Option<String> = app
        .store
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT json FROM schedules WHERE user_id=? AND id=?",
            params![uid, id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(serde_json::from_str(
        &raw.context("Pianificazione non trovata")?,
    )?)
}
fn save(app: &App, uid: i64, s: &Schedule) -> Result<()> {
    app.store.db.lock().unwrap().execute(
        "UPDATE schedules SET json=? WHERE user_id=? AND id=?",
        params![serde_json::to_string(s)?, uid, s.id],
    )?;
    Ok(())
}
pub fn run_schedule(app: &App, uid: i64, rid: &str) -> Result<Option<String>> {
    Ok(app.store.db.lock().unwrap().query_row("WITH RECURSIVE lineage(id,parent) AS (SELECT id,json_extract(json,'$.parent.run') FROM runs WHERE user_id=?1 AND id=?2 UNION ALL SELECT r.id,json_extract(r.json,'$.parent.run') FROM runs r JOIN lineage l ON r.id=l.parent WHERE r.user_id=?1) SELECT o.schedule_id FROM schedule_occurrences o JOIN lineage l ON o.run_id=l.id WHERE o.user_id=?1 LIMIT 1",params![uid,rid],|r|r.get(0)).optional()?)
}
pub fn workflow_count(app: &App, uid: i64, wid: &str) -> Result<i64> {
    Ok(app.store.db.lock().unwrap().query_row(
        "SELECT COUNT(*) FROM schedules WHERE user_id=? AND workflow_id=?",
        params![uid, wid],
        |r| r.get(0),
    )?)
}
fn entries(app: &App, uid: i64, id: &str, limit: i64) -> Result<Vec<Value>> {
    let rows: Vec<(i64, Option<String>, String, String)> = {
        let db = app.store.db.lock().unwrap();
        let mut q=db.prepare("SELECT due,run_id,state,message FROM schedule_occurrences WHERE user_id=? AND schedule_id=? ORDER BY due DESC LIMIT ?")?;
        q.query_map(params![uid, id, limit], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<std::result::Result<_, _>>()?
    };
    Ok(rows.into_iter().map(|(due,rid,state,message)|{
        let run=rid.as_ref().and_then(|id|app.store.run(uid,id).ok());
        json!({"due":due,"run_id":run.as_ref().map(|r|&r.id),"state":if state=="error" {state.as_str()} else {run.as_ref().map(|r|r.status.as_str()).unwrap_or(&state)},"message":message})
    }).collect())
}
fn current(app: &App, uid: i64, id: &str) -> Result<Option<Run>> {
    let raw:Option<String>=app.store.db.lock().unwrap().query_row(
        "SELECT r.json FROM schedule_occurrences o JOIN runs r ON r.id=o.run_id AND r.user_id=o.user_id WHERE o.user_id=? AND o.schedule_id=? AND json_extract(r.json,'$.status') NOT IN ('completed','failed','interrupted') ORDER BY o.due DESC LIMIT 1",
        params![uid,id],|r|r.get(0)).optional()?;
    raw.map(|s| Ok(serde_json::from_str(&s)?)).transpose()
}
fn needs_attention(app: &App, uid: i64, run: &Run) -> Result<bool> {
    let mut r = run.clone();
    for _ in 0..crate::subworkflows::MAX_DEPTH {
        if r.pending.is_some()
            || ["paused", "waiting_review", "waiting_user"].contains(&r.status.as_str())
        {
            return Ok(true);
        }
        let Some(call) = r
            .children
            .iter()
            .rev()
            .find(|c| c.node == r.current && Some(&c.attempt) == r.attempts.get(&c.node))
        else {
            break;
        };
        r = app.store.run(uid, &call.run)?;
    }
    Ok(false)
}
pub async fn list(State(app): State<App>, u: User) -> ApiResult<Json<Value>> {
    let mut out = Vec::new();
    for (_, s) in all(&app, Some(u.id))? {
        let history = entries(&app, u.id, &s.id, 1)?;
        let run = current(&app, u.id, &s.id)?;
        let attention = run
            .as_ref()
            .map(|r| needs_attention(&app, u.id, r))
            .transpose()?
            .unwrap_or(false);
        out.push(json!({"schedule":s,"workflow_name":app.store.workflow(u.id,&s.config.workflow_id).ok().map(|w|w.name),"last":history.first(),"current":run.map(|r|json!({"id":r.id,"status":r.status,"needs_attention":attention}))}));
    }
    Ok(Json(json!(out)))
}
pub async fn history(
    State(app): State<App>,
    u: User,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    get(&app, u.id, &id)?;
    Ok(Json(json!(entries(&app, u.id, &id, -1)?)))
}
fn validate_workflow(app: &App, uid: i64, c: &Config) -> Result<()> {
    c.validate()?;
    crate::auth::ensure_active(app, uid)?;
    crate::subworkflows::validate_tree(app, uid, &app.store.workflow(uid, &c.workflow_id)?)?;
    if let Some(id) = &c.base_run {
        ensure!(
            app.store.run(uid, id)?.status == "completed",
            "Repository di base non disponibile"
        );
    }
    Ok(())
}
pub async fn preview(
    State(_app): State<App>,
    _u: User,
    Json(c): Json<Config>,
) -> ApiResult<Json<Value>> {
    c.validate()?;
    let mut times = Vec::new();
    let mut after = now();
    for _ in 0..5 {
        if let Some(at) = c.next_after(after)? {
            times.push(at);
            after = at;
        } else {
            break;
        }
    }
    Ok(Json(json!(times)))
}
pub async fn create(
    State(app): State<App>,
    u: User,
    Json(c): Json<Config>,
) -> ApiResult<Json<Schedule>> {
    let _guard = app.lifecycle.write().await;
    validate_workflow(&app, u.id, &c)?;
    let next = c.next_after(now())?;
    ensure_next(next)?;
    let s = Schedule {
        id: id(),
        config: c,
        next,
        created: now(),
    };
    app.store
        .db
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO schedules VALUES(?,?,?,?)",
            params![
                s.id,
                u.id,
                s.config.workflow_id,
                serde_json::to_string(&s).map_err(anyhow::Error::from)?
            ],
        )
        .map_err(anyhow::Error::from)?;
    Ok(Json(s))
}
fn ensure_next(next: Option<i64>) -> Result<()> {
    ensure!(
        next.is_some(),
        "Nessuna occorrenza futura nel periodo selezionato"
    );
    Ok(())
}
pub async fn update(
    State(app): State<App>,
    u: User,
    Path(id): Path<String>,
    Json(c): Json<Config>,
) -> ApiResult<Json<Schedule>> {
    let _guard = app.lifecycle.write().await;
    let mut s = get(&app, u.id, &id)?;
    if s.config.workflow_id != c.workflow_id {
        return Err(bad("Workflow non modificabile"));
    }
    if c.active {
        validate_workflow(&app, u.id, &c)?;
    } else {
        c.validate()?;
    }
    let next = c.next_after(now())?;
    if c.active {
        ensure_next(next)?;
    }
    s.config = c;
    s.next = next;
    save(&app, u.id, &s)?;
    Ok(Json(s))
}
pub async fn delete(
    State(app): State<App>,
    u: User,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let _guard = app.lifecycle.write().await;
    get(&app, u.id, &id)?;
    app.store
        .db
        .lock()
        .unwrap()
        .execute(
            "DELETE FROM schedules WHERE user_id=? AND id=?",
            params![u.id, id],
        )
        .map_err(anyhow::Error::from)?;
    Ok(Json(json!({"ok":true})))
}
fn record(
    app: &App,
    uid: i64,
    sid: &str,
    due: i64,
    rid: Option<&str>,
    state: &str,
    message: &str,
) -> Result<bool> {
    Ok(app.store.db.lock().unwrap().execute(
        "INSERT OR IGNORE INTO schedule_occurrences VALUES(?,?,?,?,?,?)",
        params![sid, uid, due, rid, state, message],
    )? == 1)
}
async fn tick(app: &App, recovery: bool) -> Result<()> {
    // Same lifecycle lock as manual creation, schedule edits, account revocation and deletion.
    let _guard = app.lifecycle.write().await;
    let at = now();
    for (uid, mut s) in all(app, None)? {
        let Some(due) = s.next.filter(|t| *t <= at) else {
            continue;
        };
        let skip = if !s.config.active {
            Some("Pianificazione in pausa: scadenze trascorse saltate")
        } else if recovery || at - due > 5 {
            Some("Servizio non disponibile: scadenze trascorse saltate")
        } else if crate::auth::ensure_active(app, uid).is_err() {
            Some("Account non attivo")
        } else if current(app, uid, &s.id)?.is_some() {
            Some("Esecuzione precedente non terminata")
        } else {
            None
        };
        if let Some(message) = skip {
            record(app, uid, &s.id, due, None, "skipped", message)?;
        } else {
            let rid = id();
            if record(app, uid, &s.id, due, Some(&rid), "preparing", "")? {
                let result = crate::api::prepare_run(
                    app,
                    uid,
                    StartRun {
                        workflow_id: s.config.workflow_id.clone(),
                        locale: s.config.locale.clone(),
                        input: s.config.input.clone(),
                        base_run: s.config.base_run.clone(),
                        execution: s.config.execution.clone(),
                        allow_large_source_copy: s.config.allow_large_source_copy,
                    },
                    Some(rid),
                )
                .await;
                let (state, message) = match result {
                    Ok(_) => ("started", String::new()),
                    Err(e) => ("error", e.to_string()),
                };
                app.store.db.lock().unwrap().execute("UPDATE schedule_occurrences SET state=?,message=? WHERE schedule_id=? AND due=?",params![state,message,s.id,due])?;
            }
        }
        s.next = s.config.next_after(at)?;
        save(app, uid, &s)?;
    }
    Ok(())
}
pub fn start_service(app: App) -> Result<()> {
    app.store.db.lock().unwrap().execute("UPDATE schedule_occurrences SET state='error',message='Avvio interrotto dal riavvio del servizio' WHERE state='preparing'",[])?;
    tokio::spawn(async move {
        if let Err(e) = tick(&app, true).await {
            tracing::error!("Ripristino pianificazioni: {e}");
        }
        let mut timer = tokio::time::interval(std::time::Duration::from_secs(2));
        loop {
            timer.tick().await;
            if let Err(e) = tick(&app, false).await {
                tracing::error!("Pianificazioni: {e}");
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(kind: &str, start: &str) -> Config {
        Config {
            workflow_id: "scheduled-test".into(),
            locale: None,
            name: "Daily review".into(),
            kind: kind.into(),
            start: start.into(),
            end: None,
            timezone: "Europe/Rome".into(),
            every: 1,
            weekdays: vec![0, 2, 4],
            input: "scheduled input".into(),
            execution: None,
            base_run: None,
            allow_large_source_copy: false,
            active: true,
        }
    }
    fn utc(s: &str) -> i64 {
        chrono::DateTime::parse_from_rfc3339(s).unwrap().timestamp()
    }
    fn user(id: i64) -> User {
        User {
            id,
            email: format!("user{id}@test.local"),
        }
    }
    fn workflow() -> Workflow {
        Workflow {
            id: "scheduled-test".into(),
            name: "Review".into(),
            description: String::new(),
            entry: "review".into(),
            created: 0,
            execution: ExecutionPolicy::default(),
            nodes: vec![
                Node {
                    id: "source".into(),
                    kind: "Source".into(),
                    source_type: "folder".into(),
                    location: "material".into(),
                    ..Default::default()
                },
                Node {
                    id: "agent".into(),
                    kind: "Agent".into(),
                    ..Default::default()
                },
                Node {
                    id: "task".into(),
                    kind: "Task".into(),
                    prompt: "Test task".into(),
                    agent: "agent".into(),
                    ..Default::default()
                },
                Node {
                    id: "review".into(),
                    kind: "Review".into(),
                    mode: "human".into(),
                    prompt: "Check".into(),
                    ..Default::default()
                },
            ],
            edges: vec![
                Edge {
                    from: "source".into(),
                    to: "agent".into(),
                    kind: "context".into(),
                    outcome: String::new(),
                },
                Edge {
                    from: "agent".into(),
                    to: "task".into(),
                    kind: "context".into(),
                    outcome: String::new(),
                },
                Edge {
                    from: "review".into(),
                    to: "task".into(),
                    kind: "control".into(),
                    outcome: String::new(),
                },
            ],
        }
    }
    #[test]
    fn calendar_skips_missing_hour_and_fires_once_on_repeated_hour() {
        let c = config("daily", "2026-03-28T02:30");
        c.validate().unwrap();
        assert_eq!(
            c.next_after(utc("2026-03-28T01:30:00Z")).unwrap(),
            Some(utc("2026-03-30T00:30:00Z"))
        );
        let mut weekly = config("weekly", "2026-03-22T02:30");
        weekly.weekdays = vec![6];
        assert_eq!(
            weekly.next_after(utc("2026-03-22T01:30:00Z")).unwrap(),
            Some(utc("2026-04-05T00:30:00Z"))
        );
        let c = config("daily", "2026-10-24T02:30");
        let at = c.next_after(utc("2026-10-24T00:30:00Z")).unwrap().unwrap();
        assert_eq!(at, utc("2026-10-25T00:30:00Z"));
        assert_eq!(c.next_after(at).unwrap(), Some(utc("2026-10-26T01:30:00Z")));
    }
    #[test]
    fn interval_weekdays_and_end_are_bounded() {
        let mut c = config("minutes", "2026-11-02T09:00");
        c.every = 15;
        assert_eq!(
            c.next_after(utc("2026-11-02T08:15:00Z")).unwrap(),
            Some(utc("2026-11-02T08:30:00Z"))
        );
        c.end = Some("2026-11-02T09:30".into());
        assert_eq!(c.next_after(utc("2026-11-02T08:30:00Z")).unwrap(), None);
        let c = config("weekly", "2026-11-02T09:00");
        assert_eq!(
            c.next_after(utc("2026-11-02T08:00:00Z")).unwrap(),
            Some(utc("2026-11-04T08:00:00Z"))
        );
        let mut c = config("once", "2026-11-02T09:00");
        assert_eq!(c.next_after(utc("2026-11-02T08:00:00Z")).unwrap(), None);
        c.start = "2026-02-31T09:00".into();
        assert!(c.validate().is_err());
        c.start = "2026-03-29T02:30".into();
        assert!(c.validate().is_err());
    }
    async fn setup() -> (tempfile::TempDir, App, Schedule) {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        std::fs::create_dir_all(app.sources.join("material")).unwrap();
        std::fs::write(app.sources.join("material/input.txt"), "test").unwrap();
        app.store.save_workflow(1, &workflow()).unwrap();
        let mut c = config("minutes", "2026-01-01T09:00");
        c.every = 1;
        let s = create(State(app.clone()), user(1), Json(c))
            .await
            .unwrap()
            .0;
        (dir, app, s)
    }
    async fn due(app: &App, s: &mut Schedule) {
        s.next = Some(now() - 1);
        save(app, 1, s).unwrap();
    }
    #[tokio::test]
    async fn persisted_schedule_creates_no_run_and_is_owner_scoped() {
        let (dir, app, s) = setup().await;
        assert!(app.store.runs(1).unwrap().is_empty());
        assert!(!dir.path().join("users/1/workspaces").exists());
        let reopened = crate::tests::app(dir.path());
        assert_eq!(get(&reopened, 1, &s.id).unwrap().id, s.id);
        assert!(get(&app, 2, &s.id).is_err());
        assert!(
            history(State(app.clone()), user(2), Path(s.id.clone()))
                .await
                .is_err()
        );
        assert!(
            delete(State(app.clone()), user(2), Path(s.id.clone()))
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn dispatch_uses_latest_workflow_reserves_once_and_skips_overlap() {
        let (_dir, app, mut s) = setup().await;
        let mut w = workflow();
        w.name = "Changed before dispatch".into();
        app.store.save_workflow(1, &w).unwrap();
        due(&app, &mut s).await;
        tick(&app, false).await.unwrap();
        let runs = app.store.runs(1).unwrap();
        assert_eq!(runs.len(), 1);
        let r = &runs[0];
        assert_eq!(r.workflow.name, "Changed before dispatch");
        assert_eq!(r.input, "scheduled input");
        assert_eq!(run_schedule(&app, 1, &r.id).unwrap(), Some(s.id.clone()));
        tick(&app, false).await.unwrap();
        assert_eq!(app.store.runs(1).unwrap().len(), 1);
        // A review waiting for human input keeps subsequent occurrences from launching.
        for _ in 0..100 {
            if !app.busy.lock().unwrap().contains(&r.id) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert_eq!(app.store.run(1, &r.id).unwrap().status, "waiting_review");
        s = get(&app, 1, &s.id).unwrap();
        s.next = Some(now());
        save(&app, 1, &s).unwrap();
        tick(&app, false).await.unwrap();
        assert_eq!(app.store.runs(1).unwrap().len(), 1);
        assert!(
            entries(&app, 1, &s.id, 200)
                .unwrap()
                .iter()
                .any(|h| h["state"] == "skipped")
        );
        // Existing claimed occurrence is idempotent even if the next cursor is replayed.
        assert!(
            !record(
                &app,
                1,
                &s.id,
                s.next.unwrap(),
                Some(&id()),
                "preparing",
                ""
            )
            .unwrap()
        );
        s.config.active = false;
        let _ = update(
            State(app.clone()),
            user(1),
            Path(s.id.clone()),
            Json(s.config.clone()),
        )
        .await
        .unwrap();
        assert_eq!(app.store.run(1, &r.id).unwrap().status, "waiting_review");
        let _ = delete(State(app.clone()), user(1), Path(s.id.clone()))
            .await
            .unwrap();
        assert_eq!(app.store.run(1, &r.id).unwrap().status, "waiting_review");
        assert_eq!(run_schedule(&app, 1, &r.id).unwrap(), Some(s.id));
    }
    #[tokio::test]
    async fn pause_resume_delete_and_restart_never_replay_missed_occurrences() {
        let (_dir, app, mut s) = setup().await;
        due(&app, &mut s).await;
        tick(&app, true).await.unwrap();
        assert!(app.store.runs(1).unwrap().is_empty());
        assert_eq!(entries(&app, 1, &s.id, 200).unwrap()[0]["state"], "skipped");
        s = get(&app, 1, &s.id).unwrap();
        s.config.active = false;
        s = update(
            State(app.clone()),
            user(1),
            Path(s.id.clone()),
            Json(s.config.clone()),
        )
        .await
        .unwrap()
        .0;
        due(&app, &mut s).await;
        tick(&app, false).await.unwrap();
        assert!(app.store.runs(1).unwrap().is_empty());
        s.config.active = true;
        s = update(
            State(app.clone()),
            user(1),
            Path(s.id.clone()),
            Json(s.config.clone()),
        )
        .await
        .unwrap()
        .0;
        assert!(s.next.unwrap() > now());
        let _ = delete(State(app.clone()), user(1), Path(s.id.clone()))
            .await
            .unwrap();
        assert!(get(&app, 1, &s.id).is_err());
    }
    #[tokio::test]
    async fn revoked_user_and_invalid_latest_workflow_do_not_launch() {
        let (_dir, app, mut s) = setup().await;
        app.store
            .db
            .lock()
            .unwrap()
            .execute("INSERT INTO user_access VALUES(1,'suspended')", [])
            .unwrap();
        due(&app, &mut s).await;
        tick(&app, false).await.unwrap();
        assert!(app.store.runs(1).unwrap().is_empty());
        app.store
            .db
            .lock()
            .unwrap()
            .execute("UPDATE user_access SET status='active'", [])
            .unwrap();
        let mut w = workflow();
        w.entry = "missing".into();
        app.store.save_workflow(1, &w).unwrap();
        s = get(&app, 1, &s.id).unwrap();
        s.next = Some(now());
        save(&app, 1, &s).unwrap();
        tick(&app, false).await.unwrap();
        assert!(
            entries(&app, 1, &s.id, 200)
                .unwrap()
                .iter()
                .any(|h| h["state"] == "error")
        );
        assert!(app.store.runs(1).unwrap().is_empty());
        // A broken workflow must still allow pausing its schedule.
        s.config.active = false;
        let _ = update(State(app.clone()), user(1), Path(s.id), Json(s.config))
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn deleting_workflow_removes_linked_schedules() {
        let (_dir, app, s) = setup().await;
        let wid = s.config.workflow_id.clone();
        let preview = crate::deletion::preview(State(app.clone()), user(1), Path(wid.clone()))
            .await
            .unwrap()
            .0;
        assert_eq!(preview["schedules"], 1);
        let confirmation =
            serde_json::from_value(json!({"acknowledge":true,"run_ids":[]})).unwrap();
        let _ = crate::deletion::delete(State(app.clone()), user(1), Path(wid), Json(confirmation))
            .await
            .unwrap();
        assert!(get(&app, 1, &s.id).is_err());
        tick(&app, false).await.unwrap();
        assert!(app.store.runs(1).unwrap().is_empty());
    }
}
