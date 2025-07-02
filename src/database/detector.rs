use color_eyre::eyre::Result;
use std::process::Command;

#[derive(Debug)]
pub struct DatabaseChecker {
    pub name: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
}

pub fn get_installed_databases() -> Result<Vec<String>> {
    let db_tools = [
        DatabaseChecker {
            name: "PostgreSQL",
            command: "pg_isready",
            args: &[],
        },
        DatabaseChecker {
            name: "MySQL",
            command: "mysql",
            args: &["--version"],
        },
        DatabaseChecker {
            name: "SQLite",
            command: "sqlite3",
            args: &["--version"],
        },
    ];

    let mut found = Vec::new();

    for tool in db_tools.iter() {
        if Command::new(tool.command)
            .args(tool.args)
            .output()
            .map_or(false, |output| output.status.success())
        {
            found.push(tool.name.to_string());
        }
    }

    if found.is_empty() {
        found.push("No databases found in your system.".to_string());
    }
    Ok(found)
}
