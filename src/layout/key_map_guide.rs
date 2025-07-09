use crate::command::CommandCategory;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};

pub fn get_key_map_guide() -> Text<'static> {
    let mut text = Text::default();
    const COLUMN_WIDTH: usize = 38;
    const COLUMN_GAP: usize = 4;

    for category in CommandCategory::help_command_categories() {
        text.push_line(Span::styled(category.to_string(), Style::default().bold()));
        let keymaps = match category {
            CommandCategory::Global => get_global_keymaps(),
            CommandCategory::DataTable => get_data_table_keymaps(),
            CommandCategory::Sidebar => get_sidebar_keymaps(),
            CommandCategory::Editor => get_editor_keymaps(),
        };

        let max_key_len = if category == CommandCategory::Editor {
            keymaps
                .iter()
                .filter(|(key, _)| !key.contains(" Mode"))
                .map(|(key, _)| key.len())
                .max()
                .unwrap_or(0)
        } else {
            keymaps.iter().map(|(key, _)| key.len()).max().unwrap_or(0)
        };
        let key_col_width = max_key_len + 2;

        let mut i = 0;
        while i < keymaps.len() {
            let mut line_spans = Vec::new();

            if category == CommandCategory::Editor && keymaps[i].0.contains(" Mode") {
                text.push_line(Line::from(vec![Span::styled(
                    format!(
                        "  {:<width$}",
                        keymaps[i].0,
                        width = COLUMN_WIDTH * 2 + COLUMN_GAP - 2
                    ),
                    Style::default().fg(Color::White),
                )]));
                i += 1;
                continue;
            }

            let (key_l, desc_l) = keymaps[i];
            let key_span_l = Span::styled(
                format!("  {:<width$}", key_l, width = key_col_width),
                Style::default().fg(Color::Cyan),
            );
            let desc_span_l = Span::raw(format!(
                "{:<width$}",
                desc_l,
                width = COLUMN_WIDTH - key_col_width
            ));
            line_spans.push(key_span_l);
            line_spans.push(desc_span_l);

            line_spans.push(Span::raw(" ".repeat(COLUMN_GAP)));

            if let Some((key_r, desc_r)) = keymaps.get(i + 1) {
                if category == CommandCategory::Editor && key_r.contains(" Mode") {
                    text.push_line(Line::from(line_spans));
                    i += 1;
                    continue;
                }

                let key_span_r = Span::styled(
                    format!("  {:<width$}", key_r, width = key_col_width),
                    Style::default().fg(Color::Cyan),
                );
                let desc_span_r = Span::raw(format!(
                    "{:<width$}",
                    desc_r,
                    width = COLUMN_WIDTH - key_col_width
                ));
                line_spans.push(key_span_r);
                line_spans.push(desc_span_r);
            }

            text.push_line(Line::from(line_spans));
            i += 2;
        }
        text.push_line("");
    }
    text
}

fn get_global_keymaps() -> Vec<(&'static str, &'static str)> {
    vec![
        ("q", "Quit"),
        ("Tab", "Toggle focus"),
        ("F5", "Execute query"),
        ("?", "Show key map"),
    ]
}

fn get_data_table_keymaps() -> Vec<(&'static str, &'static str)> {
    vec![
        ("[", "Previous tab"),
        ("]", "Next tab"),
        ("j / ↓", "Next row"),
        ("k / ↑", "Previous row"),
        ("PageDown / Space", "Next page"),
        ("PageUp", "Previous page"),
        ("g", "Jump to first row"),
        ("G", "Jump to last row"),
        ("l / →", "Next column"),
        ("h / ←", "Previous column"),
        (">", "Scroll right"),
        ("<", "Scroll left"),
        ("w", "Increase column width"),
        ("W", "Decrease column width"),
        ("n", "Next color"),
        ("p", "Previous color"),
        ("y", "Copy selected cell"),
        ("Y", "Copy selected row"),
        ("C", "Copy query to editor"),
        ("R", "Run selected history query"),
        ("1-9", "Set tab index"),
    ]
}

fn get_sidebar_keymaps() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Enter / Space", "Toggle selected"),
        ("←", "Collapse"),
        ("→", "Expand"),
        ("↓", "Down"),
        ("↑", "Up"),
        ("Esc", "Deselect"),
        ("Home", "Select first"),
        ("End", "Select last"),
        ("PageDown", "Scroll down"),
        ("PageUp", "Scroll up"),
    ]
}

fn get_editor_keymaps() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Normal Mode", ""),
        ("  h/j/k/l", "Move cursor"),
        ("  w/b", "Move by word"),
        ("  ^/$", "Move to line start/end"),
        ("  g/G", "Move to top/bottom"),
        ("  i/a", "Enter insert mode"),
        ("  o/O", "Insert line below/above"),
        ("  v/V", "Enter visual mode"),
        ("  d/c/y", "Delete/change/yank (operator)"),
        ("  dd/cc/yy", "Delete/change/yank line"),
        ("  p", "Paste"),
        ("  u", "Undo"),
        ("  Ctrl+r", "Redo"),
        ("Insert Mode", ""),
        ("  Esc/Ctrl+c", "Enter normal mode"),
        ("Visual Mode", ""),
        ("  Esc/v", "Enter normal mode"),
        ("  d/c/y", "Delete/change/yank selection"),
    ]
}
