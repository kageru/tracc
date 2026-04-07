use super::input::InputState;
use super::Tracc;
use crate::confirm::{self, ConfirmDialog};
use crate::layout;
use crate::timesheet::TimeSheet;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, ListState, Padding, Paragraph, Wrap};

impl Tracc {
    pub(crate) fn refresh(&mut self) -> Result<(), std::io::Error> {
        let today = TimeSheet::current_date();
        let headline = self.times_headline(today);
        let summary_content = format!(
            "Sum: {}\n{}{}\n\n{}",
            self.times.sum_as_str(),
            self.times.pause_time(),
            if self.times.has_time_overflow() {
                "\ntracking exceeds day"
            } else {
                ""
            },
            self.times.time_by_tasks()
        );
        let summary = Paragraph::new(summary_content)
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .padding(Padding::new(1, 1, 0, 0)),
            );
        let times = self.times.printable();
        let timelist = layout::selectable_list(headline, &times);
        let mut state = ListState::default();
        state.select(self.times.selected_index());
        let frame_size = self.terminal.size()?;
        let frame_area = Rect::new(0, 0, frame_size.width, frame_size.height);
        let chunks = layout::layout(frame_area);
        self.frame_area = frame_area;
        self.list_area = chunks[0];
        let edit = match &self.input_state {
            InputState::Editing(edit) => Some((
                edit.text.clone(),
                edit.cursor,
                edit.popup_title(),
                edit.popup_area(frame_area, chunks[0]),
            )),
            _ => None,
        };
        let confirm = match &self.input_state {
            InputState::Confirm(confirm) => Some((confirm.message.clone(), confirm.selected)),
            _ => None,
        };

        self.terminal.draw(|frame| {
            frame.render_stateful_widget(timelist, chunks[0], &mut state);
            frame.render_widget(summary, chunks[1]);

            if let Some((text, cursor, title, popup_area)) = edit.as_ref() {
                let input = Paragraph::new(text.as_str())
                    .block(Block::default().title(*title).borders(Borders::ALL));
                frame.render_widget(Clear, *popup_area);
                frame.render_widget(input, *popup_area);

                let cursor_x = (popup_area.x + 1 + *cursor as u16)
                    .min(popup_area.x + popup_area.width.saturating_sub(2));
                frame.set_cursor_position((cursor_x, popup_area.y + 1));
            }

            if let Some((message, selected)) = confirm.as_ref() {
                let popup_area = confirm::area(frame.area());
                ConfirmDialog::new(message.as_str(), *selected).render(frame, popup_area);
            }
        })?;
        Ok(())
    }

    fn times_headline(&self, today: time::Date) -> Line<'static> {
        let mut spans = vec![Span::raw("< ")];
        let weekday = format!("{:?}", self.times.date.weekday());
        let label = format!("{} {}", self.times.date_label(), weekday);
        if self.times.date == today || !self.sheet_locked {
            spans.push(Span::styled(
                label,
                Style::default().add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                label,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
        }
        if self.times.date < today {
            spans.push(Span::raw(" >"));
        }
        Line::from(spans)
    }
}

pub(crate) fn edit_area(frame_area: Rect, list_area: Rect, selected: usize) -> Rect {
    let height = 3;
    let width = list_area.width.saturating_sub(4).max(20);
    let x = list_area.x + 2;
    let below_row = list_area.y + 1 + selected as u16 + 1;
    let max_y = frame_area.y + frame_area.height.saturating_sub(height);
    let y = below_row.min(max_y);

    Rect::new(x, y, width.min(frame_area.width.saturating_sub(2)), height)
}

pub(crate) fn centered_area(frame_area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(frame_area.width.saturating_sub(2)).max(1);
    let height = height.min(frame_area.height.saturating_sub(2)).max(1);
    let x = frame_area.x + (frame_area.width.saturating_sub(width)) / 2;
    let y = frame_area.y + (frame_area.height.saturating_sub(height)) / 2;

    Rect::new(x, y, width, height)
}

pub(crate) fn list_index_for_click(list_area: Rect, row: u16, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }

    let inner_top = list_area.y + 1;
    let inner_bottom = list_area.y + list_area.height.saturating_sub(1);
    if row < inner_top || row >= inner_bottom {
        return None;
    }

    let index = (row - inner_top) as usize;
    (index < len).then_some(index)
}

pub(crate) fn contains(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height
}

pub(crate) fn cursor_for_click(text: &str, x: u16, text_x: u16) -> usize {
    let offset = x.saturating_sub(text_x) as usize;
    let char_count = text.chars().count();
    if offset >= char_count {
        return text.len();
    }

    text.char_indices()
        .nth(offset)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}
