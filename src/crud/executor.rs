use super::postgres::PostgresExecutor;
use crate::database::pool::DbPool;

use crate::state::update_query_stats;
use crate::utils::query_timer::query_timer;
use crate::utils::query_type::Query;
use async_trait::async_trait;
use sqlx::{Column, Row, postgres::PgRow};
use std::time::Duration;

#[allow(dead_code)]
pub struct DataMeta {
    pub rows: usize,
    pub message: String,
}

#[allow(dead_code)]
pub enum ExecutionResult {
    Affected { rows: usize, message: String },
    Data { headers: Vec<String>, rows: Vec<sqlx::postgres::PgRow>, meta: DataMeta },
}

#[async_trait]
pub trait DatabaseExecutor: Send + Sync {
    type Row: Row + Send + Sync;

    async fn fetch(&self, query: &str) -> Result<Vec<Self::Row>, sqlx::Error>;
    async fn insert(&self, query: &str) -> Result<u64, sqlx::Error>;
    async fn update(&self, query: &str) -> Result<u64, sqlx::Error>;
    async fn delete(&self, query: &str) -> Result<u64, sqlx::Error>;
    #[allow(dead_code)]
    fn get_value_as_string(&self, row: &Self::Row, index: usize) -> String;
}

pub fn create_executor(pool: &DbPool) -> impl DatabaseExecutor<Row = PgRow> {
    match pool {
        DbPool::Postgres(pg_pool) => PostgresExecutor::new(pg_pool.clone()),
        DbPool::MySQL(_) => todo!(),
        DbPool::SQLite(_) => todo!(),
    }
}

fn format_affected_result(query_type: &str, rows: usize, elapsed: Duration) -> ExecutionResult {
    let message = format!(
        "{} {} rows affected.\nQuery completed in {} msec.",
        query_type,
        rows,
        elapsed.as_millis()
    );
    ExecutionResult::Affected { rows, message }
}

async fn run_affected_query<Fut>(
    fut: Fut,
    query_type: &'static str,
) -> Result<ExecutionResult, sqlx::Error>
where
    Fut: std::future::Future<Output = Result<u64, sqlx::Error>>,
{
    let (result, elapsed) = query_timer(fut).await;
    let rows = result? as usize;
    update_query_stats(rows, elapsed).await;
    Ok(format_affected_result(query_type, rows, elapsed))
}

pub async fn execute_query(pool: &DbPool, sql: &str) -> Result<ExecutionResult, sqlx::Error> {
    let executor = create_executor(pool);

    match Query::from_sql(sql) {
        Query::SELECT => {
            let (rows_result, elapsed) = query_timer(executor.fetch(sql)).await;
            let rows = rows_result?;
            let row_count = rows.len();

            update_query_stats(row_count, elapsed).await;

            let message = format!(
                "Successfully run. Total query runtime: {} ms.\n{} rows fetched.",
                elapsed.as_millis(),
                row_count,
            );

            let headers = if let Some(first_row) = rows.first() {
                first_row.columns().iter().map(|c| c.name().to_string()).collect()
            } else {
                Vec::new()
            };

            Ok(ExecutionResult::Data {
                headers,
                rows,
                meta: DataMeta {
                    rows: row_count,
                    message,
                },
            })
        }

        Query::INSERT => run_affected_query(executor.insert(sql), "INSERT").await,
        Query::UPDATE => run_affected_query(executor.update(sql), "UPDATE").await,
        Query::DELETE => run_affected_query(executor.delete(sql), "DELETE").await,

        Query::UNKNOWN => Err(sqlx::Error::Protocol("Unsupported query".into())),
    }
}
