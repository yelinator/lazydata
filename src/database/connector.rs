use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseType {
    PostgreSQL,
    MySQL,
    SQLite,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ConnectionDetails {
    pub host: Option<String>,
    pub user: Option<String>,
    pub password: Option<String>,
    pub database: Option<String>,
}

impl std::fmt::Display for DatabaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            DatabaseType::PostgreSQL => "PostgreSQL",
            DatabaseType::MySQL => "MySQL",
            DatabaseType::SQLite => "SQLite",
        };
        write!(f, "{s}")
    }
}
