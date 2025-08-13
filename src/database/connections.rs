use crate::database::connector::DatabaseType;
use color_eyre::eyre::{Result, WrapErr};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Connection {
    pub name: String,
    pub host: String,
    pub user: String,
    pub password: Option<String>,
    pub db_type: DatabaseType,
}

fn get_connections_file_path() -> Result<PathBuf> {
    let mut config_path =
        config_dir().ok_or_else(|| color_eyre::eyre::eyre!("Could not find config directory"))?;
    config_path.push("lazydata");
    fs::create_dir_all(&config_path)?;
    config_path.push("connections.json");
    Ok(config_path)
}

pub fn save_connections(connections: &[Connection]) -> Result<()> {
    let path = get_connections_file_path()?;
    let json =
        serde_json::to_string_pretty(connections).wrap_err("Failed to serialize connections")?;
    let mut file = File::create(path).wrap_err("Failed to create connections file")?;
    file.write_all(json.as_bytes())
        .wrap_err("Failed to write to connections file")?;
    Ok(())
}

pub fn load_connections() -> Result<Vec<Connection>> {
    let path = get_connections_file_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut file = File::open(path).wrap_err("Failed to open connections file")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .wrap_err("Failed to read connections file")?;
    let connections =
        serde_json::from_str(&contents).wrap_err("Failed to deserialize connections")?;
    Ok(connections)
}
