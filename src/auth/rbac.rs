//! Role enum with capability helpers.
//! Roles: admin | super_user | user.

use std::{fmt, str::FromStr};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    SuperUser,
    User,
}

impl Role {
    pub fn can_upload(self) -> bool {
        matches!(self, Role::Admin | Role::SuperUser)
    }

    pub fn can_delete(self) -> bool {
        matches!(self, Role::Admin | Role::SuperUser)
    }

    pub fn can_manage_users(self) -> bool {
        matches!(self, Role::Admin)
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::Admin => write!(f, "admin"),
            Role::SuperUser => write!(f, "super_user"),
            Role::User => write!(f, "user"),
        }
    }
}

impl FromStr for Role {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Role::Admin),
            "super_user" => Ok(Role::SuperUser),
            "user" => Ok(Role::User),
            other => Err(anyhow::anyhow!("unknown role: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_admin_manages_users() {
        assert!(Role::Admin.can_manage_users());
        assert!(!Role::SuperUser.can_manage_users());
        assert!(!Role::User.can_manage_users());
    }

    #[test]
    fn upload_and_delete_need_elevation() {
        assert!(Role::Admin.can_upload() && Role::Admin.can_delete());
        assert!(Role::SuperUser.can_upload() && Role::SuperUser.can_delete());
        assert!(!Role::User.can_upload() && !Role::User.can_delete());
    }

    #[test]
    fn display_fromstr_roundtrip() {
        for role in [Role::Admin, Role::SuperUser, Role::User] {
            assert_eq!(role.to_string().parse::<Role>().unwrap(), role);
        }
        assert!("owner".parse::<Role>().is_err());
    }
}
