use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style},
    text::Text,
    widgets::{
        Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        StatefulWidget, Widget,
    },
};

pub struct Popup<'a> {
    title: &'a str,
    content: Text<'a>,
    scroll: u16,
    scrollbar_state: &'a mut ScrollbarState,
}

impl<'a> Popup<'a> {
    pub fn new(
        title: &'a str,
        content: Text<'a>,
        scroll: u16,
        scrollbar_state: &'a mut ScrollbarState,
    ) -> Self {
        Self {
            title,
            content,
            scroll,
            scrollbar_state,
        }
    }
}

impl Widget for Popup<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let block = Block::default()
            .title(self.title)
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black).fg(Color::White));

        let popup_area = centered_rect(70, 70, area);

        let paragraph = Paragraph::new(self.content.clone())
            .block(block)
            .scroll((self.scroll, 0));

        // Render the clear widget first to clear the area
        Clear.render(popup_area, buf);
        paragraph.render(popup_area, buf);

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        *self.scrollbar_state = (*self.scrollbar_state)
            .content_length(self.content.height())
            .position(self.scroll as usize);
        scrollbar.render(
            popup_area.inner(Margin::new(1, 1)),
            buf,
            self.scrollbar_state,
        );
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
