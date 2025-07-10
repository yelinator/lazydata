use super::executor::DatabaseExecutor;
use async_trait::async_trait;
use hex;
use serde_json::Value;
use sqlx::{
    PgPool, Row,
    postgres::PgRow,
    types::{Json, Uuid, chrono},
};

pub struct PostgresExecutor {
    pool: PgPool,
}

impl PostgresExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn execute_query(&self, query: &str) -> Result<u64, sqlx::Error> {
        Ok(sqlx::query(query)
            .execute(&self.pool)
            .await?
            .rows_affected())
    }
}

#[async_trait]
impl DatabaseExecutor for PostgresExecutor {
    type Row = PgRow;

    async fn fetch(&self, query: &str) -> Result<Vec<PgRow>, sqlx::Error> {
        let rows = sqlx::query(query).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    async fn insert(&self, query: &str) -> Result<u64, sqlx::Error> {
        self.execute_query(query).await
    }

    async fn update(&self, query: &str) -> Result<u64, sqlx::Error> {
        self.execute_query(query).await
    }

    async fn delete(&self, query: &str) -> Result<u64, sqlx::Error> {
        self.execute_query(query).await
    }

    fn get_value_as_string(&self, row: &PgRow, index: usize) -> String {
        macro_rules! try_get_string {
            ($($type:ty),*) => {
                $(
                    if let Ok(val) = row.try_get::<$type, _>(index) {
                        return val.to_string();
                    }
                )*
            };
        }

        try_get_string!(
            String,
            &str,
            i16,
            i32,
            i64,
            f32,
            f64,
            bool,
            Uuid,
            chrono::NaiveDate,
            chrono::NaiveDateTime,
            chrono::NaiveTime,
            chrono::DateTime<chrono::Utc>
        );

        if let Ok(val) = row.try_get::<Value, _>(index) {
            return match serde_json::to_string(&val) {
                Ok(s) => s,
                Err(e) => format!("[json-error: {}]", e),
            };
        }

        if let Ok(Json(val)) = row.try_get::<Json<Value>, _>(index) {
            return match serde_json::to_string(&val) {
                Ok(s) => s,
                Err(e) => format!("[json-error: {}]", e),
            };
        }

        if let Ok(val) = row.try_get::<Vec<u8>, _>(index) {
            return hex::encode(val);
        }

        "".to_string()
    }
}
