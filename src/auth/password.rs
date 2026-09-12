//! Password hashing with Argon2id.
//! The Community starts fresh → no legacy bcrypt hashes to import.
//! If bcrypt import is ever needed, add the `bcrypt` crate (see STACK).

use anyhow::Result;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

/// Floor the frontend already advertises (`App.jsx` refuses fewer than 6
/// characters client-side): the API must enforce at least the same, or the
/// check is bypassable with a direct HTTP call.
pub const MIN_PASSWORD_LEN: usize = 6;
/// Ceiling the UI never had: Argon2's cost scales with input size, so an
/// unbounded password is CPU-DoS per request. 128 characters is far above
/// any memorable password.
pub const MAX_PASSWORD_LEN: usize = 128;

/// Policy for a newly chosen password. Length only — checked in bytes, so a
/// password with at least MIN_PASSWORD_LEN characters always passes
/// regardless of script. Called before hashing; see
/// `api::auth::change_password`.
pub fn validate_new_password(password: &str) -> Result<()> {
    let len = password.len();
    if len < MIN_PASSWORD_LEN {
        anyhow::bail!("new password must be at least {MIN_PASSWORD_LEN} characters");
    }
    if len > MAX_PASSWORD_LEN {
        anyhow::bail!("new password must be at most {MAX_PASSWORD_LEN} characters");
    }
    Ok(())
}

pub fn hash(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("hash error: {e}"))?
        .to_string();
    Ok(hash)
}

pub fn verify(password: &str, hash: &str) -> Result<bool> {
    let parsed = PasswordHash::new(hash).map_err(|e| anyhow::anyhow!("parse hash: {e}"))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_short_passwords_fail() {
        for password in ["", "x", "12345"] {
            assert!(
                validate_new_password(password).is_err(),
                "password={password:?}"
            );
        }
    }

    #[test]
    fn boundary_lengths_pass() {
        assert!(validate_new_password("123456").is_ok());
        assert!(validate_new_password(&"x".repeat(MAX_PASSWORD_LEN)).is_ok());
    }

    #[test]
    fn overlong_password_fails() {
        assert!(validate_new_password(&"x".repeat(MAX_PASSWORD_LEN + 1)).is_err());
    }

    #[test]
    fn hash_and_verify_roundtrip() {
        let hash = hash("correct-horse-1").unwrap();
        assert!(verify("correct-horse-1", &hash).unwrap());
        assert!(!verify("wrong-horse-2", &hash).unwrap());
    }
}
