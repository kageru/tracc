use ratatui::layout::*;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::*;
pub fn selectable_list<'a, C: AsRef<str>>(title: &'a str, content: &'a [C]) -> List<'a> {
    let items = content
        .iter()
        .map(|item| ListItem::new(item.as_ref()))
        .collect::<Vec<_>>();

    List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::TOP | Borders::RIGHT | Borders::LEFT),
        )
        .highlight_style(Style::default().fg(Color::LightGreen))
        .highlight_symbol(Line::from(">"))
}

pub fn layout(r: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
        .split(r)
        .to_vec()
}
