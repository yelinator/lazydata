use crate::{
    app::Focus,
    command::Command,
    style::{DefaultStyle, StyleProvider},
};
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Scrollbar, ScrollbarOrientation};
use ratatui::{Frame, widgets::Borders};
use tui_tree_widget::{Tree, TreeItem, TreeState};
#[must_use]
pub struct SideBar {
    pub state: TreeState<String>,
    pub items: Vec<TreeItem<'static, String>>,
    pub focus: Focus,
}

impl SideBar {
    pub fn new(items: Vec<TreeItem<'static, String>>, focus: Focus) -> Self {
        Self {
            state: TreeState::default(),
            items,
            focus,
        }
    }

    pub fn handle_command(&mut self, command: Command) -> Option<String> {
        match command {
            Command::SidebarToggleSelected => {
                self.state.toggle_selected();
                return self.state.selected().first().cloned();
            }
            Command::SidebarKeyLeft => {
                self.state.key_left();
            }
            Command::SidebarKeyRight => {
                self.state.key_right();
                return self.state.selected().first().cloned();
            }
            Command::SidebarKeyDown => {
                self.state.key_down();
            }
            Command::SidebarKeyUp => {
                self.state.key_up();
            }
            Command::SidebarDeselect => {
                self.state.select(Vec::new());
            }
            Command::SidebarSelectFirst => {
                self.state.select_first();
            }
            Command::SidebarSelectLast => {
                self.state.select_last();
            }
            Command::SidebarScrollDown(amount) => {
                self.state.scroll_down(amount as usize);
            }
            Command::SidebarScrollUp(amount) => {
                self.state.scroll_up(amount as usize);
            }
            _ => {}
        }
        None
    }

    pub fn update_focus(&mut self, new_focus: Focus) {
        self.focus = new_focus;
    }

    pub fn update_items(&mut self, new_items: Vec<TreeItem<'static, String>>) {
        self.items = new_items;
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let style = DefaultStyle {
            focus: self.focus.clone(),
        };
        let widget = Tree::new(&self.items)
            .expect("tree item IDs must be unique")
            .block(
                Block::bordered()
                    .title("Databases")
                    .borders(Borders::ALL)
                    .border_style(style.border_style(Focus::Sidebar))
                    .style(style.block_style()),
            )
            .experimental_scrollbar(Some(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .track_symbol(None)
                    .end_symbol(None),
            ))
            .highlight_style(style.highlight_style());

        frame.render_stateful_widget(widget, area, &mut self.state);
    }
}
