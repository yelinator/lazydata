use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryHistoryEntry {
    pub query: String,
    #[serde(default)]
    pub connection_name: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub success: bool,
    pub rows_affected: usize,
    pub execution_time: Duration,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct QueryStats {
    pub rows: usize,
    pub elapsed: Duration,
}

pub static GLOBAL_QUERY_STATS: Lazy<RwLock<Option<QueryStats>>> = Lazy::new(|| RwLock::new(None));
pub static GLOBAL_QUERY_HISTORY: Lazy<RwLock<Vec<QueryHistoryEntry>>> =
    Lazy::new(|| RwLock::new(Vec::new()));

fn get_history_file_path() -> Option<PathBuf> {
    dirs::home_dir().map(|mut path| {
        path.push(".lazydata");
        path.push("history.json");
        path
    })
}

pub async fn save_history() -> io::Result<()> {
    if let Some(path) = get_history_file_path() {
        let history = GLOBAL_QUERY_HISTORY.read().await;
        match serde_json::to_string_pretty(&*history) {
            Ok(json) => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                match std::fs::File::create(&path) {
                    Ok(mut file) => match file.write_all(json.as_bytes()) {
                        Ok(_) => {}
                        Err(e) => eprintln!("Error writing history to file {:?}: {}", path, e),
                    },
                    Err(e) => eprintln!("Error creating history file {:?}: {}", path, e),
                }
            }
            Err(e) => eprintln!("Error serializing history: {}", e),
        }
    }
    Ok(())
}

pub async fn load_history() -> io::Result<()> {
    if let Some(path) = get_history_file_path() {
        if path.exists() {
            match std::fs::File::open(&path) {
                Ok(mut file) => {
                    let mut json = String::new();
                    match file.read_to_string(&mut json) {
                        Ok(_) => match serde_json::from_str::<Vec<QueryHistoryEntry>>(&json) {
                            Ok(history) => {
                                let mut global_history = GLOBAL_QUERY_HISTORY.write().await;
                                *global_history = history;
                            }
                            Err(e) => {
                                eprintln!("Error deserializing history from {:?}: {}", path, e)
                            }
                        },
                        Err(e) => eprintln!("Error reading history file {:?}: {}", path, e),
                    }
                }
                Err(e) => eprintln!("Error opening history file {:?}: {}", path, e),
            }
        } else {
            // eprintln!("History file does not exist at {:?}", path);
        }
    }
    Ok(())
}

pub async fn update_query_stats(rows: usize, elapsed: Duration) {
    let mut stats = GLOBAL_QUERY_STATS.write().await;
    *stats = Some(QueryStats { rows, elapsed })
}

pub async fn get_query_stats() -> Option<QueryStats> {
    let stats = GLOBAL_QUERY_STATS.read().await;
    stats.clone()
}

pub async fn add_to_history(entry: QueryHistoryEntry) {
    let mut history = GLOBAL_QUERY_HISTORY.write().await;
    history.push(entry);
}

pub async fn get_history(connection_name: Option<String>) -> Vec<QueryHistoryEntry> {
    let history = GLOBAL_QUERY_HISTORY.read().await;
    if let Some(name) = connection_name {
        history
            .iter()
            .filter(|entry| entry.connection_name.as_deref() == Some(name.as_str()))
            .cloned()
            .collect()
    } else {
        history.clone()
    }
}
