use crate::crud::executor::{DataMeta, ExecutionResult, execute_query};
use crate::database::fetch::metadata_to_tree_items;
use crate::database::pool::DbPool;
use crate::database::{
    connector::{ConnectionDetails, DatabaseType, get_connection_details},
    detector::get_installed_databases,
    fetch::fetch_all_table_metadata,
    pool::pool,
};
use crate::layout::query_editor::{Mode, QueryEditor};
use crate::layout::{data_table::DataTable, sidebar::SideBar};
use crate::state::get_query_stats;
use color_eyre::eyre::Result;
use crossterm::execute;
use crossterm::{
    ExecutableCommand, cursor,
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    style::Print,
    terminal::{Clear, ClearType},
};
use inquire::Select;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::io::Write;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::{io::stdout, time::Duration};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tui_textarea::Input;
use tui_tree_widget::TreeItem;

use crate::command::Command;
use crate::key_maps::{DefaultKeyMapper, KeyMapper};
use crate::style::theme::{COLOR_BLACK, COLOR_HIGHLIGHT_BG, COLOR_UNFOCUSED, COLOR_WHITE};

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

    pub fn as_str(&self) -> &str {
        match self {
            Focus::Sidebar => "Sidebar",
            Focus::Editor => "Editor",
            Focus::Table => "Table",
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
    key_mapper: DefaultKeyMapper,
}

impl App<'_> {
    pub fn default() -> Self {
        Self {
            focus: Focus::Sidebar,
            query: String::new(),
            exit: false,
            data_table: DataTable::new(vec![], vec![]),
            query_editor: QueryEditor::new(),
            sidebar: SideBar::new(vec![], Focus::Sidebar),
            pool: None,
            key_mapper: DefaultKeyMapper::new(),
        }
    }

    pub async fn init(&mut self) -> Result<()> {
        let databases = get_installed_databases()?;

        if databases.is_empty() {
            println!("❌ No databases detected!");
            return Ok(());
        }

        let selected = Select::new("🚀 Select a Database", databases.clone())
            .with_help_message("Use ↑ ↓ arrows, Enter to select")
            .prompt();

        if let Ok(db_name) = selected {
            if let Some(db_type) = Self::map_db_name_to_type(&db_name) {
                self.setup_and_run_app(db_type).await?;
            } else {
                println!("❌ Unsupported database.");
            }
        } else {
            println!("\n👋 Bye");
        }

        Ok(())
    }

    fn map_db_name_to_type(name: &str) -> Option<DatabaseType> {
        match name.to_lowercase().as_str() {
            "postgresql" => Some(DatabaseType::PostgreSQL),
            "mysql" => Some(DatabaseType::MySQL),
            "sqlite" => Some(DatabaseType::SQLite),
            _ => None,
        }
    }

    fn current_query(&self) -> String {
        self.query_editor.textarea_content()
    }

    async fn setup_and_run_app(&mut self, db_type: DatabaseType) -> Result<()> {
        let details: ConnectionDetails = get_connection_details(db_type)?;
        let pool = pool(db_type, &details).await?;

        self.pool = Some(pool.clone());

        let (spinner_handle, loading) = self.loading().await;
        let metadata = fetch_all_table_metadata(&pool).await?;
        loading.store(false, Ordering::SeqCst);
        spinner_handle.await.unwrap();

        if metadata.is_empty() {
            println!("❌ No tables found in the database.");
            return Ok(());
        }

        println!("✅ Found {} tables", metadata.len());
        let items = metadata_to_tree_items(&metadata);
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
                        "🔄 Fetching tables... {}",
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
        self.sidebar.update_focus(Focus::Sidebar);

        Ok(())
    }

    pub async fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        while !self.exit {
            terminal.draw(|f| self.render_ui(f))?;
            let _ = self.handle_events().await;
        }
        Ok(())
    }

    async fn handle_events(&mut self) -> Result<()> {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key_event) = event::read()? {
                if let Some(command) = self.key_mapper.map_key_to_command(key_event, &self.focus) {
                    self.handle_command(command).await?;
                    self.query_editor.mode = self.key_mapper.editor_mode();
                }
            }
        }
        Ok(())
    }

    async fn handle_command(&mut self, command: Command) -> Result<()> {
        match command {
            // Global Commands
            Command::Quit => {
                self.exit = true;
            }
            Command::ToggleFocus => {
                self.toggle_focus();
            }
            Command::ExecuteQuery => {
                let query = self.current_query();
                if !query.is_empty() {
                    self.query = query.clone();

                    if let Some(pool) = &self.pool {
                        match execute_query(pool, &query).await {
                            Ok(ExecutionResult::Data(data, DataMeta { rows: _, message })) => {
                                self.data_table =
                                    DataTable::new(data.headers.clone(), data.rows.clone());
                                self.data_table.status_message = Some(message);
                                if let Some(stats) = get_query_stats().await {
                                    self.data_table.elapsed = stats.elapsed
                                }
                            }
                            Ok(ExecutionResult::Affected { rows: _, message }) => {
                                self.data_table.status_message = Some(message);
                                if let Some(stats) = get_query_stats().await {
                                    self.data_table.elapsed = stats.elapsed
                                }
                            }
                            Err(err) => {
                                self.data_table.tabs.set_index(1);
                                self.data_table.status_message = Some(format!("❌ Error: {}", err));
                            }
                        }
                    }
                }
            }

            // Data Table Commands
            Command::DataTablePreviousTab => self.data_table.tabs.previous(),
            Command::DataTableNextTab => self.data_table.tabs.next(),
            Command::DataTableNextRow => self.data_table.next_row(),
            Command::DataTablePreviousRow => self.data_table.previous_row(),
            Command::DataTableScrollRight => self.data_table.scroll_right(),
            Command::DataTableScrollLeft => self.data_table.scroll_left(),
            Command::DataTableNextColor => self.data_table.next_color(),
            Command::DataTablePreviousColor => self.data_table.previous_color(),
            Command::DataTableNextPage => self.data_table.next_page(),
            Command::DataTablePreviousPage => self.data_table.previous_page(),
            Command::DataTableJumpToFirstRow => self.data_table.jump_to_absolute_row(0),
            Command::DataTableJumpToLastRow => self
                .data_table
                .jump_to_absolute_row(self.data_table.data.len().saturating_sub(1)),
            Command::DataTableNextColumn => self.data_table.next_column(),
            Command::DataTablePreviousColumn => self.data_table.previous_column(),
            Command::DataTableAdjustColumnWidthIncrease => self.data_table.adjust_column_width(1),
            Command::DataTableAdjustColumnWidthDecrease => self.data_table.adjust_column_width(-1),
            Command::DataTableCopySelectedCell => {
                if let Some(content) = self.data_table.copy_selected_cell() {
                    self.data_table.status_message = Some(format!("Copied: {}", content));
                }
            }
            Command::DataTableCopySelectedRow => {
                if let Some(content) = self.data_table.copy_selected_row() {
                    self.data_table.status_message = Some(format!("Copied row: {}", content));
                }
            }
            Command::DataTableSetTabIndex(idx) => {
                if idx < self.data_table.tabs.titles.len() {
                    self.data_table.tabs.set_index(idx);
                }
            }

            // Sidebar Commands
            Command::SidebarToggleSelected => {
                self.sidebar.state.toggle_selected();
            }
            Command::SidebarKeyLeft => {
                self.sidebar.state.key_left();
            }
            Command::SidebarKeyRight => {
                self.sidebar.state.key_right();
            }
            Command::SidebarKeyDown => {
                self.sidebar.state.key_down();
            }
            Command::SidebarKeyUp => {
                self.sidebar.state.key_up();
            }
            Command::SidebarDeselect => {
                self.sidebar.state.select(Vec::new());
            }
            Command::SidebarSelectFirst => {
                self.sidebar.state.select_first();
            }
            Command::SidebarSelectLast => {
                self.sidebar.state.select_last();
            }
            Command::SidebarScrollDown(amount) => {
                self.sidebar.state.scroll_down(amount as usize);
            }
            Command::SidebarScrollUp(amount) => {
                self.sidebar.state.scroll_up(amount as usize);
            }

            Command::EditorInputChar(c) => {
                let key_event = KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
                self.query_editor.input(Input::from(key_event));
            }
            Command::EditorInputBackspace => {
                let key_event = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
                self.query_editor.input(Input::from(key_event));
            }
            Command::EditorInputDelete => {
                let key_event = KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE);
                self.query_editor.input(Input::from(key_event));
            }
            Command::EditorInputEnter => {
                let key_event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
                self.query_editor.input(Input::from(key_event));
            }
            Command::EditorMoveCursor(move_action) => {
                self.query_editor.textarea.move_cursor(move_action);
            }
            Command::EditorDeleteLineByEnd => {
                self.query_editor.textarea.delete_line_by_end();
            }
            Command::EditorCancelSelection => {
                self.query_editor.textarea.cancel_selection();
            }
            Command::EditorPaste => {
                self.query_editor.textarea.paste();
            }
            Command::EditorUndo => {
                self.query_editor.textarea.undo();
            }
            Command::EditorRedo => {
                self.query_editor.textarea.redo();
            }
            Command::EditorDeleteNextChar => {
                self.query_editor.textarea.delete_next_char();
            }
            Command::EditorSetMode(mode) => {
                self.query_editor.mode = mode;
            }
            Command::EditorScrollRelative(rows, cols) => {
                self.query_editor.textarea.scroll((rows, cols));
            }
            Command::EditorScroll(scrolling_action) => {
                self.query_editor.textarea.scroll(scrolling_action);
            }
            Command::EditorStartSelection => {
                self.query_editor.textarea.start_selection();
            }
            Command::EditorCopySelection => {
                self.query_editor.textarea.copy();
            }
            Command::EditorCutSelection => {
                self.query_editor.textarea.cut();
            }
            Command::EditorPerformPendingOperator => {
                self.query_editor.textarea.cancel_selection();
                self.query_editor.mode = Mode::Normal;
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

        self.query_editor
            .draw(f, right_chunks[0], self.focus.clone());

        self.data_table
            .draw(f, right_chunks[1], &self.focus.clone());

        let focus_text = Line::from(vec![
            Span::styled(
                format!(" Focus: {} ", self.focus.as_str()),
                Style::default()
                    .bg(COLOR_HIGHLIGHT_BG)
                    .fg(COLOR_BLACK)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" (Tab to change) "),
            Span::styled(
                " q: Quit ",
                Style::default().bg(COLOR_UNFOCUSED).fg(COLOR_WHITE),
            ),
            Span::styled(
                " F5: Execute Query ",
                Style::default().bg(COLOR_UNFOCUSED).fg(COLOR_WHITE),
            ),
        ]);

        let status_block = Paragraph::new(focus_text)
            .block(Block::default().borders(Borders::TOP))
            .style(Style::default().fg(COLOR_WHITE).bg(Color::Black));

        f.render_widget(status_block, outer_chunks[1]);
    }

    fn toggle_focus(&mut self) {
        self.focus = self.focus.clone().next();
        self.sidebar.update_focus(self.focus.clone());
    }
}
