use crate::app::Focus;
use crate::command::Command;
use crate::layout::query_editor::Mode;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use tui_textarea::{CursorMove, Input, Key, Scrolling};

pub trait KeyMapper {
    fn map_key_to_command(
        &mut self,
        key_event: KeyEvent,
        current_focus: &Focus,
        tab_index: usize,
    ) -> Option<Command>;

    fn editor_mode(&self) -> Mode;
    fn map_popup_key(&mut self, key_event: KeyEvent) -> Option<Command>;
}

pub struct DefaultKeyMapper {
    editor_mode: Mode,
    editor_pending_input: Option<Input>,
}

impl DefaultKeyMapper {
    pub fn new() -> Self {
        Self {
            editor_mode: Mode::Normal,
            editor_pending_input: None,
        }
    }

    fn map_query_editor_key(&mut self, input: Input) -> Option<Command> {
        if input.key == Key::Null {
            return Some(Command::NoOp);
        }

        if let Some(pending) = self.editor_pending_input.take() {
            if pending.key == Key::Char('g')
                && !pending.ctrl
                && input.key == Key::Char('g')
                && !input.ctrl
            {
                return Some(Command::EditorMoveCursor(CursorMove::Top));
            }
            if let Key::Char(op @ ('y' | 'd' | 'c')) = pending.key
                && input.key == Key::Char(op) {
                    return match op {
                        'y' => Some(Command::EditorCopySelection),
                        'd' => Some(Command::EditorDeleteLineByEnd),
                        'c' => Some(Command::EditorDeleteLineByEnd),
                        _ => None,
                    };
                }

            let command_from_pending = match pending.key {
                Key::Char(op @ ('y' | 'd' | 'c')) => {
                    Some(Command::EditorSetMode(Mode::Operator(op)))
                }
                _ => None,
            };
            if command_from_pending.is_some() {
                match input.key {
                    Key::Char('h')
                    | Key::Char('j')
                    | Key::Char('k')
                    | Key::Char('l')
                    | Key::Char('w')
                    | Key::Char('e')
                    | Key::Char('b')
                    | Key::Char('^')
                    | Key::Char('$')
                    | Key::Char('g')
                    | Key::Char('G')
                    | Key::PageUp
                    | Key::PageDown => {
                        self.editor_pending_input = Some(input);
                        return command_from_pending;
                    }
                    _ => {}
                }
            }
        }

        match self.editor_mode {
            Mode::Normal => match input.key {
                Key::Char('h') => Some(Command::EditorMoveCursor(CursorMove::Back)),
                Key::Char('j') => Some(Command::EditorMoveCursor(CursorMove::Down)),
                Key::Char('k') => Some(Command::EditorMoveCursor(CursorMove::Up)),
                Key::Char('l') => Some(Command::EditorMoveCursor(CursorMove::Forward)),
                Key::Char('w') => Some(Command::EditorMoveCursor(CursorMove::WordForward)),
                Key::Char('e') => {
                    if input.ctrl {
                        Some(Command::EditorScrollRelative(1, 0))
                    } else {
                        Some(Command::EditorMoveCursor(CursorMove::WordEnd))
                    }
                }
                Key::Char('b') => {
                    if input.ctrl {
                        Some(Command::EditorScroll(Scrolling::PageUp))
                    } else {
                        Some(Command::EditorMoveCursor(CursorMove::WordBack))
                    }
                }
                Key::Char('^') => Some(Command::EditorMoveCursor(CursorMove::Head)),
                Key::Char('$') => Some(Command::EditorMoveCursor(CursorMove::End)),
                Key::Char('D') => Some(Command::EditorDeleteLineByEnd),
                Key::Char('C') => {
                    self.editor_mode = Mode::Insert;
                    Some(Command::EditorDeleteLineByEnd)
                }
                Key::Char('p') => Some(Command::EditorPaste),
                Key::Char('u') if !input.ctrl => Some(Command::EditorUndo),
                Key::Char('r') if input.ctrl => Some(Command::EditorRedo),
                Key::Char('x') => Some(Command::EditorDeleteNextChar),
                Key::Char('i') => {
                    self.editor_mode = Mode::Insert;
                    Some(Command::EditorSetMode(Mode::Insert))
                }
                Key::Char('a') => {
                    self.editor_mode = Mode::Insert;
                    Some(Command::EditorMoveCursor(CursorMove::Forward))
                }
                Key::Char('A') => {
                    self.editor_mode = Mode::Insert;
                    Some(Command::EditorMoveCursor(CursorMove::End))
                }
                Key::Char('o') => {
                    self.editor_mode = Mode::Insert;
                    Some(Command::EditorInputEnter)
                }
                Key::Char('O') => {
                    self.editor_mode = Mode::Insert;
                    Some(Command::EditorInputEnter)
                }
                Key::Char('I') => {
                    self.editor_mode = Mode::Insert;
                    Some(Command::EditorMoveCursor(CursorMove::Head))
                }
                Key::Char('y') if input.ctrl => Some(Command::EditorScrollRelative(-1, 0)),
                Key::Char('d') if input.ctrl => {
                    Some(Command::EditorScroll(Scrolling::HalfPageDown))
                }
                Key::Char('u') if input.ctrl => Some(Command::EditorScroll(Scrolling::HalfPageUp)),
                Key::Char('f') if input.ctrl => Some(Command::EditorScroll(Scrolling::PageDown)),
                Key::Char('v') => {
                    self.editor_mode = Mode::Visual;
                    Some(Command::EditorStartSelection)
                }
                Key::Char('V') => {
                    self.editor_mode = Mode::Visual;
                    Some(Command::EditorStartSelection)
                }
                Key::Char('g') => {
                    self.editor_pending_input = Some(input);
                    Some(Command::NoOp)
                }
                Key::Char('G') => Some(Command::EditorMoveCursor(CursorMove::Bottom)),
                Key::Char(op @ ('y' | 'd' | 'c')) => {
                    self.editor_pending_input = Some(input);
                    self.editor_mode = Mode::Operator(op);
                    Some(Command::EditorSetMode(Mode::Operator(op)))
                }
                _ => Some(Command::NoOp),
            },
            Mode::Insert => match input.key {
                Key::Esc => {
                    self.editor_mode = Mode::Normal;
                    Some(Command::EditorSetMode(Mode::Normal))
                }
                Key::Char('c') if input.ctrl => {
                    self.editor_mode = Mode::Normal;
                    Some(Command::EditorSetMode(Mode::Normal))
                }
                Key::Backspace => Some(Command::EditorInputBackspace),
                Key::Delete => Some(Command::EditorInputDelete),
                Key::Enter => Some(Command::EditorInputEnter),
                Key::Left => Some(Command::EditorMoveCursor(CursorMove::Back)),
                Key::Right => Some(Command::EditorMoveCursor(CursorMove::Forward)),
                Key::Up => Some(Command::EditorMoveCursor(CursorMove::Up)),
                Key::Down => Some(Command::EditorMoveCursor(CursorMove::Down)),
                Key::Home => Some(Command::EditorMoveCursor(CursorMove::Head)),
                Key::End => Some(Command::EditorMoveCursor(CursorMove::End)),
                Key::PageUp => Some(Command::EditorScroll(Scrolling::PageUp)),
                Key::PageDown => Some(Command::EditorScroll(Scrolling::PageDown)),
                Key::Char(c) => Some(Command::EditorInputChar(c)),
                _ => Some(Command::NoOp),
            },
            Mode::Visual => match input.key {
                Key::Char('h') => Some(Command::EditorMoveCursor(CursorMove::Back)),
                Key::Char('j') => Some(Command::EditorMoveCursor(CursorMove::Down)),
                Key::Char('k') => Some(Command::EditorMoveCursor(CursorMove::Up)),
                Key::Char('l') => Some(Command::EditorMoveCursor(CursorMove::Forward)),
                Key::Char('w') => Some(Command::EditorMoveCursor(CursorMove::WordForward)),
                Key::Char('e') => Some(Command::EditorMoveCursor(CursorMove::WordEnd)),
                Key::Char('b') => Some(Command::EditorMoveCursor(CursorMove::WordBack)),
                Key::Char('^') => Some(Command::EditorMoveCursor(CursorMove::Head)),
                Key::Char('$') => Some(Command::EditorMoveCursor(CursorMove::End)),
                Key::Char('g') => {
                    self.editor_pending_input = Some(input);
                    Some(Command::NoOp)
                }
                Key::Char('G') => Some(Command::EditorMoveCursor(CursorMove::Bottom)),
                Key::Char('y') => {
                    self.editor_mode = Mode::Normal;
                    Some(Command::EditorCopySelection)
                }
                Key::Char('d') => {
                    self.editor_mode = Mode::Normal;
                    Some(Command::EditorCutSelection)
                }
                Key::Char('c') => {
                    self.editor_mode = Mode::Insert;
                    Some(Command::EditorCutSelection)
                }
                Key::Esc | Key::Char('v') => {
                    self.editor_mode = Mode::Normal;
                    Some(Command::EditorCancelSelection)
                }
                _ => Some(Command::NoOp),
            },
            Mode::Operator(op) => {
                let motion_command = match input.key {
                    Key::Char('h') => Some(Command::EditorMoveCursor(CursorMove::Back)),
                    Key::Char('j') => Some(Command::EditorMoveCursor(CursorMove::Down)),
                    Key::Char('k') => Some(Command::EditorMoveCursor(CursorMove::Up)),
                    Key::Char('l') => Some(Command::EditorMoveCursor(CursorMove::Forward)),
                    Key::Char('w') => Some(Command::EditorMoveCursor(CursorMove::WordForward)),
                    Key::Char('e') => Some(Command::EditorMoveCursor(CursorMove::WordEnd)),
                    Key::Char('b') => Some(Command::EditorMoveCursor(CursorMove::WordBack)),
                    Key::Char('^') => Some(Command::EditorMoveCursor(CursorMove::Head)),
                    Key::Char('$') => Some(Command::EditorMoveCursor(CursorMove::End)),
                    Key::Char('g') => {
                        self.editor_pending_input = Some(input);
                        return Some(Command::NoOp);
                    }
                    Key::Char('G') => Some(Command::EditorMoveCursor(CursorMove::Bottom)),
                    Key::Char(c) if c == op => {
                        self.editor_mode = Mode::Normal;
                        return Some(Command::EditorPerformPendingOperator);
                    }
                    _ => None,
                };

                if motion_command.is_some() {
                    self.editor_mode = Mode::Normal;
                    Some(Command::EditorPerformPendingOperator)
                } else {
                    self.editor_mode = Mode::Normal;
                    Some(Command::NoOp)
                }
            }
        }
    }

    fn map_data_table_key(&self, key: KeyCode, tab_index: usize) -> Option<Command> {
        use KeyCode::*;
        match key {
            Char('[') => Some(Command::DataTablePreviousTab),
            Char(']') => Some(Command::DataTableNextTab),

            Char('j') | Down => {
                if tab_index == 2 {
                    Some(Command::DataTableNextHistoryRow)
                } else {
                    Some(Command::DataTableNextRow)
                }
            }
            Char('k') | Up => {
                if tab_index == 2 {
                    Some(Command::DataTablePreviousHistoryRow)
                } else {
                    Some(Command::DataTablePreviousRow)
                }
            }
            PageDown => Some(Command::DataTableNextPage),
            PageUp => Some(Command::DataTablePreviousPage),
            Char(' ') => Some(Command::DataTableNextPage),
            Char('g') => Some(Command::DataTableJumpToFirstRow),
            Char('G') => Some(Command::DataTableJumpToLastRow),

            Char('>') => Some(Command::DataTableScrollRight),
            Char('<') => Some(Command::DataTableScrollLeft),
            Char('l') | Right => Some(Command::DataTableNextColumn),
            Char('h') | Left => Some(Command::DataTablePreviousColumn),
            Char('w') => Some(Command::DataTableAdjustColumnWidthIncrease),
            Char('W') => Some(Command::DataTableAdjustColumnWidthDecrease),

            Char('n') => Some(Command::DataTableNextColor),
            Char('p') => Some(Command::DataTablePreviousColor),

            Char('y') => Some(Command::DataTableCopySelectedCell),
            Char('Y') => Some(Command::DataTableCopySelectedRow),
            Char('C') => Some(Command::DataTableCopyQueryToEditor),
            Char('R') => Some(Command::DataTableRunSelectedHistoryQuery),

            Char(c) if c.is_ascii_digit() => {
                if let Some(digit) = c.to_digit(10) {
                    if digit > 0 {
                        Some(Command::DataTableSetTabIndex((digit as usize) - 1))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn map_sidebar_key(&self, key: KeyCode) -> Option<Command> {
        use KeyCode::*;
        match key {
            Char('\n') | Char(' ') => Some(Command::SidebarToggleSelected),
            Left => Some(Command::SidebarKeyLeft),
            Right => Some(Command::SidebarKeyRight),
            Down => Some(Command::SidebarKeyDown),
            Up => Some(Command::SidebarKeyUp),
            Esc => Some(Command::SidebarDeselect),
            Home => Some(Command::SidebarSelectFirst),
            End => Some(Command::SidebarSelectLast),
            PageDown => Some(Command::SidebarScrollDown(3)),
            PageUp => Some(Command::SidebarScrollUp(3)),
            _ => None,
        }
    }
}

impl KeyMapper for DefaultKeyMapper {
    fn map_key_to_command(
        &mut self,
        key_event: KeyEvent,
        current_focus: &Focus,
        tab_index: usize,
    ) -> Option<Command> {
        if key_event.kind != KeyEventKind::Press {
            return None;
        }

        let command = match key_event.code {
            KeyCode::Char('q') => Some(Command::Quit),
            KeyCode::Char('?') => Some(Command::ShowKeyMap),
            KeyCode::Tab => Some(Command::ToggleFocus),
            KeyCode::F(5) => Some(Command::ExecuteQuery),
            _ => None,
        };

        if command.is_some() {
            return command;
        }

        match current_focus {
            Focus::Editor => {
                let input = Input::from(key_event);
                self.map_query_editor_key(input)
            }
            Focus::Table => self.map_data_table_key(key_event.code, tab_index),
            Focus::Sidebar => self.map_sidebar_key(key_event.code),
        }
    }

    fn map_popup_key(&mut self, key_event: KeyEvent) -> Option<Command> {
        if key_event.kind != KeyEventKind::Press {
            return None;
        }

        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('?') => Some(Command::ClosePopup),
            KeyCode::Char('k') | KeyCode::Up => Some(Command::KeyMapScrollUp),
            KeyCode::Char('j') | KeyCode::Down => Some(Command::KeyMapScrollDown),
            _ => None,
        }
    }

    fn editor_mode(&self) -> Mode {
        self.editor_mode
    }
}
