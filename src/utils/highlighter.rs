use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use syntect::{
    easy::HighlightLines, highlighting::Theme, parsing::SyntaxSet, util::LinesWithEndings,
};

#[allow(dead_code)]
pub fn highlight_sql(
    text: &str,
    ps: &SyntaxSet,
    theme: &Theme,
    cursor_row: usize,
    cursor_col: usize,
    cursor_style: Style,
) -> Vec<Line<'static>> {
    let syntax = ps.find_syntax_by_extension("sql").unwrap();
    let mut h = HighlightLines::new(syntax, theme);

    LinesWithEndings::from(text)
        .enumerate()
        .map(|(row_idx, line)| {
            let ranges = h.highlight_line(line, ps).unwrap_or_default();
            let mut styled_spans: Vec<Span> = Vec::new();
            let mut current_col_offset = 0;

            for (style, content) in ranges {
                let foreground_color =
                    Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                let base_style = Style::default().fg(foreground_color);

                let cursor_in_segment = row_idx == cursor_row
                    && cursor_col >= current_col_offset
                    && cursor_col < current_col_offset + content.len();

                if cursor_in_segment {
                    let cursor_relative_col = cursor_col - current_col_offset;

                    if cursor_relative_col > 0 {
                        styled_spans.push(Span::styled(
                            content[..cursor_relative_col].to_string(),
                            base_style,
                        ));
                    }

                    styled_spans.push(Span::styled(
                        content[cursor_relative_col..=cursor_relative_col].to_string(),
                        cursor_style,
                    ));

                    if cursor_relative_col + 1 < content.len() {
                        styled_spans.push(Span::styled(
                            content[cursor_relative_col + 1..].to_string(),
                            base_style,
                        ));
                    }
                } else {
                    styled_spans.push(Span::styled(content.to_string(), base_style));
                }
                current_col_offset += content.len();
            }
            Line::from(styled_spans)
        })
        .collect()
}
