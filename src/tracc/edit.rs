use super::{render, Tracc};
use crate::timesheet::{self, TimePoint};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use std::convert::TryFrom;
use time::{Date, Month};

#[derive(Copy, Clone)]
pub(crate) enum EditKind {
    Text(usize),
    Time(usize),
    NewAt { index: usize, time: i64 },
    LoadDay(usize),
}

#[derive(Copy, Clone)]
pub(crate) enum EditOutcome {
    Continue,
    Commit,
    Cancel,
}

pub(crate) struct EditState {
    pub(crate) kind: EditKind,
    pub(crate) text: String,
    pub(crate) cursor: usize,
}

impl EditState {
    pub(crate) fn text(index: usize, text: String) -> Self {
        let cursor = text.len();
        Self {
            kind: EditKind::Text(index),
            text,
            cursor,
        }
    }

    pub(crate) fn text_at_start(index: usize, text: String) -> Self {
        Self {
            kind: EditKind::Text(index),
            text,
            cursor: 0,
        }
    }

    pub(crate) fn text_empty(index: usize) -> Self {
        Self {
            kind: EditKind::Text(index),
            text: String::new(),
            cursor: 0,
        }
    }

    pub(crate) fn time(index: usize, time: i64) -> Self {
        let text = format_time(time);
        let cursor = text.len();
        Self {
            kind: EditKind::Time(index),
            text,
            cursor,
        }
    }

    pub(crate) fn time_at_start(index: usize, time: i64) -> Self {
        Self {
            kind: EditKind::Time(index),
            text: format_time(time),
            cursor: 0,
        }
    }

    pub(crate) fn time_empty(index: usize) -> Self {
        Self {
            kind: EditKind::Time(index),
            text: String::new(),
            cursor: 0,
        }
    }

    pub(crate) fn new_at(index: usize, time: i64) -> Self {
        Self {
            kind: EditKind::NewAt { index, time },
            text: String::new(),
            cursor: 0,
        }
    }

    pub(crate) fn date(index: usize, date: Date) -> Self {
        let text = format_date(date);
        let cursor = text.len();
        Self {
            kind: EditKind::LoadDay(index),
            text,
            cursor,
        }
    }

    pub(crate) fn handle_key(&mut self, input: KeyEvent) -> EditOutcome {
        match input.code {
            KeyCode::Esc => return EditOutcome::Cancel,
            KeyCode::Enter => return EditOutcome::Commit,
            KeyCode::Char('j')
                if input
                    .state
                    .contains(crossterm::event::KeyEventState::KEYPAD) =>
            {
                return EditOutcome::Commit;
            }
            KeyCode::Backspace if input.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_prev_word()
            }
            KeyCode::Delete if input.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_next_word()
            }
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete_next_char(),
            KeyCode::Left if input.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_prev_word()
            }
            KeyCode::Right if input.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_next_word()
            }
            KeyCode::Left => self.move_left(),
            KeyCode::Right => self.move_right(),
            KeyCode::Home => self.move_home(),
            KeyCode::End => self.move_end(),
            KeyCode::Char(x) => self.insert_char(x),
            _ => (),
        };
        EditOutcome::Continue
    }

    pub(crate) fn popup_title(&self) -> &'static str {
        match self.kind {
            EditKind::Text(_) => " edit item ",
            EditKind::Time(_) => " edit time ",
            EditKind::NewAt { .. } => " new item ",
            EditKind::LoadDay(_) => " load date ",
        }
    }

    pub(crate) fn anchor(&self) -> usize {
        match self.kind {
            EditKind::Text(index) => index,
            EditKind::Time(index) => index,
            EditKind::NewAt { index, .. } => index.saturating_sub(1),
            EditKind::LoadDay(index) => index,
        }
    }

    pub(crate) fn popup_area(&self, frame_area: Rect, list_area: Rect) -> Rect {
        match self.kind {
            EditKind::LoadDay(_) => render::centered_area(frame_area, 13, 3),
            _ => render::edit_area(frame_area, list_area, self.anchor()),
        }
    }

    fn insert_char(&mut self, chr: char) {
        self.text.insert(self.cursor, chr);
        self.cursor += chr.len_utf8();
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev = prev_char_boundary(&self.text, self.cursor);
        self.text.drain(prev..self.cursor);
        self.cursor = prev;
    }

    fn delete_next_char(&mut self) {
        if self.cursor >= self.text.len() {
            return;
        }
        let next = next_char_boundary(&self.text, self.cursor);
        self.text.drain(self.cursor..next);
    }

    fn delete_prev_word(&mut self) {
        let start = prev_word_boundary(&self.text, self.cursor);
        if start == self.cursor {
            return;
        }
        self.text.drain(start..self.cursor);
        self.cursor = start;
    }

    fn delete_next_word(&mut self) {
        let end = next_word_boundary(&self.text, self.cursor);
        if end == self.cursor {
            return;
        }
        self.text.drain(self.cursor..end);
    }

    fn move_left(&mut self) {
        self.cursor = prev_char_boundary(&self.text, self.cursor);
    }

    fn move_right(&mut self) {
        self.cursor = next_char_boundary(&self.text, self.cursor);
    }

    fn move_prev_word(&mut self) {
        self.cursor = prev_word_boundary(&self.text, self.cursor);
    }

    fn move_next_word(&mut self) {
        self.cursor = next_word_boundary(&self.text, self.cursor);
    }

    fn move_home(&mut self) {
        self.cursor = 0;
    }

    fn move_end(&mut self) {
        self.cursor = self.text.len();
    }
}

impl Tracc {
    pub(crate) fn commit_edit(
        &mut self,
        edit: EditState,
    ) -> Result<Option<EditState>, std::io::Error> {
        let EditState { kind, text, cursor } = edit;

        match kind {
            EditKind::Text(index) => {
                self.record_change_snapshot();
                self.times.selected = index;
                if text.is_empty() {
                    self.times.remove_current();
                } else {
                    self.times.set_selected_text(text);
                }
                self.persist_state();
                Ok(None)
            }
            EditKind::Time(index) => match timesheet::parse_minutes(&text) {
                Ok(time) => {
                    self.record_change_snapshot();
                    self.times.set_selected_time(time);
                    self.persist_state();
                    Ok(None)
                }
                Err(_) => Ok(Some(EditState {
                    kind: EditKind::Time(index),
                    text,
                    cursor,
                })),
            },
            EditKind::NewAt { index, time } => {
                if text.is_empty() {
                    self.persist_state();
                    Ok(None)
                } else {
                    self.record_change_snapshot();
                    let item = TimePoint::new(&text, time);
                    self.times.insert_at(item, index);
                    self.persist_state();
                    Ok(None)
                }
            }
            EditKind::LoadDay(index) => match parse_date(&text) {
                Ok(date) => {
                    self.load_day(date)?;
                    Ok(None)
                }
                Err(_) => Ok(Some(EditState {
                    kind: EditKind::LoadDay(index),
                    text,
                    cursor,
                })),
            },
        }
    }
}

pub(crate) fn format_date(date: Date) -> String {
    let (year, month, day) = date.to_calendar_date();
    format!("{:04}-{:02}-{:02}", year, u8::from(month), day)
}

pub(crate) fn parse_date(value: &str) -> Result<Date, String> {
    let value = value.trim();
    let Some((year, rest)) = value.split_once('-') else {
        return Err(format!("invalid date value: {value}"));
    };
    let Some((month, day)) = rest.split_once('-') else {
        return Err(format!("invalid date value: {value}"));
    };

    let year = year
        .parse::<i32>()
        .map_err(|_| format!("invalid year in date value: {value}"))?;
    let month = month
        .parse::<u8>()
        .map_err(|_| format!("invalid month in date value: {value}"))?;
    let day = day
        .parse::<u8>()
        .map_err(|_| format!("invalid day in date value: {value}"))?;

    Date::from_calendar_date(
        year,
        Month::try_from(month).map_err(|_| format!("invalid month in date value: {value}"))?,
        day,
    )
    .map_err(|_| format!("invalid date value: {value}"))
}

pub(crate) fn format_time(minutes: i64) -> String {
    let hours = minutes.div_euclid(60);
    let minutes = minutes.rem_euclid(60);
    format!("{:02}:{:02}", hours, minutes)
}

fn prev_char_boundary(text: &str, idx: usize) -> usize {
    text[..idx]
        .char_indices()
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn next_char_boundary(text: &str, idx: usize) -> usize {
    text[idx..]
        .chars()
        .next()
        .map(|chr| idx + chr.len_utf8())
        .unwrap_or(idx)
}

fn prev_word_boundary(text: &str, idx: usize) -> usize {
    let mut start = idx;

    while start > 0 {
        let prev = prev_char_boundary(text, start);
        if !text[prev..start].chars().all(char::is_whitespace) {
            break;
        }
        start = prev;
    }

    while start > 0 {
        let prev = prev_char_boundary(text, start);
        if text[prev..start].chars().all(char::is_whitespace) {
            break;
        }
        start = prev;
    }

    start
}

fn next_word_boundary(text: &str, idx: usize) -> usize {
    let mut end = idx;

    while end < text.len() {
        let next = next_char_boundary(text, end);
        if !text[end..next].chars().all(char::is_whitespace) {
            break;
        }
        end = next;
    }

    while end < text.len() {
        let next = next_char_boundary(text, end);
        if text[end..next].chars().all(char::is_whitespace) {
            break;
        }
        end = next;
    }

    end
}
