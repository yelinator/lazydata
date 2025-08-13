use super::pool::DbPool;
use color_eyre::eyre::Result;
use ratatui::text::Text;
use sqlx::{MySqlPool, PgPool, Row, SqlitePool};
use tui_tree_widget::TreeItem;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Database {
    pub name: String,
    pub tables: Vec<Table>,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub metadata: Option<TableMetadata>,
}

#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub data_type: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TableMetadata {
    pub name: String,
    pub columns: Vec<Column>,
    pub constraints: Vec<String>,
    pub indexes: Vec<String>,
    pub rls_policies: Vec<String>,
    pub rules: Vec<String>,
    pub triggers: Vec<String>,
    pub row_count: i64,
    pub estimated_size: String,
    pub table_type: String,
}

pub trait Displayable {
    fn to_string(&self) -> String;
    fn name(&self) -> String;
}

impl Displayable for Column {
    fn to_string(&self) -> String {
        format!("{} ({})", self.name, self.data_type)
    }
    fn name(&self) -> String {
        self.name.clone()
    }
}

impl Displayable for String {
    fn to_string(&self) -> String {
        self.clone()
    }
    fn name(&self) -> String {
        self.clone()
    }
}

#[async_trait::async_trait]
pub trait MetadataFetcher: Send + Sync {
    async fn fetch_tables(&self) -> Result<Vec<Table>>;
    async fn fetch_table_metadata(&self, table_name: &str) -> Result<TableMetadata>;
    async fn fetch_databases(&self) -> Result<Vec<String>>;
}

#[async_trait::async_trait]
impl MetadataFetcher for PgPool {
    async fn fetch_tables(&self) -> Result<Vec<Table>> {
        let rows = sqlx::query(
            "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' ORDER BY table_name ASC",
        )
        .fetch_all(self)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| Table {
                name: row.get("table_name"),
                metadata: None,
            })
            .collect())
    }

    async fn fetch_table_metadata(&self, table_name: &str) -> Result<TableMetadata> {
        let row = sqlx::query(
            r#"
                SELECT
                    c.relname AS table_name,
                    CASE
                        WHEN c.reltuples < 0 THEN 0
                        ELSE c.reltuples::BIGINT
                    END AS row_estimate,
                    pg_size_pretty(pg_total_relation_size(c.oid)) AS total_size,
                    CASE c.relkind
                        WHEN 'r' THEN 'table'
                        WHEN 'v' THEN 'view'
                        WHEN 'm' THEN 'materialized view'
                        WHEN 'f' THEN 'foreign table'
                        ELSE 'other'
                    END AS table_type
                FROM pg_class c
                JOIN pg_namespace n ON n.oid = c.relnamespace
                WHERE n.nspname = 'public' AND c.relkind IN ('r', 'v', 'm', 'f') AND c.relname = $1
            "#,
        )
        .bind(table_name)
        .fetch_one(self)
        .await?;

        let table_name: String = row.get("table_name");
        let row_count: i64 = row.get("row_estimate");
        let estimated_size: String = row.get("total_size");
        let table_type: String = row.get("table_type");

        let columns = get_pg_columns(self, &table_name).await?;
        let constraints = get_pg_constraints(self, &table_name).await?;
        let indexes = get_pg_indexes(self, &table_name).await?;
        let rls_policies = get_pg_rls_policies(self, &table_name).await?;
        let rules = get_pg_rules(self, &table_name).await?;
        let triggers = get_pg_triggers(self, &table_name).await?;

        Ok(TableMetadata {
            name: table_name,
            columns,
            constraints,
            indexes,
            rls_policies,
            rules,
            triggers,
            row_count,
            estimated_size,
            table_type,
        })
    }

    async fn fetch_databases(&self) -> Result<Vec<String>> {
        let rows = sqlx::query("SELECT datname FROM pg_database WHERE datistemplate = false;")
            .fetch_all(self)
            .await?;
        Ok(rows.into_iter().map(|r| r.get("datname")).collect())
    }
}

#[async_trait::async_trait]
impl MetadataFetcher for MySqlPool {
    async fn fetch_tables(&self) -> Result<Vec<Table>> {
        let rows = sqlx::query("SHOW TABLES ORDER BY table_name ASC")
            .fetch_all(self)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| Table {
                name: row.get(0),
                metadata: None,
            })
            .collect())
    }

    async fn fetch_table_metadata(&self, table_name: &str) -> Result<TableMetadata> {
        let row = sqlx::query("SHOW TABLE STATUS WHERE Name = ?")
            .bind(table_name)
            .fetch_one(self)
            .await?;

        let table_name: String = row.get("Name");
        let row_count: i64 = row.try_get("Rows").unwrap_or(0);
        let estimated_size: String = {
            let data_length: i64 = row.try_get("Data_length").unwrap_or(0);
            let index_length: i64 = row.try_get("Index_length").unwrap_or(0);
            format!("{} bytes", data_length + index_length)
        };
        let table_type: String = row.try_get("Comment").unwrap_or("".to_string());

        let columns = sqlx::query(&format!("SHOW COLUMNS FROM `{}`", table_name))
            .fetch_all(self)
            .await?
            .into_iter()
            .map(|r| Column {
                name: r.get("Field"),
                data_type: r.get("Type"),
            })
            .collect();

        let triggers = sqlx::query("SHOW TRIGGERS WHERE `Table` = ?")
            .bind(&table_name)
            .fetch_all(self)
            .await?
            .into_iter()
            .map(|r| r.get("Trigger"))
            .collect();

        Ok(TableMetadata {
            name: table_name,
            columns,
            constraints: vec![],
            indexes: vec![],
            rls_policies: vec![],
            rules: vec![],
            triggers,
            row_count,
            estimated_size,
            table_type,
        })
    }

    async fn fetch_databases(&self) -> Result<Vec<String>> {
        let rows = sqlx::query("SHOW DATABASES;").fetch_all(self).await?;
        Ok(rows
            .into_iter()
            .map(|r| r.get(0))
            .filter(|name: &String| {
                !["information_schema", "mysql", "performance_schema", "sys"]
                    .contains(&name.as_str())
            })
            .collect())
    }
}

#[async_trait::async_trait]
impl MetadataFetcher for SqlitePool {
    async fn fetch_tables(&self) -> Result<Vec<Table>> {
        let rows =
            sqlx::query("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name ASC")
                .fetch_all(self)
                .await?;
        Ok(rows
            .into_iter()
            .map(|row| Table {
                name: row.get("name"),
                metadata: None,
            })
            .collect())
    }

    async fn fetch_table_metadata(&self, table_name: &str) -> Result<TableMetadata> {
        let columns_rows = sqlx::query(&format!("PRAGMA table_info('{}')", table_name))
            .fetch_all(self)
            .await?;
        let columns = columns_rows
            .iter()
            .map(|r| Column {
                name: r.get("name"),
                data_type: r.get("type"),
            })
            .collect();

        let indexes_rows = sqlx::query(&format!("PRAGMA index_list('{}')", table_name))
            .fetch_all(self)
            .await?;
        let indexes = indexes_rows.iter().map(|r| r.get("name")).collect();

        let triggers_rows =
            sqlx::query("SELECT name FROM sqlite_master WHERE type='trigger' AND tbl_name=?")
                .bind(table_name)
                .fetch_all(self)
                .await?;
        let triggers = triggers_rows.iter().map(|r| r.get("name")).collect();

        Ok(TableMetadata {
            name: table_name.to_string(),
            columns,
            constraints: vec![],
            indexes,
            rls_policies: vec![],
            rules: vec![],
            triggers,
            row_count: 0,
            estimated_size: "N/A".to_string(),
            table_type: "table".to_string(),
        })
    }

    async fn fetch_databases(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }
}

pub async fn fetch_tables(pool: &DbPool) -> Result<Vec<Table>> {
    match pool {
        DbPool::Postgres(pg) => pg.fetch_tables().await,
        DbPool::MySQL(mysql) => mysql.fetch_tables().await,
        DbPool::SQLite(sqlite) => sqlite.fetch_tables().await,
    }
}

pub async fn fetch_table_details(pool: &DbPool, table_name: &str) -> Result<TableMetadata> {
    match pool {
        DbPool::Postgres(pg) => pg.fetch_table_metadata(table_name).await,
        DbPool::MySQL(mysql) => mysql.fetch_table_metadata(table_name).await,
        DbPool::SQLite(sqlite) => sqlite.fetch_table_metadata(table_name).await,
    }
}

pub async fn fetch_databases(pool: &DbPool) -> Result<Vec<String>> {
    match pool {
        DbPool::Postgres(pg) => pg.fetch_databases().await,
        DbPool::MySQL(mysql) => mysql.fetch_databases().await,
        DbPool::SQLite(sqlite) => sqlite.fetch_databases().await,
    }
}

async fn get_pg_columns(pool: &PgPool, table: &str) -> sqlx::Result<Vec<Column>> {
    let rows = sqlx::query(
        "SELECT column_name, data_type FROM information_schema.columns WHERE table_schema = 'public' AND table_name = $1",
    )
    .bind(table)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Column {
            name: r.get("column_name"),
            data_type: r.get("data_type"),
        })
        .collect())
}

async fn get_pg_constraints(pool: &PgPool, table: &str) -> sqlx::Result<Vec<String>> {
    let rows = sqlx::query(
        "SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = $1 AND constraint_type != 'CHECK'",
    )
    .bind(table)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.get("constraint_name")).collect())
}

async fn get_pg_indexes(pool: &PgPool, table: &str) -> sqlx::Result<Vec<String>> {
    let rows = sqlx::query("SELECT indexname FROM pg_indexes WHERE tablename = $1")
        .bind(table)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|r| r.get("indexname")).collect())
}

async fn get_pg_rls_policies(pool: &PgPool, table: &str) -> sqlx::Result<Vec<String>> {
    let rows = sqlx::query("SELECT policyname FROM pg_policies WHERE tablename = $1")
        .bind(table)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|r| r.get("policyname")).collect())
}

async fn get_pg_rules(pool: &PgPool, table: &str) -> sqlx::Result<Vec<String>> {
    let rows = sqlx::query("SELECT rulename FROM pg_rules WHERE tablename = $1")
        .bind(table)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|r| r.get("rulename")).collect())
}

async fn get_pg_triggers(pool: &PgPool, table: &str) -> sqlx::Result<Vec<String>> {
    let rows = sqlx::query("SELECT tgname FROM pg_trigger JOIN pg_class ON tgrelid = pg_class.oid WHERE relname = $1 AND NOT tgisinternal")
        .bind(table)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|r| r.get("tgname")).collect())
}

pub fn build_category_node<T: Displayable>(
    parent: &str,
    label: &str,
    items: &[T],
) -> TreeItem<'static, String> {
    let id = format!("{}_{}", parent, label);
    if items.is_empty() {
        TreeItem::new_leaf(id.clone(), label.to_string())
    } else {
        let children = items
            .iter()
            .map(|item| {
                let child_id = format!("{}_{}", id, item.name());
                TreeItem::new_leaf(child_id, item.to_string())
            })
            .collect();

        TreeItem::new(id, label.to_string(), children).unwrap()
    }
}

pub fn metadata_to_tree_items(databases: &[Database]) -> Vec<TreeItem<'static, String>> {
    databases
        .iter()
        .map(|db| {
            let db_id = format!("db_{}", db.name);
            let tables_node = {
                let table_nodes = db
                    .tables
                    .iter()
                    .map(|table| {
                        let table_id = format!("tbl_{}_{}", &db.name, &table.name);
                        if let Some(metadata) = &table.metadata {
                            let columns_node = {
                                let column_nodes = metadata
                                    .columns
                                    .iter()
                                    .map(|column| {
                                        let column_id = format!("{}_col_{}", table_id, column.name);
                                        let sub_children = vec![
                                            TreeItem::new_leaf(
                                                format!("{}_columns", column_id),
                                                "Columns",
                                            ),
                                            TreeItem::new_leaf(
                                                format!("{}_constraints", column_id),
                                                "Constraints",
                                            ),
                                            TreeItem::new_leaf(
                                                format!("{}_other", column_id),
                                                "Other",
                                            ),
                                        ];
                                        TreeItem::new(column_id, column.to_string(), sub_children)
                                            .unwrap()
                                    })
                                    .collect::<Vec<_>>();
                                TreeItem::new(
                                    format!("{}_columns", table_id),
                                    "Columns",
                                    column_nodes,
                                )
                                .unwrap()
                            };

                            let children = vec![
                                columns_node,
                                build_category_node(
                                    &table_id,
                                    "Constraints",
                                    &metadata.constraints,
                                ),
                                build_category_node(&table_id, "Indexes", &metadata.indexes),
                                build_category_node(
                                    &table_id,
                                    "RLS Policies",
                                    &metadata.rls_policies,
                                ),
                                build_category_node(&table_id, "Rules", &metadata.rules),
                                build_category_node(&table_id, "Triggers", &metadata.triggers),
                            ];
                            TreeItem::new(
                                table_id.clone(),
                                Text::from(format!(
                                    "{} ({} row{})",
                                    metadata.name,
                                    metadata.row_count,
                                    if metadata.row_count == 1 { "" } else { "s" }
                                )),
                                children,
                            )
                            .unwrap()
                        } else {
                            TreeItem::new_leaf(table_id.clone(), table.name.clone())
                        }
                    })
                    .collect::<Vec<_>>();
                TreeItem::new(format!("{}_tables", db_id), "Tables", table_nodes).unwrap()
            };
            TreeItem::new(db_id, db.name.clone(), vec![tables_node]).unwrap()
        })
        .collect()
}
