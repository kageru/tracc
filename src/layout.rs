use tui::layout::*;
use tui::style::{Color, Style};
use tui::widgets::*;
pub fn selectable_list<'a, C: AsRef<str>>(
    title: &'a str,
    content: &'a [C],
    selected: Option<usize>,
) -> SelectableList<'a> {
    SelectableList::default()
        .block(
            Block::default()
                .title(title)
                .borders(Borders::TOP | Borders::RIGHT | Borders::LEFT),
        )
        .items(content)
        .select(selected)
        .highlight_style(Style::default().fg(Color::LightGreen))
        .highlight_symbol(">")
}

pub fn layout(r: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(40),
                Constraint::Percentage(40),
                Constraint::Percentage(20),
            ]
            .as_ref(),
        )
        .split(r)
}
