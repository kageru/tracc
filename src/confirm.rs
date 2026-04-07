use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ConfirmChoice {
    Yes,
    No,
}

impl ConfirmChoice {
    pub fn toggle(self) -> Self {
        match self {
            Self::Yes => Self::No,
            Self::No => Self::Yes,
        }
    }
}

pub fn area(frame_area: Rect) -> Rect {
    let width = frame_area.width.saturating_sub(10).clamp(1, 50);
    let height = 7;
    let x = frame_area.x + (frame_area.width.saturating_sub(width)) / 2;
    let y = frame_area.y + (frame_area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}

pub struct ConfirmDialog<'a> {
    message: &'a str,
    selected: ConfirmChoice,
}

impl<'a> ConfirmDialog<'a> {
    pub fn new(message: &'a str, selected: ConfirmChoice) -> Self {
        Self { message, selected }
    }

    pub fn render(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        let block = Block::default().title(" confirm ").borders(Borders::ALL);
        let inner = Rect::new(
            area.x + 1,
            area.y + 1,
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        );
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
            .split(inner);

        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(self.message)
                .wrap(Wrap { trim: true })
                .alignment(Alignment::Center),
            parts[0],
        );
        frame.render_widget(
            Paragraph::new(buttons(self.selected)).alignment(Alignment::Center),
            parts[1],
        );
    }
}

fn buttons(selected: ConfirmChoice) -> Line<'static> {
    let yes = button(" yes ", selected == ConfirmChoice::Yes);
    let no = button(" no ", selected == ConfirmChoice::No);
    Line::from(vec![yes, Span::raw(" "), no])
}

fn button(label: &'static str, selected: bool) -> Span<'static> {
    if selected {
        Span::styled(
            label,
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(label, Style::default().fg(Color::Gray))
    }
}
