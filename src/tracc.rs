use super::confirm::{self, ConfirmChoice, ConfirmDialog};
use super::layout;
use super::timesheet::{self, TimePoint, TimeSheet};
use crossterm::event::{self, Event, KeyCode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, ListState, Paragraph, Wrap};
use std::{
    fs,
    io::{self, Write},
};
use time::Date;

type Terminal = ratatui::Terminal<CrosstermBackend<io::Stdout>>;

pub enum Mode {
    Insert,
    Normal,
}

#[derive(Copy, Clone)]
enum EditKind {
    Existing(usize),
    Time(usize),
    NewAt { index: usize, time: i64 },
}

struct EditState {
    kind: EditKind,
    text: String,
    cursor: usize,
}

enum PendingAction {
    BeginEdit(EditState),
    ShiftCurrent(i64),
    RemoveCurrent,
    Paste,
}

struct ConfirmState {
    message: String,
    action: PendingAction,
    selected: ConfirmChoice,
}

impl EditState {
    fn existing(index: usize, text: String) -> Self {
        let cursor = text.len();
        Self {
            kind: EditKind::Existing(index),
            text,
            cursor,
        }
    }

    fn time(index: usize, time: i64) -> Self {
        let text = format_time(time);
        let cursor = text.len();
        Self {
            kind: EditKind::Time(index),
            text,
            cursor,
        }
    }

    fn new_at(index: usize, time: i64) -> Self {
        Self {
            kind: EditKind::NewAt { index, time },
            text: String::new(),
            cursor: 0,
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

    fn move_left(&mut self) {
        self.cursor = prev_char_boundary(&self.text, self.cursor);
    }

    fn move_right(&mut self) {
        self.cursor = next_char_boundary(&self.text, self.cursor);
    }

    fn move_home(&mut self) {
        self.cursor = 0;
    }

    fn move_end(&mut self) {
        self.cursor = self.text.len();
    }
}

pub struct Tracc {
    times: TimeSheet,
    terminal: Terminal,
    input_mode: Mode,
    edit: Option<EditState>,
    confirm: Option<ConfirmState>,
    sheet_locked: bool,
}

const MAX_NEW_ITEM_MINUTES: i64 = 48 * 60;

impl Tracc {
    pub fn new(terminal: Terminal) -> Self {
        let date = TimeSheet::current_date();
        let times = TimeSheet::open(date);
        Self {
            sheet_locked: !times.is_today(),
            times,
            terminal,
            input_mode: Mode::Normal,
            edit: None,
            confirm: None,
        }
    }

    pub fn run(&mut self) -> Result<(), io::Error> {
        loop {
            self.refresh()?;
            let input = read_key()?;
            if self.handle_confirm_input(input)? {
                continue;
            }
            match self.input_mode {
                Mode::Normal => match input {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('j') => self.times.selection_down(),
                    KeyCode::Char('k') => self.times.selection_up(),
                    KeyCode::Char('J') => self.next_day()?,
                    KeyCode::Char('K') => self.previous_day()?,
                    KeyCode::Char('G') => self.times.selection_first(),
                    KeyCode::Char('g') => match read_key()? {
                        KeyCode::Char('g') => self.times.selection_last(),
                        KeyCode::Char('t') => self.goto_today()?,
                        _ => {}
                    },
                    KeyCode::Char('o') => {
                        self.begin_new_item()?;
                    }
                    KeyCode::Char('a') => {
                        let selected = self.times.selected;
                        if let Some(text) = self.times.selected_text() {
                            self.guard_mutation(
                                PendingAction::BeginEdit(EditState::existing(selected, text)),
                                self.timesheet_change_message(),
                            )?;
                        }
                    }
                    KeyCode::Char('A') => {
                        let selected = self.times.selected;
                        if let Some(time) = self.times.selected_time() {
                            self.guard_mutation(
                                PendingAction::BeginEdit(EditState::time(selected, time)),
                                self.timesheet_change_message(),
                            )?;
                        }
                    }
                    KeyCode::Char(' ') => (),
                    // Subtract only 1 minute because the number is truncated to the next multiple
                    // of 5 afterwards, so this is effectively a -5.
                    // See https://git.kageru.moe/kageru/tracc/issues/8
                    KeyCode::Char('-') => {
                        self.guard_mutation(
                            PendingAction::ShiftCurrent(-1),
                            self.timesheet_change_message(),
                        )?;
                    }
                    KeyCode::Char('+') => {
                        self.guard_mutation(
                            PendingAction::ShiftCurrent(5),
                            self.timesheet_change_message(),
                        )?;
                    }
                    KeyCode::Char('d') => {
                        self.guard_mutation(
                            PendingAction::RemoveCurrent,
                            format!(
                                "Delete the current timesheet entry for {}?",
                                self.times.date_label()
                            ),
                        )?;
                    }
                    // yy
                    KeyCode::Char('y') => {
                        if read_key()? == KeyCode::Char('y') {
                            self.times.yank();
                        }
                    }
                    KeyCode::Char('p') => {
                        self.guard_mutation(
                            PendingAction::Paste,
                            format!(
                                "Paste into {} and change its timesheet data?",
                                self.times.date_label()
                            ),
                        )?;
                    }
                    _ => (),
                },
                Mode::Insert => match input {
                    KeyCode::Enter => {
                        if self.commit_edit() {
                            self.set_mode(Mode::Normal)?;
                            self.persist_state();
                        }
                    }
                    KeyCode::Esc => {
                        self.edit = None;
                        self.set_mode(Mode::Normal)?;
                    }
                    KeyCode::Backspace => self.edit_mut().backspace(),
                    KeyCode::Left => self.edit_mut().move_left(),
                    KeyCode::Right => self.edit_mut().move_right(),
                    KeyCode::Home => self.edit_mut().move_home(),
                    KeyCode::End => self.edit_mut().move_end(),
                    KeyCode::Char(x) => self.edit_mut().insert_char(x),
                    _ => (),
                },
            };
        }
        self.terminal.clear()?;
        Ok(())
    }

    fn handle_confirm_input(&mut self, input: KeyCode) -> Result<bool, io::Error> {
        let Some(_) = self.confirm.as_ref() else {
            return Ok(false);
        };

        match input {
            KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
                if let Some(confirm) = self.confirm.as_mut() {
                    confirm.selected = confirm.selected.toggle();
                }
            }
            KeyCode::BackTab => {
                if let Some(confirm) = self.confirm.as_mut() {
                    confirm.selected = confirm.selected.toggle();
                }
            }
            KeyCode::Char('y') => {
                self.accept_confirm()?;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.reject_confirm();
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(confirm) = self.confirm.as_ref() {
                    match confirm.selected {
                        ConfirmChoice::Yes => self.accept_confirm()?,
                        ConfirmChoice::No => self.reject_confirm(),
                    }
                }
            }
            _ => {}
        }
        Ok(true)
    }

    fn guard_mutation(&mut self, action: PendingAction, message: String) -> Result<(), io::Error> {
        if self.times.is_today() || !self.sheet_locked {
            self.execute_action(action)?;
        } else {
            self.confirm = Some(ConfirmState {
                message,
                action,
                selected: ConfirmChoice::Yes,
            });
        }
        Ok(())
    }

    fn accept_confirm(&mut self) -> Result<(), io::Error> {
        if let Some(confirm) = self.confirm.take() {
            self.sheet_locked = false;
            self.execute_action(confirm.action)?;
        }
        Ok(())
    }

    fn timesheet_change_message(&self) -> String {
        format!("Change timesheet data for {}?", self.times.date_label())
    }

    fn reject_confirm(&mut self) {
        self.confirm = None;
    }

    fn execute_action(&mut self, action: PendingAction) -> Result<(), io::Error> {
        match action {
            PendingAction::BeginEdit(edit) => self.begin_edit(edit),
            PendingAction::ShiftCurrent(minutes) => {
                self.times.shift_current(minutes);
                self.persist_state();
                Ok(())
            }
            PendingAction::RemoveCurrent => {
                self.times.remove_current();
                self.persist_state();
                Ok(())
            }
            PendingAction::Paste => {
                self.times.paste();
                self.persist_state();
                Ok(())
            }
        }
    }

    fn previous_day(&mut self) -> Result<(), io::Error> {
        if let Some(date) = self.times.date.previous_day() {
            self.load_day(date)?;
        }
        Ok(())
    }

    fn next_day(&mut self) -> Result<(), io::Error> {
        let today = TimeSheet::current_date();
        if let Some(date) = self.times.date.next_day() {
            if date > today {
                return Ok(());
            }
            self.load_day(date)?;
        }
        Ok(())
    }

    fn goto_today(&mut self) -> Result<(), io::Error> {
        self.load_day(TimeSheet::current_date())
    }

    fn load_day(&mut self, date: Date) -> Result<(), io::Error> {
        self.times = TimeSheet::open(date);
        self.edit = None;
        self.confirm = None;
        self.sheet_locked = !self.times.is_today();
        self.input_mode = Mode::Normal;
        self.terminal.hide_cursor()
    }

    fn set_mode(&mut self, mode: Mode) -> Result<(), io::Error> {
        match mode {
            Mode::Insert => self.terminal.show_cursor()?,
            Mode::Normal => {
                self.edit = None;
                self.terminal.hide_cursor()?;
            }
        }
        self.input_mode = mode;
        Ok(())
    }

    fn begin_edit(&mut self, edit: EditState) -> Result<(), io::Error> {
        self.edit = Some(edit);
        self.set_mode(Mode::Insert)
    }

    fn begin_new_item(&mut self) -> Result<(), io::Error> {
        let index = self.times.insertion_index_for_now();
        let time = self.times.current_minutes_since_start();
        let edit = EditState::new_at(index, time);

        if time > MAX_NEW_ITEM_MINUTES {
            self.confirm = Some(ConfirmState {
                message: format!(
                    "The current time for this sheet is beyond 48 hours. Continue anyway?"
                ),
                action: PendingAction::BeginEdit(edit),
                selected: ConfirmChoice::No,
            });
        } else {
            self.guard_mutation(
                PendingAction::BeginEdit(edit),
                self.timesheet_change_message(),
            )?;
        }

        Ok(())
    }

    fn edit_mut(&mut self) -> &mut EditState {
        self.edit.as_mut().expect("edit mode without edit state")
    }

    fn commit_edit(&mut self) -> bool {
        let Some(edit) = self.edit.take() else {
            return true;
        };

        let EditState { kind, text, cursor } = edit;

        match kind {
            EditKind::Existing(index) => {
                self.times.selected = index;
                if text.is_empty() {
                    self.times.remove_current();
                } else {
                    self.times.set_selected_text(text);
                }
                true
            }
            EditKind::Time(index) => match self.times.set_selected_time_from_input(&text) {
                Ok(()) => true,
                Err(_) => {
                    self.edit = Some(EditState {
                        kind: EditKind::Time(index),
                        text,
                        cursor,
                    });
                    false
                }
            },
            EditKind::NewAt { index, time } => {
                if text.is_empty() {
                    true
                } else {
                    let item = TimePoint::new(&text, time);
                    self.times.insert_at(item, index);
                    true
                }
            }
        }
    }

    fn refresh(&mut self) -> Result<(), io::Error> {
        let today = TimeSheet::current_date();
        let headline = self.times_headline(today);
        let summary_content = format!(
            "Sum: {}\n{}{}\n\n{}",
            self.times.sum_as_str(),
            self.times.pause_time(),
            if self.times.has_time_overflow() {
                "\ntime overflow detected"
            } else {
                ""
            },
            self.times.time_by_tasks()
        );
        let summary = Paragraph::new(summary_content)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL));
        let times = self.times.printable();
        let timelist = layout::selectable_list(headline, &times);
        let mut state = ListState::default();
        state.select(self.times.selected_index());
        let edit = self.edit.as_ref().map(|edit| {
            let title = match edit.kind {
                EditKind::Existing(_) => " edit item ",
                EditKind::Time(_) => " edit time ",
                EditKind::NewAt { .. } => " new item ",
            };
            let anchor = match edit.kind {
                EditKind::Existing(index) => index,
                EditKind::Time(index) => index,
                EditKind::NewAt { index, .. } => index.saturating_sub(1),
            };
            (edit.text.clone(), edit.cursor, title, anchor)
        });
        let confirm = self
            .confirm
            .as_ref()
            .map(|confirm| (confirm.message.clone(), confirm.selected));

        self.terminal.draw(|frame| {
            let chunks = layout::layout(frame.area());
            frame.render_stateful_widget(timelist, chunks[0], &mut state);
            frame.render_widget(summary, chunks[1]);

            if let Some((text, cursor, title, anchor)) = edit.as_ref() {
                let popup_area = edit_area(frame.area(), chunks[0], *anchor);
                let input = Paragraph::new(text.as_str())
                    .block(Block::default().title(*title).borders(Borders::ALL));
                frame.render_widget(Clear, popup_area);
                frame.render_widget(input, popup_area);

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

    fn times_headline(&self, today: Date) -> Line<'static> {
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

    pub fn persist_state(&self) {
        fn write(path: &std::path::Path, content: &str) {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).ok();
            }
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(path)
                .ok()
                .unwrap_or_else(|| {
                    panic!("Can’t save state to JSON. Dumping raw data:\n{}", content)
                })
                .write_all(content.as_bytes())
                .unwrap();
        }
        let times_ser = serde_json::to_string(&self.times.times).unwrap();
        write(&timesheet::storage_path_for(self.times.date), &times_ser);
    }
}

fn read_key() -> Result<KeyCode, io::Error> {
    loop {
        if let Event::Key(key) = event::read()? {
            return Ok(key.code);
        }
    }
}

fn edit_area(frame_area: Rect, list_area: Rect, selected: usize) -> Rect {
    let height = 3;
    let width = list_area.width.saturating_sub(4).max(20);
    let x = list_area.x + 2;
    let below_row = list_area.y + 1 + selected as u16 + 1;
    let max_y = frame_area.y + frame_area.height.saturating_sub(height);
    let y = below_row.min(max_y);

    Rect::new(x, y, width.min(frame_area.width.saturating_sub(2)), height)
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

fn format_time(minutes: i64) -> String {
    let hours = minutes.div_euclid(60);
    let minutes = minutes.rem_euclid(60);
    format!("{:02}:{:02}", hours, minutes)
}
