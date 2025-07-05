use crate::app::Focus;
use crate::command::Command;
use crate::components::tabs::StatefulTabs;
use crate::state::QueryHistoryEntry;
use crate::style::theme::COLOR_BLOCK_BG;
use crate::style::{DefaultStyle, StyleProvider};
use arboard::Clipboard;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::palette::tailwind;
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, Borders, Cell, HighlightSpacing, Paragraph, Row, Scrollbar, ScrollbarOrientation,
    ScrollbarState, Table, TableState, Tabs,
};
use ratatui::{Frame, symbols};
use serde_json::Value;
use sqlx::{Row as SqlxRow, postgres::PgRow, types::Json};
use std::collections::HashMap;
use std::time::Duration;
use unicode_width::UnicodeWidthStr;

const PALETTES: [tailwind::Palette; 4] = [
    tailwind::BLUE,
    tailwind::EMERALD,
    tailwind::INDIGO,
    tailwind::RED,
];

const ITEM_HEIGHT: usize = 1;

struct TableColors {
    buffer_bg: Color,
    header_bg: Color,
    header_fg: Color,
    row_fg: Color,
    selected_row_style_fg: Color,
    selected_column_style_fg: Color,
    selected_cell_style_fg: Color,
}

impl TableColors {
    const fn new(color: &tailwind::Palette) -> Self {
        Self {
            buffer_bg: tailwind::SLATE.c950,
            header_bg: color.c900,
            header_fg: tailwind::SLATE.c200,
            row_fg: tailwind::SLATE.c200,
            selected_row_style_fg: color.c400,
            selected_column_style_fg: color.c400,
            selected_cell_style_fg: color.c600,
        }
    }
}

pub struct DataTable<'a> {
    state: TableState,
    pub history_table_state: TableState,
    pub headers: Vec<String>,
    pub rows: Vec<PgRow>,
    pub query_history: Vec<QueryHistoryEntry>,
    pub column_widths: Vec<u16>,
    pub min_column_widths: Vec<u16>,
    vertical_scroll_state: ScrollbarState,
    horizontal_scroll_state: ScrollbarState,
    horizontal_scroll: usize,
    colors: TableColors,
    color_index: usize,
    pub tabs: StatefulTabs<'a>,
    pub status_message: Option<String>,
    pub elapsed: Duration,
    page_size: usize,
    pub current_page: usize,
    pub loading_state: LoadingState,
}

pub enum LoadingState {
    Idle,
    Loading,
    Error(String),
}

impl<'a> DataTable<'a> {
    pub fn new(
        headers: Vec<String>,
        rows: Vec<PgRow>,
        query_history: Vec<QueryHistoryEntry>,
    ) -> Self {
        let mut tabs = StatefulTabs::new(vec!["Data Output", "Messages", "Query History"]);
        if rows.is_empty() {
            tabs.set_index(1);
        }

        let (column_widths, min_column_widths) = Self::calculate_column_widths(&headers, &rows);

        Self {
            state: TableState::default().with_selected(if rows.is_empty() {
                None
            } else {
                Some(0)
            }),
            history_table_state: TableState::default(),
            vertical_scroll_state: ScrollbarState::new(
                (rows.len().min(100).saturating_sub(1)) * ITEM_HEIGHT,
            ),
            horizontal_scroll_state: ScrollbarState::new(
                column_widths.iter().sum::<u16>().saturating_sub(1) as usize,
            ),
            colors: TableColors::new(&PALETTES[0]),
            color_index: 0,
            horizontal_scroll: 0,
            headers,
            rows,
            query_history,
            column_widths,
            min_column_widths,
            tabs,
            status_message: None,
            elapsed: Duration::ZERO,
            page_size: 100,
            current_page: 0,
            loading_state: LoadingState::Idle,
        }
    }

    fn calculate_column_widths(headers: &[String], rows: &[PgRow]) -> (Vec<u16>, Vec<u16>) {
        let mut widths: Vec<u16> = headers.iter().map(|h| h.width() as u16).collect();

        let sample_size = 100;
        for row in rows.iter().take(std::cmp::min(rows.len(), sample_size)) {
            for (col_idx, col_width) in widths.iter_mut().enumerate() {
                let val = Self::get_value_as_string(row, col_idx);
                *col_width = (*col_width).max(val.width() as u16);
            }
        }

        let final_widths: Vec<u16> = widths.iter().map(|&w| w.saturating_add(2).max(3)).collect();
        (final_widths.clone(), final_widths)
    }

    fn get_value_as_string(row: &PgRow, index: usize) -> String {
        macro_rules! try_get_string {
            ($($typ:ty),*) => {
                $(
                    if let Ok(val) = row.try_get::<$typ, _>(index) {
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
            sqlx::types::Uuid,
            sqlx::types::chrono::NaiveDate,
            sqlx::types::chrono::NaiveDateTime,
            sqlx::types::chrono::NaiveTime,
            sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>
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

    pub fn handle_command(&mut self, command: Command) {
        match command {
            Command::DataTablePreviousTab => self.tabs.previous(),
            Command::DataTableNextTab => self.tabs.next(),
            Command::DataTableNextRow => self.next_row(),
            Command::DataTablePreviousRow => self.previous_row(),
            Command::DataTableNextHistoryRow => self.next_history_row(),
            Command::DataTablePreviousHistoryRow => self.previous_history_row(),
            Command::DataTableScrollRight => self.scroll_right(),
            Command::DataTableScrollLeft => self.scroll_left(),
            Command::DataTableNextColor => self.next_color(),
            Command::DataTablePreviousColor => self.previous_color(),
            Command::DataTableNextPage => self.next_page(),
            Command::DataTablePreviousPage => self.previous_page(),
            Command::DataTableJumpToFirstRow => self.jump_to_absolute_row(0),
            Command::DataTableJumpToLastRow => {
                self.jump_to_absolute_row(self.rows.len().saturating_sub(1))
            }
            Command::DataTableNextColumn => self.next_column(),
            Command::DataTablePreviousColumn => self.previous_column(),
            Command::DataTableAdjustColumnWidthIncrease => self.adjust_column_width(1),
            Command::DataTableAdjustColumnWidthDecrease => self.adjust_column_width(-1),
            Command::DataTableCopySelectedCell => {
                if let Some(content) = self.copy_selected_cell() {
                    self.status_message = Some(format!("Copied: {}", content));
                }
            }
            Command::DataTableCopySelectedRow => {
                if let Some(content) = self.copy_selected_row() {
                    self.status_message = Some(format!("Copied row: {}", content));
                }
            }
            Command::DataTableCopyQueryToEditor => {
                if let Some(query) = self.copy_selected_query_to_editor() {
                    self.status_message = Some(format!("Copied query: {}", query));
                }
            }
            Command::DataTableRunSelectedHistoryQuery => {
                if let Some(query) = self.get_selected_history_query() {
                    self.status_message = Some(format!("Running query: {}", query));
                }
            }
            Command::DataTableSetTabIndex(idx) => {
                if idx < self.tabs.titles.len() {
                    self.tabs.set_index(idx);
                }
            }
            _ => {}
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn total_pages(&self) -> usize {
        if self.rows.is_empty() {
            return 1;
        }
        (self.rows.len() as f64 / self.page_size as f64).ceil() as usize
    }

    fn get_current_page_rows(&self) -> Vec<Vec<String>> {
        let start_index = self.current_page * self.page_size;
        let end_index = (start_index + self.page_size).min(self.rows.len());
        self.rows[start_index..end_index]
            .iter()
            .map(|row| {
                (0..self.headers.len())
                    .map(|i| Self::get_value_as_string(row, i))
                    .collect()
            })
            .collect()
    }

    pub fn next_row(&mut self) {
        if self.is_empty() {
            return;
        }

        let current_page_rows_len = self.get_current_page_rows().len();
        let i = match self.state.selected() {
            Some(i) if i >= current_page_rows_len.saturating_sub(1) => 0,
            Some(i) => i + 1,
            None => 0,
        };
        self.state.select(Some(i));
        self.vertical_scroll_state = self.vertical_scroll_state.position(i * ITEM_HEIGHT);
    }

    pub fn previous_row(&mut self) {
        if self.rows.is_empty() {
            return;
        }

        let current_page_rows_len = self.get_current_page_rows().len();
        let i = match self.state.selected() {
            Some(0) => current_page_rows_len.saturating_sub(1),
            Some(i) => i - 1,
            None => 0,
        };
        self.state.select(Some(i));
        self.vertical_scroll_state = self.vertical_scroll_state.position(i * ITEM_HEIGHT);
    }

    pub fn next_history_row(&mut self) {
        let i = match self.history_table_state.selected() {
            Some(i) => {
                if i >= self.query_history.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.history_table_state.select(Some(i));
    }

    pub fn previous_history_row(&mut self) {
        let i = match self.history_table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.query_history.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.history_table_state.select(Some(i));
    }

    pub fn next_column(&mut self) {
        self.state.select_next_column();
    }

    pub fn previous_column(&mut self) {
        self.state.select_previous_column();
    }

    pub fn scroll_right(&mut self) {
        if self.horizontal_scroll < self.column_widths.len().saturating_sub(1) {
            self.horizontal_scroll = self.horizontal_scroll.saturating_add(1);
            self.horizontal_scroll_state = self
                .horizontal_scroll_state
                .position(self.horizontal_scroll);
        }
    }

    pub fn scroll_left(&mut self) {
        if self.horizontal_scroll > 0 {
            self.horizontal_scroll = self.horizontal_scroll.saturating_sub(1);
            self.horizontal_scroll_state = self
                .horizontal_scroll_state
                .position(self.horizontal_scroll);
        }
    }

    pub fn next_page(&mut self) {
        if self.current_page < self.total_pages().saturating_sub(1) {
            self.current_page += 1;
            self.state.select(Some(0));
            self.vertical_scroll_state = ScrollbarState::new(
                (self.get_current_page_rows().len().saturating_sub(1)) * ITEM_HEIGHT,
            );
            self.vertical_scroll_state = self.vertical_scroll_state.position(0);
        }
    }

    pub fn previous_page(&mut self) {
        if self.current_page > 0 {
            self.current_page = self.current_page.saturating_sub(1);
            self.state.select(Some(0));
            self.vertical_scroll_state = ScrollbarState::new(
                (self.get_current_page_rows().len().saturating_sub(1)) * ITEM_HEIGHT,
            );
            self.vertical_scroll_state = self.vertical_scroll_state.position(0);
        }
    }

    pub fn next_color(&mut self) {
        self.color_index = (self.color_index + 1) % PALETTES.len();
    }

    pub fn previous_color(&mut self) {
        let count = PALETTES.len();
        self.color_index = (self.color_index + count - 1) % count;
    }

    pub fn set_colors(&mut self) {
        self.colors = TableColors::new(&PALETTES[self.color_index]);
    }

    pub fn jump_to_absolute_row(&mut self, absolute_row: usize) {
        if self.rows.is_empty() {
            return;
        }

        let total_rows = self.rows.len();
        let target_absolute_row = absolute_row.min(total_rows.saturating_sub(1));

        let target_page = target_absolute_row / self.page_size;
        self.current_page = target_page; // Update current page

        let row_on_page = target_absolute_row % self.page_size;
        self.state.select(Some(row_on_page)); // Select row on the *new* page

        // Recalculate vertical scroll state content length for the new page
        self.vertical_scroll_state = ScrollbarState::new(
            (self.get_current_page_rows().len().saturating_sub(1)) * ITEM_HEIGHT,
        );
        self.vertical_scroll_state = self
            .vertical_scroll_state
            .position(row_on_page * ITEM_HEIGHT);
    }

    #[allow(dead_code)]
    pub fn jump_to_column(&mut self, col: usize) {
        if col < self.headers.len() {
            self.horizontal_scroll = col;
            self.horizontal_scroll_state = self.horizontal_scroll_state.position(col);
        }
    }

    #[allow(dead_code)]
    pub fn search_in_table(&mut self, query: &str) -> Option<(usize, usize)> {
        for (row_idx, row) in self.rows.iter().enumerate() {
            for (col_idx, _col) in row.columns().iter().enumerate() {
                let cell_value = Self::get_value_as_string(row, col_idx);
                if cell_value.to_lowercase().contains(&query.to_lowercase()) {
                    let page_row_idx = row_idx % self.page_size;
                    let target_page = row_idx / self.page_size;

                    self.current_page = target_page; // Set current page
                    self.state.select(Some(page_row_idx)); // Select row on the target page

                    // Update vertical scroll state for the *new* page and its position
                    self.vertical_scroll_state = ScrollbarState::new(
                        (self.get_current_page_rows().len().saturating_sub(1)) * ITEM_HEIGHT,
                    );
                    self.vertical_scroll_state = self
                        .vertical_scroll_state
                        .position(page_row_idx * ITEM_HEIGHT);

                    self.horizontal_scroll = col_idx; // Scroll to the found column
                    self.horizontal_scroll_state = self.horizontal_scroll_state.position(col_idx);
                    return Some((page_row_idx, col_idx));
                }
            }
        }
        None
    }

    pub fn copy_selected_cell(&self) -> Option<String> {
        let content = match (self.state.selected(), self.state.selected_column()) {
            (Some(row_idx_on_page), Some(col_idx)) => {
                let absolute_row_idx = self.current_page * self.page_size + row_idx_on_page;
                let adjusted_col = col_idx.saturating_sub(1) + self.horizontal_scroll;
                let row = self.rows.get(absolute_row_idx)?;

                if col_idx == 0 {
                    (absolute_row_idx + 1).to_string()
                } else if adjusted_col < row.columns().len() {
                    Self::get_value_as_string(row, adjusted_col)
                } else {
                    return None;
                }
            }
            _ => return None,
        };

        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard.set_text(&content);
        }

        Some(content)
    }

    pub fn copy_selected_row(&self) -> Option<String> {
        let selected_row_index_on_page = self.state.selected()?;
        let absolute_selected_row_index =
            self.current_page * self.page_size + selected_row_index_on_page;

        let headers = &self.headers;
        let row_data = self.rows.get(absolute_selected_row_index)?;

        let mut row_as_json_object: HashMap<String, Value> = HashMap::new();
        for (i, header) in headers.iter().enumerate() {
            let cell_value = Self::get_value_as_string(row_data, i);
            let json_value = if cell_value.eq_ignore_ascii_case("null")
                || cell_value.eq_ignore_ascii_case("[null]")
            {
                Value::Null
            } else {
                Value::String(cell_value)
            };
            row_as_json_object.insert(header.clone(), json_value);
        }

        let json_string = serde_json::to_string_pretty(&row_as_json_object)
            .map_err(|e| eprintln!("Error: Failed to serialize row data to JSON: {}", e))
            .ok()?;

        if let Ok(mut clipboard) = Clipboard::new() {
            if let Err(e) = clipboard.set_text(&json_string) {
                eprintln!("Warning: Could not set clipboard text: {}", e);
            }
        } else {
            eprintln!("Warning: Could not access clipboard.");
        }

        Some(json_string)
    }

    pub fn copy_selected_query_to_editor(&self) -> Option<String> {
        if let Some(selected) = self.history_table_state.selected() {
            let query = self
                .query_history
                .iter()
                .rev()
                .nth(selected)
                .unwrap()
                .query
                .clone();
            if let Ok(mut clipboard) = Clipboard::new() {
                let _ = clipboard.set_text(query.clone());
            }
            Some(query)
        } else {
            None
        }
    }

    pub fn get_selected_history_query(&self) -> Option<String> {
        if let Some(selected) = self.history_table_state.selected() {
            let query = self
                .query_history
                .iter()
                .rev()
                .nth(selected)
                .unwrap()
                .query
                .clone();
            Some(query)
        } else {
            None
        }
    }

    pub fn adjust_column_width(&mut self, delta: i16) {
        if let Some(col) = self.state.selected_column() {
            self.column_widths[col] = (self.column_widths[col] as i16 + delta)
                .max(self.min_column_widths[col] as i16)
                as u16;
        }
    }

    pub fn build_status_paragraph(&self, title: &'a str, style: &DefaultStyle) -> Paragraph<'a> {
        let title_block = Block::default()
            .borders(Borders::ALL)
            .border_style(style.border_style(Focus::Table))
            .style(style.block_style());

        Paragraph::new(Text::from(title))
            .block(title_block)
            .alignment(Alignment::Center)
    }

    fn create_padded_cell_text(content: &str) -> Text<'_> {
        Text::from(Line::raw(content))
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, current_focus: &Focus) {
        // Optimization: Create DefaultStyle once for this `draw` call
        let app_style = DefaultStyle {
            focus: current_focus.clone(),
        };
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(area);

        let tab_area = main_layout[0];
        let content_area = main_layout[1];
        let query_info_area = main_layout[2];

        let base_style = Style::default().bg(COLOR_BLOCK_BG);
        let total_rows_str = format!("Total Rows: {}", self.rows.len());
        let query_done_str = format!("Query Complete: {} ms", self.elapsed.as_millis());
        let pagination_info_str = format!("Page: {}/{}", self.current_page + 1, self.total_pages());

        let tab_lines = [total_rows_str, query_done_str, pagination_info_str]
            .iter()
            .map(|text| Line::from(Span::styled(text.clone(), base_style)))
            .collect::<Vec<_>>();

        let query_info_tabs = Tabs::new(tab_lines)
            .select(0)
            .highlight_style(base_style)
            .divider(symbols::line::VERTICAL)
            .style(app_style.block_style());
        frame.render_widget(query_info_tabs, query_info_area);

        let tabs_widget = self
            .tabs
            .widget()
            .block(Block::default().border_style(app_style.border_style(Focus::Table)));
        frame.render_widget(tabs_widget, tab_area);

        match self.tabs.index {
            0 => {
                self.set_colors();

                match self.loading_state {
                    LoadingState::Idle => {
                        if self.is_empty() {
                            let message = "No data output. Execute a query to get output";
                            let status_widget = self.build_status_paragraph(message, &app_style);
                            frame.render_widget(status_widget, content_area);
                        } else {
                            self.render_table(frame, content_area, current_focus);
                            self.render_scrollbar(frame, content_area);
                        }
                    }
                    LoadingState::Loading => {
                        let loading_widget =
                            self.build_status_paragraph("Loading data...", &app_style);
                        frame.render_widget(loading_widget, content_area);
                    }
                    LoadingState::Error(ref err_msg) => {
                        let error_message = format!("Error loading data: {}", err_msg);
                        let error_widget = self.build_status_paragraph(&error_message, &app_style);
                        frame.render_widget(error_widget, content_area);
                    }
                }
            }
            1 => {
                let messages_block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(app_style.border_style(Focus::Table))
                    .style(app_style.block_style());
                let message = self.status_message.clone().unwrap_or("".to_string());
                let messages_paragraph = Paragraph::new(message).block(messages_block);
                frame.render_widget(messages_paragraph, content_area);
            }
            2 => {
                self.render_history_table(frame, content_area, current_focus);
            }
            _ => {}
        }
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect, current_focus: &Focus) {
        let table_widget_style = DefaultStyle {
            focus: current_focus.clone(),
        };

        let colors = &self.colors;
        let horizontal_scroll = self.horizontal_scroll;
        let page_size = self.page_size;
        let current_page = self.current_page;
        let item_height = ITEM_HEIGHT;
        let data_column_widths = &self.column_widths;
        let data_headers = &self.headers;

        let owned_current_page_rows: Vec<Vec<String>> = self.get_current_page_rows();

        let header_style = Style::default().fg(colors.header_fg).bg(colors.header_bg);
        let selected_row_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(colors.selected_row_style_fg);
        let selected_col_style = Style::default().fg(colors.selected_column_style_fg);
        let selected_cell_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(colors.selected_cell_style_fg);

        let numbering_col_width = 4;
        let mut visible_columns = 0;
        let mut total_width_of_visible_data_columns = 0;
        let available_width = area.width.saturating_sub(1);

        for &width in data_column_widths.iter().skip(horizontal_scroll) {
            if numbering_col_width + total_width_of_visible_data_columns + width > available_width {
                break;
            }
            total_width_of_visible_data_columns += width;
            visible_columns += 1;
        }

        let mut adjusted_widths = Vec::with_capacity(visible_columns + 1);
        adjusted_widths.push(Constraint::Length(numbering_col_width));

        let mut remaining_width_for_data_cols = available_width.saturating_sub(numbering_col_width);

        for &width in data_column_widths
            .iter()
            .skip(horizontal_scroll)
            .take(visible_columns)
        {
            if remaining_width_for_data_cols >= width {
                adjusted_widths.push(Constraint::Length(width));
                remaining_width_for_data_cols -= width;
            } else {
                adjusted_widths.push(Constraint::Length(remaining_width_for_data_cols));
                break;
            }
        }

        let visible_headers: Vec<&str> = data_headers
            .iter()
            .skip(horizontal_scroll)
            .take(visible_columns)
            .map(|s| s.as_str())
            .collect();

        let header = std::iter::once(Cell::from("#"))
            .chain(visible_headers.into_iter().map(Cell::from))
            .collect::<Row>()
            .style(header_style)
            .height(1);

        let rows = owned_current_page_rows.iter().enumerate().map(|(i, row)| {
            let absolute_row_number = current_page * page_size + i + 1;
            let number_cell = Cell::from(Text::from(format!("{}", absolute_row_number)));

            let data_cells = row
                .iter()
                .skip(horizontal_scroll)
                .take(visible_columns)
                .map(|text| Cell::from(Self::create_padded_cell_text(text.as_str())));

            Row::new(std::iter::once(number_cell).chain(data_cells))
                .style(Style::new().fg(colors.row_fg))
                .height(item_height as u16)
        });

        let bar = " █ ";
        let t = Table::new(rows, adjusted_widths)
            .header(header)
            .row_highlight_style(selected_row_style)
            .column_highlight_style(selected_col_style)
            .cell_highlight_style(selected_cell_style)
            .highlight_symbol(vec!["".into(), bar.into(), "".into()])
            .bg(colors.buffer_bg)
            .highlight_spacing(HighlightSpacing::Always)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(table_widget_style.border_style(Focus::Table))
                    .style(table_widget_style.block_style()),
            );

        frame.render_stateful_widget(t, area, &mut self.state);
    }

    fn render_history_table(&mut self, frame: &mut Frame, area: Rect, current_focus: &Focus) {
        let history_widget_style = DefaultStyle {
            focus: current_focus.clone(),
        };

        let header_style = Style::default()
            .fg(self.colors.header_fg)
            .bg(self.colors.header_bg);
        let selected_row_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(self.colors.selected_row_style_fg);

        let header = ["Query", "Timestamp", "Status", "Rows", "Time (ms)"]
            .iter()
            .map(|h| Cell::from(*h))
            .collect::<Row>()
            .style(header_style)
            .height(1);

        let rows = self.query_history.iter().rev().map(|entry| {
            let query = entry.query.clone();
            let timestamp = entry.timestamp.to_string();
            let status = if entry.success { "OK" } else { "Error" };
            let rows_affected = entry.rows_affected.to_string();
            let execution_time = entry.execution_time.as_millis().to_string();

            Row::new(vec![
                Cell::from(query),
                Cell::from(timestamp),
                Cell::from(status),
                Cell::from(rows_affected),
                Cell::from(execution_time),
            ])
        });

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(50),
                Constraint::Percentage(20),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(history_widget_style.border_style(Focus::Table))
                .style(history_widget_style.block_style()),
        )
        .row_highlight_style(selected_row_style);

        frame.render_stateful_widget(table, area, &mut self.history_table_state);
    }

    fn render_scrollbar(&mut self, frame: &mut Frame, area: Rect) {
        if self.is_empty() {
            return;
        }

        self.vertical_scroll_state = self
            .vertical_scroll_state
            .content_length(self.get_current_page_rows().len().saturating_sub(1) * ITEM_HEIGHT);

        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut self.vertical_scroll_state,
        );

        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::HorizontalBottom)
                .begin_symbol(None)
                .end_symbol(None)
                .thumb_symbol(symbols::line::THICK_HORIZONTAL),
            area.inner(Margin {
                horizontal: 1,
                vertical: 0,
            }),
            &mut self.horizontal_scroll_state,
        );
    }

    pub fn start_loading(&mut self) {
        self.tabs.set_index(0);
        self.loading_state = LoadingState::Loading;
    }

    pub fn finish_loading(
        &mut self,
        headers: Vec<String>,
        rows: Vec<PgRow>,
        elapsed: Duration,
        query_history: Vec<QueryHistoryEntry>,
    ) {
        self.headers = headers;
        self.rows = rows;
        self.elapsed = elapsed;
        self.loading_state = LoadingState::Idle;
        self.status_message = Some(format!("Query complete in {} ms.", elapsed.as_millis()));
        self.query_history = query_history;

        let (column_widths, min_column_widths) =
            Self::calculate_column_widths(&self.headers, &self.rows);
        self.column_widths = column_widths;
        self.min_column_widths = min_column_widths;

        self.state =
            TableState::default().with_selected(if self.is_empty() { None } else { Some(0) });
        self.vertical_scroll_state =
            ScrollbarState::new((self.rows.len().min(100).saturating_sub(1)) * ITEM_HEIGHT);
        self.horizontal_scroll_state =
            ScrollbarState::new(self.column_widths.iter().sum::<u16>().saturating_sub(1) as usize);
        self.current_page = 0;

        if self.is_empty() {
            self.tabs.set_index(1);
        } else {
            self.tabs.set_index(0);
        }
    }

    pub fn set_error_state(&mut self, message: String) {
        self.loading_state = LoadingState::Error(message.clone());
        self.status_message = Some(format!("Error: {}", message));
        self.tabs.set_index(1);
    }
}
