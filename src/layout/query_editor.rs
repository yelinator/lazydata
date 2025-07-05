use crate::app::Focus;
use crate::command::Command;
use crate::style::{DefaultStyle, StyleProvider};
use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders};
use std::fmt;
use tui_textarea::{Input, TextArea};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    Operator(char),
}

impl Mode {
    fn block<'a>(&self, current_focus: &Focus) -> Block<'a> {
        let style = DefaultStyle {
            focus: current_focus.clone(),
        };
        let help = match self {
            Self::Normal => "type i to enter insert mode",
            Self::Insert => "type Esc to back to normal mode",
            Self::Visual => "type y to yank, type d to delete, type Esc to back to normal mode",
            Self::Operator(_) => "move cursor to apply operator",
        };
        let title = format!("{} MODE ({})", self, help);
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(style.border_style(Focus::Editor))
            .style(style.block_style())
    }

    fn cursor_style(&self) -> Style {
        let color = match self {
            Self::Normal => Color::Reset,
            Self::Insert => Color::LightBlue,
            Self::Visual => Color::LightYellow,
            Self::Operator(_) => Color::LightGreen,
        };
        Style::default().fg(color).add_modifier(Modifier::REVERSED)
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            Self::Normal => write!(f, "NORMAL"),
            Self::Insert => write!(f, "INSERT"),
            Self::Visual => write!(f, "VISUAL"),
            Self::Operator(c) => write!(f, "OPERATOR({})", c),
        }
    }
}

pub struct QueryEditor {
    pub mode: Mode,
    pub textarea: TextArea<'static>,
}

impl QueryEditor {
    pub fn new() -> Self {
        let mut textarea = TextArea::default();
        textarea.set_block(Block::default().borders(Borders::ALL).title("SQL Editor"));
        Self {
            mode: Mode::Normal,
            textarea,
        }
    }

    pub fn handle_command(&mut self, command: Command, key_event: KeyEvent) {
        match command {
            Command::EditorInputChar(_) => {
                self.input(Input::from(key_event));
            }
            Command::EditorInputBackspace => {
                self.input(Input::from(key_event));
            }
            Command::EditorInputDelete => {
                self.input(Input::from(key_event));
            }
            Command::EditorInputEnter => {
                self.input(Input::from(key_event));
            }
            Command::EditorMoveCursor(move_action) => {
                self.textarea.move_cursor(move_action);
            }
            Command::EditorDeleteLineByEnd => {
                self.textarea.delete_line_by_end();
            }
            Command::EditorCancelSelection => {
                self.textarea.cancel_selection();
            }
            Command::EditorPaste => {
                self.textarea.paste();
            }
            Command::EditorUndo => {
                self.textarea.undo();
            }
            Command::EditorRedo => {
                self.textarea.redo();
            }
            Command::EditorDeleteNextChar => {
                self.textarea.delete_next_char();
            }
            Command::EditorSetMode(mode) => {
                self.mode = mode;
            }
            Command::EditorScrollRelative(rows, cols) => {
                self.textarea.scroll((rows, cols));
            }
            Command::EditorScroll(scrolling_action) => {
                self.textarea.scroll(scrolling_action);
            }
            Command::EditorStartSelection => {
                self.textarea.start_selection();
            }
            Command::EditorCopySelection => {
                self.textarea.copy();
            }
            Command::EditorCutSelection => {
                self.textarea.cut();
            }
            Command::EditorPerformPendingOperator => {
                self.textarea.cancel_selection();
                self.mode = Mode::Normal;
            }
            _ => {}
        }
    }

    pub fn input(&mut self, input: Input) {
        self.textarea.input(input);
    }

    pub fn textarea_content(&self) -> String {
        self.textarea.lines().join("\n")
    }

    pub fn set_textarea_content(&mut self, content: String) {
        self.textarea = TextArea::from(content.lines().map(String::from).collect::<Vec<String>>());
        self.textarea.set_block(self.mode.block(&Focus::Editor));
        self.textarea.set_cursor_style(self.mode.cursor_style());
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, current_focus: Focus) {
        self.textarea.set_block(self.mode.block(&current_focus));
        self.textarea.set_cursor_style(self.mode.cursor_style());
        frame.render_widget(&self.textarea, area);
    }
}
