//! User persistence (rag_users.db → `users` table).
//!
//! Schema: id, username, email, password_hash, role, created_at,
//! last_login, is_active

use anyhow::Result;
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::Rng;
use sqlx::SqlitePool;

use crate::auth::rbac::Role;

#[derive(Debug, sqlx::FromRow)]
pub struct UserRow {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: String,
    pub last_login: Option<String>,
    #[allow(dead_code)]
    pub is_active: i64,
}

impl UserRow {
    pub fn role(&self) -> Role {
        self.role.parse().unwrap_or(Role::User)
    }
}

pub async fn find_by_username(pool: &SqlitePool, username: &str) -> Result<Option<UserRow>> {
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, email, password_hash, role, created_at, last_login, is_active
         FROM users WHERE username = ? AND is_active = 1"
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn create(
    pool: &SqlitePool,
    username: &str,
    email: &str,
    password_hash: &str,
    role: Role,
) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    let role_str = role.to_string();
    let id = sqlx::query(
        "INSERT INTO users (username, email, password_hash, role, created_at, is_active)
         VALUES (?, ?, ?, ?, ?, 1)"
    )
    .bind(username)
    .bind(email)
    .bind(password_hash)
    .bind(role_str)
    .bind(now)
    .execute(pool)
    .await?
    .last_insert_rowid();
    Ok(id)
}

pub async fn find_by_id(pool: &SqlitePool, user_id: i64) -> Result<Option<UserRow>> {
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, email, password_hash, role, created_at, last_login, is_active
         FROM users WHERE id = ? AND is_active = 1"
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Crea o aggiorna l'admin di default.
///
/// Behaviour:
/// - The admin does not exist yet → created, with `AUTH__ADMIN_DEFAULT_PASSWORD`
///   if that is set and passes the password policy, otherwise with a random
///   password printed once to the log.
/// - The admin already exists → `AUTH__ADMIN_DEFAULT_PASSWORD` is IGNORED, and
///   a warning says so. It used to overwrite the stored password on every
///   single start, which meant an installation carrying that variable could
///   never really change its admin password: the value in `.env` won at the
///   next restart, silently, even after the admin had set a new one from the
///   UI. And because the only check was "not empty", `=x` was a one-character
///   administrator, reinstated at every boot.
/// - `AUTH__ADMIN_RESET_PASSWORD` overwrites it deliberately, exists for the
///   one real need the old behaviour served — being locked out — and says
///   loudly in the log that it did so. Unset it after use, or the next restart
///   resets the password again.
pub async fn seed_admin(
    pool: &SqlitePool,
    configured_password: Option<&str>,
    reset_password: Option<&str>,
) -> Result<()> {
    use crate::auth::password;

    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE username = ?")
        .bind("admin")
        .fetch_one(pool)
        .await? > 0;

    // Deliberate reset. The only path that may overwrite an existing password.
    if let Some(p) = reset_password.filter(|p| !p.is_empty()) {
        password::validate_new_password(p)
            .map_err(|e| anyhow::anyhow!("AUTH__ADMIN_RESET_PASSWORD: {e}"))?;
        let hash = password::hash(p)?;
        if exists {
            sqlx::query("UPDATE users SET password_hash = ? WHERE username = ?")
                .bind(&hash)
                .bind("admin")
                .execute(pool)
                .await?;
            tracing::warn!(
                "admin password RESET from AUTH__ADMIN_RESET_PASSWORD — unset that \
                 variable, or the next restart will reset it again"
            );
        } else {
            create(pool, "admin", "admin@rag-engine.local", &hash, Role::Admin).await?;
            tracing::info!("admin created with AUTH__ADMIN_RESET_PASSWORD");
        }
        return Ok(());
    }

    if exists {
        if configured_password.is_some_and(|p| !p.is_empty()) {
            tracing::warn!(
                "AUTH__ADMIN_DEFAULT_PASSWORD is set but the admin account already \
                 exists, so it was ignored — it seeds a new installation, it does not \
                 change an existing password. Use the UI, or AUTH__ADMIN_RESET_PASSWORD \
                 if you are locked out."
            );
        }
        return Ok(());
    }

    // Seeding a fresh installation.
    if let Some(p) = configured_password.filter(|p| !p.is_empty()) {
        password::validate_new_password(p)
            .map_err(|e| anyhow::anyhow!("AUTH__ADMIN_DEFAULT_PASSWORD: {e}"))?;
        let hash = password::hash(p)?;
        create(pool, "admin", "admin@rag-engine.local", &hash, Role::Admin).await?;
        tracing::info!("admin created with AUTH__ADMIN_DEFAULT_PASSWORD");
        return Ok(());
    }

    // Nothing configured → generate one and print it once.
    let generated: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(22)
        .map(char::from)
        .collect();
    tracing::warn!("========================================");
    tracing::warn!("ACCOUNT ADMIN CREATO CON PASSWORD CASUALE");
    tracing::warn!("  Username: admin");
    tracing::warn!("  Password: {generated}");
    tracing::warn!("SAVE THIS PASSWORD — it will not be shown again!");
    tracing::warn!("Per impostarne una fissa alla PRIMA installazione: AUTH__ADMIN_DEFAULT_PASSWORD=...");
    tracing::warn!("========================================");
    let hash = password::hash(&generated)?;
    create(pool, "admin", "admin@rag-engine.local", &hash, Role::Admin).await?;
    Ok(())
}

pub async fn update_password(pool: &SqlitePool, user_id: i64, new_hash: &str) -> Result<()> {
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(new_hash)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn touch_last_login(pool: &SqlitePool, user_id: i64) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE users SET last_login = ? WHERE id = ?")
        .bind(now)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
