use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Read,
    Control,
    Config,
    Admin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthContext {
    pub subject: Option<String>,
    pub permissions: Vec<Permission>,
}

impl AuthContext {
    pub fn allows(&self, required: Permission) -> bool {
        self.permissions.contains(&Permission::Admin) || self.permissions.contains(&required)
    }
}
