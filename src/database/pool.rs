use sqlx::{mysql::MySqlPool, postgres::PgPool, sqlite::SqlitePool};

use super::connector::{ConnectionDetails, DatabaseType};

#[derive(Debug, Clone)]
pub enum DbPool {
    Postgres(PgPool),
    MySQL(MySqlPool),
    SQLite(SqlitePool),
}

impl DbPool {
    pub fn get_type(&self) -> DatabaseType {
        match self {
            DbPool::Postgres(_) => DatabaseType::PostgreSQL,
            DbPool::MySQL(_) => DatabaseType::MySQL,
            DbPool::SQLite(_) => DatabaseType::SQLite,
        }
    }
}

pub async fn pool(
    db_type: DatabaseType,
    details: &ConnectionDetails,
) -> Result<DbPool, sqlx::Error> {
    let conn_str = &details.connection_string();

    let pool = match db_type {
        DatabaseType::PostgreSQL => {
            let pool = PgPool::connect(conn_str).await?;
            DbPool::Postgres(pool)
        }
        DatabaseType::MySQL => {
            let pool = MySqlPool::connect(conn_str).await?;
            DbPool::MySQL(pool)
        }
        DatabaseType::SQLite => {
            let pool = SqlitePool::connect(conn_str).await?;
            DbPool::SQLite(pool)
        }
    };

    Ok(pool)
}
