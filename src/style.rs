use crate::app::Focus;
use ratatui::style::{Modifier, Style};

/// Predefined colors for consistent style
pub mod theme {
    use ratatui::style::Color;

    pub const COLOR_FOCUS: Color = Color::Rgb(137, 220, 235);
    pub const COLOR_UNFOCUSED: Color = Color::Rgb(88, 91, 112);
    pub const COLOR_BLOCK_BG: Color = Color::Rgb(30, 30, 46);
    pub const COLOR_HIGHLIGHT_BG: Color = Color::Rgb(137, 220, 235);
    pub const COLOR_HIGHLIGHT_FG: Color = Color::Black;
    pub const COLOR_BLACK: Color = Color::Black;
    pub const COLOR_WHITE: Color = Color::White;
}

pub trait StyleProvider {
    fn border_style(&self, current: Focus) -> Style;
    fn block_style(&self) -> Style;
    fn highlight_style(&self) -> Style;
}

pub struct DefaultStyle {
    pub focus: Focus,
}

impl StyleProvider for DefaultStyle {
    fn border_style(&self, current: Focus) -> Style {
        if self.focus == current {
            Style::default()
                .fg(theme::COLOR_FOCUS)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::COLOR_UNFOCUSED)
        }
    }

    fn block_style(&self) -> Style {
        Style::default().bg(theme::COLOR_BLOCK_BG)
    }

    fn highlight_style(&self) -> Style {
        Style::default()
            .bg(theme::COLOR_HIGHLIGHT_BG)
            .fg(theme::COLOR_HIGHLIGHT_FG)
            .add_modifier(Modifier::BOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Modifier, Style};

    #[test]
    fn test_border_style_when_focused() {
        let style = DefaultStyle {
            focus: Focus::Sidebar,
        };
        let result = style.border_style(Focus::Sidebar);
        assert_eq!(
            result,
            Style::default()
                .fg(theme::COLOR_FOCUS)
                .add_modifier(Modifier::BOLD)
        )
    }

    #[test]
    fn test_block_style() {
        let style = DefaultStyle {
            focus: Focus::Sidebar,
        };
        let result = style.block_style();
        assert_eq!(result, Style::default().bg(theme::COLOR_BLOCK_BG))
    }

    #[test]
    fn test_highlight_style() {
        let style = DefaultStyle {
            focus: Focus::Sidebar,
        };
        let result = style.highlight_style();
        assert_eq!(
            result,
            Style::default()
                .bg(theme::COLOR_HIGHLIGHT_BG)
                .fg(theme::COLOR_HIGHLIGHT_FG)
                .add_modifier(Modifier::BOLD)
        )
    }
}
