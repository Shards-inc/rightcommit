use crate::{
    App,
    auth::{ApiError, ApiResult, bad},
    model::User,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

static ACCESS_CHANGES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn require_admin(user: &User) -> ApiResult<()> {
    if user.id != 1 {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Operazione riservata all’amministratore".into(),
        ));
    }
    Ok(())
}
pub async fn list(State(app): State<App>, user: User) -> ApiResult<Json<Value>> {
    require_admin(&user)?;
    let db = app.store.db.lock().unwrap();
    let mut statement = db.prepare("SELECT u.id,u.email,COALESCE(a.status,'active') FROM users u LEFT JOIN user_access a ON a.user_id=u.id WHERE COALESCE(a.status,'active')!='removed' ORDER BY u.id").map_err(anyhow::Error::from)?;
    let users = statement.query_map([], |r| Ok(json!({"id":r.get::<_,i64>(0)?,"email":r.get::<_,String>(1)?,"status":r.get::<_,String>(2)?,"admin":r.get::<_,i64>(0)?==1}))).map_err(anyhow::Error::from)?.collect::<Result<Vec<_>,_>>().map_err(anyhow::Error::from)?;
    Ok(Json(json!({"users":users})))
}
#[derive(Deserialize)]
pub struct Change {
    #[serde(default)]
    acknowledge: bool,
    #[serde(default)]
    status: String,
    expected_status: String,
}
async fn change(
    app: App,
    actor: User,
    uid: i64,
    request: Change,
    status: &str,
) -> ApiResult<Json<Value>> {
    require_admin(&actor)?;
    // Finish revoking runtime access before a concurrent reactivation can succeed.
    let _access_change = ACCESS_CHANGES.lock().await;
    if uid == actor.id || uid == 1 {
        return Err(bad(
            "Non puoi sospendere o rimuovere l’amministratore corrente",
        ));
    }
    if !request.acknowledge {
        return Err(bad("Conferma l’operazione sull’utente"));
    }
    if !["active", "suspended", "removed"].contains(&status) {
        return Err(bad("Stato utente non valido"));
    }
    {
        let mut db = app.store.db.lock().unwrap();
        let tx = db.transaction().map_err(anyhow::Error::from)?;
        let current: Option<String> = tx.query_row("SELECT COALESCE(a.status,'active') FROM users u LEFT JOIN user_access a ON a.user_id=u.id WHERE u.id=?", [uid], |r| r.get(0)).optional().map_err(anyhow::Error::from)?;
        let current = current
            .filter(|s| s != "removed")
            .ok_or_else(|| bad("Utente non trovato"))?;
        if current != request.expected_status {
            return Err(bad(
                "Lo stato dell’utente è cambiato. Aggiorna l’elenco e riprova.",
            ));
        }
        if current == status {
            return Err(bad("Lo stato dell’utente è già aggiornato"));
        }
        tx.execute("INSERT INTO user_access(user_id,status) VALUES(?,?) ON CONFLICT(user_id) DO UPDATE SET status=excluded.status", params![uid,status]).map_err(anyhow::Error::from)?;
        // Reactivation requires a fresh login; revoked sessions never become valid again.
        tx.execute("DELETE FROM tokens WHERE user_id=?", [uid])
            .map_err(anyhow::Error::from)?;
        tx.commit().map_err(anyhow::Error::from)?;
    }
    if status != "active" {
        app.mcp
            .logins
            .lock()
            .unwrap()
            .retain(|_, login| login.uid != uid);
        app.mcp.clear_user(uid).await;
        let ids: Vec<String> = {
            let db = app.store.db.lock().unwrap();
            let mut query = db
                .prepare("SELECT id FROM runs WHERE user_id=?")
                .map_err(anyhow::Error::from)?;
            query
                .query_map([uid], |r| r.get(0))
                .map_err(anyhow::Error::from)?
                .collect::<Result<_, _>>()
                .map_err(anyhow::Error::from)?
        };
        let controls: Vec<_> = {
            let workers = app.workers.lock().unwrap();
            ids.iter()
                .filter_map(|id| workers.get(id).cloned())
                .collect()
        };
        for control in &controls {
            let _ = control.stop.send(true);
        }
        for mut control in controls {
            while !*control.done.borrow_and_update() {
                if control.done.changed().await.is_err() {
                    break;
                }
            }
        }
        // Cancelling the worker stops its CLI; also persist a resumable interrupted state.
        for id in &ids {
            let mut run = app.store.run(uid, id)?;
            if ["starting", "running", "waiting_external", "waiting_child"]
                .contains(&run.status.as_str())
            {
                run.status = "interrupted".into();
                run.pause_requested = false;
                run.error = Some("Esecuzione interrotta: accesso account revocato".into());
                for state in run.states.values_mut() {
                    if state == "running" {
                        *state = "interrupted".into();
                    }
                }
                crate::engine::save(&app, uid, &mut run)?;
            }
        }
        // Wake existing streams so they observe session revocation immediately.
        for id in ids {
            crate::agents::emit(
                &app,
                uid,
                &id,
                "access_revoked",
                "",
                "Accesso account revocato",
            );
        }
    }
    Ok(Json(json!({"ok":true,"status":status})))
}
pub async fn set_status(
    State(app): State<App>,
    user: User,
    Path(uid): Path<i64>,
    Json(request): Json<Change>,
) -> ApiResult<Json<Value>> {
    if !["active", "suspended"].contains(&request.status.as_str()) {
        return Err(bad("Stato utente non valido"));
    }
    let status = request.status.clone();
    change(app, user, uid, request, &status).await
}
pub async fn remove(
    State(app): State<App>,
    user: User,
    Path(uid): Path<i64>,
    Json(request): Json<Change>,
) -> ApiResult<Json<Value>> {
    // Account removal is a durable tombstone: keep ownership and stored work intact.
    change(app, user, uid, request, "removed").await
}
