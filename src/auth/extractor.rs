//! Axum extractor: reads `Authorization: Bearer <token>`, decodes the JWT,
//! then re-checks it against the database before handing `Claims` to a
//! handler. Handlers that declare `claims: Claims` get a 401 automatically
//! when the token is absent, invalid, expired, or no longer backed by an
//! active account.
//!
//! # Why the database is consulted on every request
//!
//! A JWT is a snapshot of who someone was when it was issued, and this one
//! lives for eight hours by default. Trusting it as-is meant that
//! deactivating a user, demoting an administrator, or changing a password
//! after it leaked had NO effect until the token expired on its own: the
//! claims kept asserting the old identity and the old role, and every
//! handler believed them. There is no revocation list to consult instead —
//! logging out only clears the browser's localStorage.
//!
//! So the role is now read from the row, not from the token, and the lookup
//! (`find_by_id`) already filters `is_active = 1`, which makes deactivation
//! take effect on the next request rather than at the next expiry.
//!
//! The cost is one SQLite lookup on a primary key per authenticated request
//! — microseconds against a local file, and paid once per request rather
//! than per handler. What it does NOT yet cover is a password change: the
//! tokens issued before it stay valid until they expire. Closing that needs
//! a token version column on `users`, compared against a claim; this is the
//! cheap 90% of it.

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    Json,
};
use serde_json::json;

use crate::auth::jwt::{Claims, decode_token};
use crate::db::users;
use crate::state::AppState;

#[async_trait]
impl FromRequestParts<AppState> for Claims {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let auth = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let token = auth.strip_prefix("Bearer ").ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "missing or non-Bearer Authorization header"})),
            )
        })?;

        let mut claims = decode_token(token, &state.settings.auth.jwt_secret).map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "invalid or expired token"})),
            )
        })?;

        // The token said who they were; the row says who they are. See the
        // module documentation above for why this lookup is worth its cost.
        match users::find_by_id(&state.db, claims.user_id).await {
            Ok(Some(user)) => {
                // The role comes from the database, never from the token: a
                // demotion has to take effect now, not in eight hours.
                claims.role = user.role();
                Ok(claims)
            }
            // Deleted, or deactivated: find_by_id already filters is_active.
            Ok(None) => Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "account no longer active"})),
            )),
            Err(e) => {
                // Our failure, not theirs — and it must not read as a
                // credentials problem, or a database outage would look like
                // every user being logged out.
                tracing::error!(error = %e, "auth: user lookup failed");
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal server error"})),
                ))
            }
        }
    }
}
