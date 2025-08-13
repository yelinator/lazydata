use crate::crud::executor::{DataMeta, ExecutionResult, execute_query};
use crate::database::connections::{Connection, load_connections, save_connections};
use crate::database::fetch::{
    Database, TableMetadata, fetch_databases, fetch_table_details, fetch_tables,
    metadata_to_tree_items,
};
use crate::database::pool::DbPool;
use crate::database::{
    connector::{ConnectionDetails, DatabaseType},
    pool::pool,
};
use crate::layout::query_editor::QueryEditor;
use crate::layout::{data_table::DataTable, sidebar::SideBar};
use crate::state::{get_history, get_query_stats, load_history, save_history};
use color_eyre::eyre::Result;
use crossterm::execute;
use crossterm::{
    ExecutableCommand, cursor,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEvent},
    style::Print,
    terminal::{Clear, ClearType},
};
use inquire::{Confirm, Password, Select, Text};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, ScrollbarState},
};
use std::collections::HashMap;
use std::io::Write;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::{io::stdout, time::Duration};
use tokio::task::JoinHandle;
use tokio::time::sleep;

use tui_tree_widget::TreeItem;

use crate::command::Command;
use crate::components::popup::Popup;
use crate::key_maps::{DefaultKeyMapper, KeyMapper};
use crate::layout::key_map_guide::get_key_map_guide;
use crate::style::theme::{COLOR_UNFOCUSED, COLOR_WHITE};

#[derive(PartialEq, Debug, Clone)]
pub enum Focus {
    Sidebar,
    Editor,
    Table,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Focus::Sidebar => Focus::Editor,
            Focus::Editor => Focus::Table,
            Focus::Table => Focus::Sidebar,
        }
    }
}

pub struct App<'a> {
    pub focus: Focus,
    pub query: String,
    pub exit: bool,
    pub data_table: DataTable<'a>,
    pub query_editor: QueryEditor,
    pub sidebar: SideBar,
    pub pool: Option<DbPool>,
    pub connection_name: Option<String>,
    key_mapper: DefaultKeyMapper,
    pub show_key_map: bool,
    pub key_map_scroll: u16,
    key_map_scroll_state: ScrollbarState,
    connections: Vec<Connection>,
    databases: Vec<Database>,
    current_connection: Option<Connection>,
    table_details_cache: HashMap<String, TableMetadata>,
}

impl App<'_> {
    pub fn default() -> Self {
        Self {
            focus: Focus::Sidebar,
            query: String::new(),
            exit: false,
            data_table: DataTable::new(vec![], vec![], vec![]),
            query_editor: QueryEditor::new(),
            sidebar: SideBar::new(vec![], Focus::Sidebar),
            pool: None,
            connection_name: None,
            key_mapper: DefaultKeyMapper::new(),
            show_key_map: false,
            key_map_scroll: 0,
            key_map_scroll_state: ScrollbarState::default(),
            connections: Vec::new(),
            databases: Vec::new(),
            current_connection: None,
            table_details_cache: HashMap::new(),
        }
    }

    pub async fn init(&mut self) -> Result<()> {
        self.connections = load_connections()?;

        if self.connections.is_empty() {
            println!("No saved connections found.");
            let confirm_create = Confirm::new("Would you like to create a new connection?")
                .with_default(true)
                .prompt()?;
            if confirm_create {
                self.create_new_connection().await?;
            } else {
                println!("\n👋 Bye");
            }
        } else {
            self.select_connection().await?;
        }

        Ok(())
    }

    async fn create_new_connection(&mut self) -> Result<()> {
        let db_type = Select::new(
            "Select database type:",
            vec![
                DatabaseType::PostgreSQL,
                DatabaseType::MySQL,
                DatabaseType::SQLite,
            ],
        )
        .prompt()?;

        let name = Text::new("Connection Name:").prompt()?;
        let host = Text::new("Host:").prompt()?;
        let user = Text::new("User:").prompt()?;
        let password = Password::new("Password:").prompt()?;
        let save_password = Confirm::new("Save password?")
            .with_default(false)
            .prompt()?;

        let new_connection = Connection {
            name,
            host,
            user,
            password: if save_password { Some(password) } else { None },
            db_type,
        };

        self.connections.push(new_connection.clone());
        save_connections(&self.connections)?;
        self.current_connection = Some(new_connection.clone());

        self.setup_and_run_app(new_connection).await?;
        Ok(())
    }

    async fn select_connection(&mut self) -> Result<()> {
        let mut options = self
            .connections
            .iter()
            .map(|c| c.name.clone())
            .collect::<Vec<_>>() as Vec<String>;
        options.push("Create new connection".to_string());

        let selected = Select::new("Select a connection:", options).prompt()?;

        if selected == "Create new connection" {
            self.create_new_connection().await?;
        } else {
            let mut connection = self
                .connections
                .iter()
                .find(|c| c.name == selected)
                .cloned()
                .unwrap();

            if connection.password.is_none() {
                connection.password = Some(Password::new("Password:").prompt()?);
            }
            self.current_connection = Some(connection.clone());
            self.setup_and_run_app(connection).await?;
        }
        Ok(())
    }

    fn current_query(&self) -> String {
        self.query_editor.textarea_content()
    }

    async fn setup_and_run_app(&mut self, connection: Connection) -> Result<()> {
        let details = ConnectionDetails {
            host: Some(connection.host.clone()),
            user: Some(connection.user.clone()),
            password: connection.password.clone(),
            database: None,
        };
        self.connection_name = Some(connection.name.clone());
        load_history().await?;
        self.data_table.query_history = get_history(self.connection_name.clone()).await;
        let pool = pool(connection.db_type, &details, None).await?;
        self.pool = Some(pool.clone());

        let (spinner_handle, loading) = self.loading().await;
        let databases = fetch_databases(&pool).await?;
        let mut db_vec = Vec::new();
        for db_name in &databases {
            db_vec.push(Database {
                name: db_name.clone(),
                tables: vec![],
            });
        }
        self.databases = db_vec;
        loading.store(false, Ordering::SeqCst);
        spinner_handle.await.unwrap();

        if self.databases.is_empty() {
            println!("❌ No databases found on the server.");
            return Ok(());
        }

        println!("✅ Found {} databases", self.databases.len());
        let items = metadata_to_tree_items(&self.databases);
        self.setup_ui(items).await?;

        stdout().execute(EnableMouseCapture)?;
        let terminal = ratatui::init();
        let _ = self.run(terminal).await;
        ratatui::restore();
        stdout().execute(DisableMouseCapture)?;
        Ok(())
    }

    pub async fn loading(&mut self) -> (JoinHandle<()>, Arc<AtomicBool>) {
        let loading = Arc::new(AtomicBool::new(true));
        let spinner_flag = loading.clone();

        let spinner_handle = tokio::spawn(async move {
            let spinner = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let mut i = 0;
            let mut stdout = stdout();

            while spinner_flag.load(Ordering::SeqCst) {
                execute!(
                    stdout,
                    cursor::MoveToColumn(0),
                    Clear(ClearType::CurrentLine),
                    Print(format!(
                        "🔄 Fetching databases... {}",
                        spinner[i % spinner.len()]
                    )),
                )
                .unwrap();
                stdout.flush().unwrap();
                sleep(Duration::from_millis(100)).await;
                i += 1;
            }

            execute!(
                stdout,
                cursor::MoveToColumn(0),
                Clear(ClearType::CurrentLine),
            )
            .unwrap();
        });
        (spinner_handle, loading)
    }

    async fn setup_ui(&mut self, sidebar_items: Vec<TreeItem<'static, String>>) -> Result<()> {
        self.focus = Focus::Sidebar;
        self.sidebar.update_items(sidebar_items);
        self.sidebar.update_focus(self.focus.clone());

        Ok(())
    }

    fn draw_once(&mut self, terminal: &mut DefaultTerminal) {
        let _ = terminal.draw(|f| self.render_ui(f));
    }

    pub async fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        while !self.exit {
            terminal.draw(|f| self.render_ui(f))?;
            let _ = self.handle_events(&mut terminal).await;
        }
        save_history().await?;
        Ok(())
    }

    async fn handle_events(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key_event) = event::read()? {
                let command = if self.show_key_map {
                    self.key_mapper.map_popup_key(key_event)
                } else {
                    self.key_mapper.map_key_to_command(
                        key_event,
                        &self.focus,
                        self.data_table.tabs.index,
                    )
                };

                if let Some(command) = command {
                    self.handle_command(command, key_event, terminal).await?;
                    self.query_editor.mode = self.key_mapper.editor_mode();
                }
            }
        }
        Ok(())
    }

    async fn execute_current_query(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let query = self.current_query();
        if !query.is_empty() {
            self.query = query.clone();

            self.data_table.start_loading();
            self.draw_once(terminal);

            if let Some(pool) = &self.pool {
                match execute_query(pool, &query, self.connection_name.clone()).await {
                    Ok(ExecutionResult::Data {
                        headers,
                        rows,
                        meta: DataMeta { rows: _, message },
                    }) => {
                        let elapsed_duration = if let Some(stats) = get_query_stats().await {
                            stats.elapsed
                        } else {
                            Duration::ZERO
                        };
                        self.data_table.query_history =
                            get_history(self.connection_name.clone()).await;
                        self.data_table
                            .finish_loading(headers, rows, elapsed_duration);
                        self.data_table.status_message = Some(message);
                    }
                    Ok(ExecutionResult::Affected { rows: _, message }) => {
                        let elapsed_duration = if let Some(stats) = get_query_stats().await {
                            stats.elapsed
                        } else {
                            Duration::ZERO
                        };
                        self.data_table.query_history =
                            get_history(self.connection_name.clone()).await;
                        self.data_table
                            .finish_loading(Vec::new(), Vec::new(), elapsed_duration);
                        self.data_table.status_message = Some(message);
                    }
                    Err(err) => {
                        self.data_table
                            .set_error_state(format!("❌ Error: {}", err));
                    }
                }
            } else {
                // Handle the case where the pool is not available (e.g., not connected to a DB)
                self.data_table
                    .set_error_state("Database connection pool not available.".to_string());
            }
        }
        Ok(())
    }

    async fn handle_command(
        &mut self,
        command: Command,
        key_event: KeyEvent,
        terminal: &mut DefaultTerminal,
    ) -> Result<()> {
        match command {
            // Global Commands
            Command::Quit => {
                self.exit = true;
            }
            Command::ShowKeyMap => {
                self.show_key_map = true;
                self.key_map_scroll = 0; // Reset scroll when showing
            }
            Command::ClosePopup => {
                self.show_key_map = false;
            }
            Command::KeyMapScrollUp => {
                self.key_map_scroll = self.key_map_scroll.saturating_sub(1);
            }
            Command::KeyMapScrollDown => {
                self.key_map_scroll = self.key_map_scroll.saturating_add(1);
            }
            Command::ToggleFocus => {
                self.toggle_focus();
            }
            Command::ExecuteQuery => {
                self.execute_current_query(terminal).await?;
            }

            Command::DataTablePreviousTab
            | Command::DataTableNextTab
            | Command::DataTableNextRow
            | Command::DataTablePreviousRow
            | Command::DataTableNextHistoryRow
            | Command::DataTablePreviousHistoryRow
            | Command::DataTableScrollRight
            | Command::DataTableScrollLeft
            | Command::DataTableNextColor
            | Command::DataTablePreviousColor
            | Command::DataTableNextPage
            | Command::DataTablePreviousPage
            | Command::DataTableJumpToFirstRow
            | Command::DataTableJumpToLastRow
            | Command::DataTableNextColumn
            | Command::DataTablePreviousColumn
            | Command::DataTableAdjustColumnWidthIncrease
            | Command::DataTableAdjustColumnWidthDecrease
            | Command::DataTableCopySelectedCell
            | Command::DataTableCopySelectedRow
            | Command::DataTableCopyQueryToEditor => {
                self.data_table.handle_command(command);
            }
            Command::DataTableRunSelectedHistoryQuery => {
                if let Some(query) = self.data_table.get_selected_history_query() {
                    self.query_editor.set_textarea_content(
                        query,
                        &self.focus,
                        self.connection_name.clone(),
                    );
                    self.execute_current_query(terminal).await?;
                }
            }
            Command::DataTableSetTabIndex(idx) => {
                if idx < self.data_table.tabs.titles.len() {
                    self.data_table.tabs.set_index(idx);
                }
            }

            Command::SidebarToggleSelected => {
                if let Some(identifier) = self.sidebar.handle_command(command) {
                    if identifier.starts_with("db_") {
                        let db_name = identifier.strip_prefix("db_").unwrap().to_string();
                        if let Some(db) = self.databases.iter_mut().find(|db| db.name == db_name) {
                            if db.tables.is_empty() {
                                // Only fetch if not already fetched
                                if let Some(connection) = &self.current_connection {
                                    let details = ConnectionDetails {
                                        host: Some(connection.host.clone()),
                                        user: Some(connection.user.clone()),
                                        password: connection.password.clone(),
                                        database: Some(db_name.clone()),
                                    };
                                    let pool =
                                        pool(connection.db_type, &details, Some(&db_name)).await?;
                                    self.pool = Some(pool.clone());
                                    let tables = fetch_tables(&pool).await?;
                                    db.tables = tables;
                                    let items = metadata_to_tree_items(&self.databases);
                                    self.sidebar.update_items(items);
                                }
                            }
                        }
                    } else if identifier.starts_with("tbl_") {
                        let parts: Vec<&str> = identifier.split('_').collect();
                        let db_name = parts[1].to_string();
                        let table_name = parts[2].to_string();

                        let cache_key = format!("{}/{}", db_name, table_name);

                        if let Some(metadata) = self.table_details_cache.get(&cache_key) {
                            if let Some(db) =
                                self.databases.iter_mut().find(|db| db.name == db_name)
                            {
                                if let Some(table) =
                                    db.tables.iter_mut().find(|t| t.name == table_name)
                                {
                                    table.metadata = Some(metadata.clone());
                                }
                            }
                        } else if let Some(pool) = &self.pool {
                            let metadata = fetch_table_details(pool, &table_name).await?;
                            self.table_details_cache.insert(cache_key, metadata.clone());
                            if let Some(db) =
                                self.databases.iter_mut().find(|db| db.name == db_name)
                            {
                                if let Some(table) =
                                    db.tables.iter_mut().find(|t| t.name == table_name)
                                {
                                    table.metadata = Some(metadata);
                                }
                            }
                        }
                        let items = metadata_to_tree_items(&self.databases);
                        self.sidebar.update_items(items);
                    }
                }
            }

            Command::SidebarKeyLeft
            | Command::SidebarKeyRight
            | Command::SidebarKeyDown
            | Command::SidebarKeyUp
            | Command::SidebarDeselect
            | Command::SidebarSelectFirst
            | Command::SidebarSelectLast
            | Command::SidebarScrollDown(_)
            | Command::SidebarScrollUp(_) => {
                self.sidebar.handle_command(command);
            }

            Command::EditorInputChar(_)
            | Command::EditorInputBackspace
            | Command::EditorInputDelete
            | Command::EditorInputEnter
            | Command::EditorMoveCursor(_)
            | Command::EditorDeleteLineByEnd
            | Command::EditorCancelSelection
            | Command::EditorPaste
            | Command::EditorUndo
            | Command::EditorRedo
            | Command::EditorDeleteNextChar
            | Command::EditorSetMode(_)
            | Command::EditorScrollRelative(_, _)
            | Command::EditorScroll(_)
            | Command::EditorStartSelection
            | Command::EditorCopySelection
            | Command::EditorCutSelection
            | Command::EditorPerformPendingOperator => {
                self.query_editor.handle_command(command, key_event);
            }
            Command::NoOp => { /* No operation, do nothing */ }
        }
        Ok(())
    }

    fn render_ui(&mut self, f: &mut Frame) {
        let outer_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(2)])
            .split(f.area());

        let top_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(outer_chunks[0]);

        self.sidebar.render(f, top_chunks[0]);

        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(top_chunks[1]);

        self.query_editor.draw(
            f,
            right_chunks[0],
            self.focus.clone(),
            self.connection_name.clone(),
        );

        self.data_table
            .draw(f, right_chunks[1], &self.focus.clone());

        let focus_text = Line::from(vec![
            /* Span::styled(
                format!(" Focus: {} ", self.focus.as_str()),
                Style::default()
                    .bg(COLOR_HIGHLIGHT_BG)
                    .fg(COLOR_BLACK)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" (Tab to change) "), */
            Span::styled(
                " q: Quit ",
                Style::default().bg(COLOR_UNFOCUSED).fg(COLOR_WHITE),
            ),
            Span::styled(
                " F5: Execute Query ",
                Style::default().bg(COLOR_UNFOCUSED).fg(COLOR_WHITE),
            ),
            Span::styled(
                " ?: Key Maps ",
                Style::default().bg(COLOR_UNFOCUSED).fg(COLOR_WHITE),
            ),
        ]);

        let status_block = Paragraph::new(focus_text)
            .block(Block::default().borders(Borders::TOP))
            .style(Style::default().fg(COLOR_WHITE).bg(Color::Black));

        f.render_widget(status_block, outer_chunks[1]);

        if self.show_key_map {
            let popup = Popup::new(
                "Key Maps",
                get_key_map_guide(),
                self.key_map_scroll,
                &mut self.key_map_scroll_state,
            );
            f.render_widget(popup, f.area());
        }
    }

    fn toggle_focus(&mut self) {
        self.focus = self.focus.clone().next();
        self.sidebar.update_focus(self.focus.clone());
    }
}
