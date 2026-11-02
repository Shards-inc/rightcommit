use crate::{App, model::*, store::hash};
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use rand::RngCore;
use rusqlite::{OptionalExtension, params};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

#[derive(Debug)]
pub struct ApiError(pub StatusCode, pub String);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, axum::Json(serde_json::json!({"error":self.1}))).into_response()
    }
}
impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self(StatusCode::BAD_REQUEST, e.to_string())
    }
}
pub type ApiResult<T> = Result<T, ApiError>;
pub fn bad(s: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, s.into())
}
impl FromRequestParts<App> for User {
    type Rejection = ApiError;
    async fn from_request_parts(p: &mut Parts, app: &App) -> ApiResult<Self> {
        let token = p
            .headers
            .get("cookie")
            .and_then(|v| v.to_str().ok())
            .and_then(|c| {
                c.split(';')
                    .find_map(|p| p.trim().strip_prefix("nodo_session="))
            })
            .ok_or_else(|| ApiError(StatusCode::UNAUTHORIZED, "Accedi per continuare".into()))?;
        if token.len() != 64 {
            return Err(ApiError(
                StatusCode::UNAUTHORIZED,
                "Sessione non valida".into(),
            ));
        }
        let user=app.store.db.lock().unwrap().query_row("SELECT u.id,u.email FROM users u JOIN tokens t ON t.user_id=u.id LEFT JOIN user_access a ON a.user_id=u.id WHERE t.hash=? AND t.expires>? AND COALESCE(a.status,'active')='active'",params![hash(token),now()],|r|Ok(User{id:r.get(0)?,email:r.get(1)?})).optional().map_err(anyhow::Error::from)?;
        user.ok_or_else(|| ApiError(StatusCode::UNAUTHORIZED, "Sessione scaduta".into()))
    }
}
pub fn ensure_active(app: &App, uid: i64) -> anyhow::Result<()> {
    let status: Option<String> = app.store.db.lock().unwrap().query_row(
        "SELECT status FROM user_access WHERE user_id=?", [uid], |r| r.get(0),
    ).optional()?;
    anyhow::ensure!(status.as_deref().is_none_or(|s| s == "active"), "Account non attivo");
    Ok(())
}
static ATTEMPTS: LazyLock<Mutex<HashMap<String, (i64, u32)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
pub async fn password_hash(password: String) -> anyhow::Result<String> {
    anyhow::ensure!(
        (12..=256).contains(&password.len()),
        "La password deve contenere tra 12 e 256 caratteri"
    );
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .map(|h| h.to_string())
            .map_err(|_| anyhow::anyhow!("Hash password fallito"))
    })
    .await?
}
pub async fn create_user(app: &App, email: String, password: String) -> ApiResult<User> {
    let email = email.trim().to_lowercase();
    if email.len() > 200 || !email.contains('@') {
        return Err(bad("Email non valida"));
    }
    let ph = password_hash(password).await?;
    let uid = {
        let db = app.store.db.lock().unwrap();
        db.execute(
            "INSERT INTO users(email,password) VALUES(?,?)",
            params![email, ph],
        )
        .map_err(|_| bad("Account non creabile: email già presente"))?;
        db.last_insert_rowid()
    };
    crate::agents::user_home(app, uid)?;
    for w in crate::presets::all() {
        app.store.save_workflow(uid, &w)?;
    }
    Ok(User { id: uid, email })
}
pub async fn login(app: &App, email: String, password: String) -> ApiResult<(User, String)> {
    let email = email.trim().to_lowercase();
    if email.len() > 200 || password.len() > 256 {
        return Err(bad("Credenziali non valide"));
    }
    {
        let mut a = ATTEMPTS.lock().unwrap();
        a.retain(|_, (t, _)| now() - *t < 120);
        let v = a.entry(email.clone()).or_insert((now(), 0));
        if v.1 >= 8 {
            return Err(ApiError(
                StatusCode::TOO_MANY_REQUESTS,
                "Troppi tentativi. Riprova tra due minuti".into(),
            ));
        }
        v.1 += 1;
    }
    let row: Option<(i64, String)> = app
        .store
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT u.id,u.password FROM users u LEFT JOIN user_access a ON a.user_id=u.id WHERE u.email=? AND COALESCE(a.status,'active')='active'",
            [&email],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(anyhow::Error::from)?;
    let Some((uid, ph)) = row else {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "Credenziali non valide".into(),
        ));
    };
    let valid = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&ph).is_ok_and(|p| {
            Argon2::default()
                .verify_password(password.as_bytes(), &p)
                .is_ok()
        })
    })
    .await
    .map_err(anyhow::Error::from)?;
    if !valid {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "Credenziali non valide".into(),
        ));
    }
    ATTEMPTS.lock().unwrap().remove(&email);
    let mut b = [0; 32];
    rand::thread_rng().fill_bytes(&mut b);
    let token = hex::encode(b);
    let inserted = app.store.db.lock().unwrap().execute(
        "INSERT INTO tokens(hash,user_id,expires) SELECT ?,?,? WHERE EXISTS(SELECT 1 FROM users WHERE id=?) AND NOT EXISTS(SELECT 1 FROM user_access WHERE user_id=? AND status!='active')",
        params![hash(&token), uid, now() + 7 * 86400, uid, uid],
    ).map_err(anyhow::Error::from)?;
    if inserted != 1 { return Err(ApiError(StatusCode::UNAUTHORIZED, "Account non attivo".into())); }
    Ok((User { id: uid, email }, token))
}
pub fn cookie(app: &App, token: &str) -> String {
    format!(
        "nodo_session={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=604800{}",
        if app.secure_cookie { "; Secure" } else { "" }
    )
}

pub async fn change_password(app: &App, u: &User, current: String, new: String) -> ApiResult<()> {
    if current.len() > 256 {
        return Err(bad("Password non valida"));
    }
    let ph: String = app
        .store
        .db
        .lock()
        .unwrap()
        .query_row("SELECT password FROM users WHERE id=?", [u.id], |r| {
            r.get(0)
        })
        .map_err(anyhow::Error::from)?;
    let valid = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&ph).is_ok_and(|p| {
            Argon2::default()
                .verify_password(current.as_bytes(), &p)
                .is_ok()
        })
    })
    .await
    .map_err(anyhow::Error::from)?;
    if !valid {
        return Err(bad("Password attuale non valida"));
    }
    let ph = password_hash(new).await?;
    let mut db = app.store.db.lock().unwrap();
    let tx = db.transaction().map_err(anyhow::Error::from)?;
    tx.execute("UPDATE users SET password=? WHERE id=?", params![ph, u.id])
        .map_err(anyhow::Error::from)?;
    tx.execute("DELETE FROM tokens WHERE user_id=?", [u.id])
        .map_err(anyhow::Error::from)?;
    tx.commit().map_err(anyhow::Error::from)?;
    Ok(())
}
