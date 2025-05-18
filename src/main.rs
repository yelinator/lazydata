mod app;
mod crud;
mod database;
mod layout;
mod style;
mod utils;

use app::App;
use color_eyre::eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let mut app = App::default();
    app.init().await?;
    Ok(())
}
