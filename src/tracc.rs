use super::confirm::{self, ConfirmChoice, ConfirmDialog};
use super::layout;
use super::timesheet::{self, TimePoint, TimeSheet};
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, ListState, Padding, Paragraph, Wrap};
use std::{
    collections::VecDeque,
    fs,
    io::{self, Write},
};
use time::Date;

type Terminal = ratatui::Terminal<CrosstermBackend<io::Stdout>>;

#[derive(Copy, Clone)]
enum EditKind {
    Existing(usize),
    Time(usize),
    NewAt { index: usize, time: i64 },
}

#[derive(Copy, Clone)]
enum PrefixState {
    G,
    Y,
}

enum InputState {
    Normal,
    Editing(EditState),
    Confirm(ConfirmState),
    Prefix(PrefixState),
    Quit,
}

enum EditOutcome {
    Continue,
    Commit,
    Cancel,
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

    fn existing_at_start(index: usize, text: String) -> Self {
        Self {
            kind: EditKind::Existing(index),
            text,
            cursor: 0,
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

    fn time_at_start(index: usize, time: i64) -> Self {
        Self {
            kind: EditKind::Time(index),
            text: format_time(time),
            cursor: 0,
        }
    }

    fn new_at(index: usize, time: i64) -> Self {
        Self {
            kind: EditKind::NewAt { index, time },
            text: String::new(),
            cursor: 0,
        }
    }

    fn handle_key(&mut self, input: KeyEvent) -> EditOutcome {
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

    fn popup_title(&self) -> &'static str {
        match self.kind {
            EditKind::Existing(_) => " edit item ",
            EditKind::Time(_) => " edit time ",
            EditKind::NewAt { .. } => " new item ",
        }
    }

    fn anchor(&self) -> usize {
        match self.kind {
            EditKind::Existing(index) => index,
            EditKind::Time(index) => index,
            EditKind::NewAt { index, .. } => index.saturating_sub(1),
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

pub struct Tracc {
    times: TimeSheet,
    terminal: Terminal,
    input_state: InputState,
    frame_area: Rect,
    list_area: Rect,
    sheet_locked: bool,
    undo_history: VecDeque<TimeSheet>,
    redo_history: VecDeque<TimeSheet>,
}

const MAX_NEW_ITEM_MINUTES: i64 = 48 * 60;
const MAX_UNDO_SNAPSHOTS: usize = 20;

impl Tracc {
    pub fn new(terminal: Terminal) -> Self {
        let date = TimeSheet::current_date();
        let times = TimeSheet::open(date);
        Self {
            sheet_locked: !times.is_today(),
            times,
            terminal,
            input_state: InputState::Normal,
            frame_area: Rect::default(),
            list_area: Rect::default(),
            undo_history: VecDeque::new(),
            redo_history: VecDeque::new(),
        }
    }

    pub fn run(&mut self) -> Result<(), io::Error> {
        loop {
            self.refresh()?;
            let input = event::read()?;
            self.handle_input(input)?;
            if matches!(self.input_state, InputState::Quit) {
                break;
            }
        }
        self.terminal.clear()?;
        Ok(())
    }

    fn handle_input(&mut self, input: Event) -> Result<(), io::Error> {
        match input {
            Event::Key(input) => {
                let state = std::mem::replace(&mut self.input_state, InputState::Normal);
                self.input_state = match state {
                    InputState::Normal => self.handle_normal_input(input)?,
                    InputState::Editing(edit) => self.handle_edit_input(edit, input)?,
                    InputState::Confirm(confirm) => self.handle_confirm_input(confirm, input)?,
                    InputState::Prefix(prefix) => self.handle_prefix_input(prefix, input)?,
                    InputState::Quit => InputState::Quit,
                };
            }
            Event::Mouse(mouse) => self.handle_mouse_input(mouse)?,
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse_input(&mut self, mouse: MouseEvent) -> Result<(), io::Error> {
        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return Ok(());
        }

        match &mut self.input_state {
            InputState::Normal => {
                if let Some(index) =
                    list_index_for_click(self.list_area, mouse.row, self.times.times.len())
                {
                    self.times.selected = index;
                }
            }
            InputState::Editing(edit) => {
                let popup_area = edit_area(self.frame_area, self.list_area, edit.anchor());
                if contains(popup_area, mouse.column, mouse.row) {
                    edit.cursor = cursor_for_click(&edit.text, mouse.column, popup_area.x + 1);
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_normal_input(&mut self, input: KeyEvent) -> Result<InputState, io::Error> {
        match input.code {
            KeyCode::Char('q') => Ok(InputState::Quit),
            KeyCode::Char('j') => {
                self.times.selection_down();
                Ok(InputState::Normal)
            }
            KeyCode::Char('k') => {
                self.times.selection_up();
                Ok(InputState::Normal)
            }
            KeyCode::Char('J') => {
                self.next_day()?;
                Ok(InputState::Normal)
            }
            KeyCode::Char('K') => {
                self.previous_day()?;
                Ok(InputState::Normal)
            }
            KeyCode::Char('G') => {
                self.times.selection_first();
                Ok(InputState::Normal)
            }
            KeyCode::Char('g') => Ok(InputState::Prefix(PrefixState::G)),
            KeyCode::Char('y') => Ok(InputState::Prefix(PrefixState::Y)),
            KeyCode::Char('o') => self.begin_new_item(),
            KeyCode::Char('a') => {
                let selected = self.times.selected;
                if let Some(text) = self.times.selected_text() {
                    self.guard_mutation(
                        PendingAction::BeginEdit(EditState::existing(selected, text)),
                        self.timesheet_change_message(),
                    )
                } else {
                    Ok(InputState::Normal)
                }
            }
            KeyCode::Char('i') => {
                let selected = self.times.selected;
                if let Some(text) = self.times.selected_text() {
                    self.guard_mutation(
                        PendingAction::BeginEdit(EditState::existing_at_start(selected, text)),
                        self.timesheet_change_message(),
                    )
                } else {
                    Ok(InputState::Normal)
                }
            }
            KeyCode::Char('A') => {
                let selected = self.times.selected;
                if let Some(time) = self.times.selected_time() {
                    self.guard_mutation(
                        PendingAction::BeginEdit(EditState::time(selected, time)),
                        self.timesheet_change_message(),
                    )
                } else {
                    Ok(InputState::Normal)
                }
            }
            KeyCode::Char('I') => {
                let selected = self.times.selected;
                if let Some(time) = self.times.selected_time() {
                    self.guard_mutation(
                        PendingAction::BeginEdit(EditState::time_at_start(selected, time)),
                        self.timesheet_change_message(),
                    )
                } else {
                    Ok(InputState::Normal)
                }
            }
            KeyCode::Char(' ') => Ok(InputState::Normal),
            // Subtract only 1 minute because the number is truncated to the next multiple
            // of 5 afterwards, so this is effectively a -5.
            // See https://git.kageru.moe/kageru/tracc/issues/8
            KeyCode::Char('-') => self.guard_mutation(
                PendingAction::ShiftCurrent(-1),
                self.timesheet_change_message(),
            ),
            KeyCode::Char('+') => self.guard_mutation(
                PendingAction::ShiftCurrent(5),
                self.timesheet_change_message(),
            ),
            KeyCode::Char('d') => self.guard_mutation(
                PendingAction::RemoveCurrent,
                format!(
                    "Delete the current timesheet entry for {}?",
                    self.times.date_label()
                ),
            ),
            KeyCode::Char('p') => self.guard_mutation(
                PendingAction::Paste,
                format!(
                    "Paste into {} and change its timesheet data?",
                    self.times.date_label()
                ),
            ),
            KeyCode::Char('u') => {
                self.undo_previous_edit()?;
                Ok(InputState::Normal)
            }
            KeyCode::Char('r') if input.modifiers.contains(KeyModifiers::CONTROL) => {
                self.redo_previous_edit()?;
                Ok(InputState::Normal)
            }
            _ => Ok(InputState::Normal),
        }
    }

    fn handle_prefix_input(
        &mut self,
        prefix: PrefixState,
        input: KeyEvent,
    ) -> Result<InputState, io::Error> {
        match prefix {
            PrefixState::G => {
                match input.code {
                    KeyCode::Char('g') => self.times.selection_last(),
                    KeyCode::Char('t') => self.goto_today()?,
                    _ => {}
                }
                Ok(InputState::Normal)
            }
            PrefixState::Y => {
                if matches!(input.code, KeyCode::Char('y')) {
                    self.times.yank();
                }
                Ok(InputState::Normal)
            }
        }
    }

    fn handle_confirm_input(
        &mut self,
        mut confirm: ConfirmState,
        input: KeyEvent,
    ) -> Result<InputState, io::Error> {
        match input.code {
            KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                confirm.selected = confirm.selected.toggle();
                Ok(InputState::Confirm(confirm))
            }
            KeyCode::Char('y') => Ok(self.accept_confirm(confirm)?),
            KeyCode::Char('n') | KeyCode::Esc => Ok(InputState::Normal),
            KeyCode::Enter | KeyCode::Char(' ') => match confirm.selected {
                ConfirmChoice::Yes => Ok(self.accept_confirm(confirm)?),
                ConfirmChoice::No => Ok(InputState::Normal),
            },
            _ => Ok(InputState::Confirm(confirm)),
        }
    }

    fn handle_edit_input(
        &mut self,
        mut edit: EditState,
        input: KeyEvent,
    ) -> Result<InputState, io::Error> {
        match edit.handle_key(input) {
            EditOutcome::Continue => Ok(InputState::Editing(edit)),
            EditOutcome::Cancel => {
                self.terminal.hide_cursor()?;
                Ok(InputState::Normal)
            }
            EditOutcome::Commit => match self.commit_edit(edit) {
                Some(edit) => Ok(InputState::Editing(edit)),
                None => {
                    self.terminal.hide_cursor()?;
                    Ok(InputState::Normal)
                }
            },
        }
    }

    fn guard_mutation(
        &mut self,
        action: PendingAction,
        message: String,
    ) -> Result<InputState, io::Error> {
        if self.times.is_today() || !self.sheet_locked {
            self.execute_action(action)
        } else {
            Ok(InputState::Confirm(ConfirmState {
                message,
                action,
                selected: ConfirmChoice::Yes,
            }))
        }
    }

    fn timesheet_change_message(&self) -> String {
        format!("Change timesheet data for {}?", self.times.date_label())
    }

    fn accept_confirm(&mut self, confirm: ConfirmState) -> Result<InputState, io::Error> {
        self.sheet_locked = false;
        self.execute_action(confirm.action)
    }

    fn execute_action(&mut self, action: PendingAction) -> Result<InputState, io::Error> {
        match action {
            PendingAction::BeginEdit(edit) => self.begin_edit(edit),
            PendingAction::ShiftCurrent(minutes) => {
                if self.times.selected_index().is_some() {
                    self.record_change_snapshot();
                    self.times.shift_current(minutes);
                    self.persist_state();
                }
                Ok(InputState::Normal)
            }
            PendingAction::RemoveCurrent => {
                if self.times.selected_index().is_some() {
                    self.record_change_snapshot();
                    self.times.remove_current();
                    self.persist_state();
                }
                Ok(InputState::Normal)
            }
            PendingAction::Paste => {
                if self.times.can_paste() {
                    self.record_change_snapshot();
                    self.times.paste();
                    self.persist_state();
                }
                Ok(InputState::Normal)
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
        self.input_state = InputState::Normal;
        self.undo_history.clear();
        self.redo_history.clear();
        self.sheet_locked = !self.times.is_today();
        self.terminal.hide_cursor()
    }

    fn begin_edit(&mut self, edit: EditState) -> Result<InputState, io::Error> {
        self.terminal.show_cursor()?;
        Ok(InputState::Editing(edit))
    }

    fn begin_new_item(&mut self) -> Result<InputState, io::Error> {
        let index = self.times.insertion_index_for_now();
        let time = self.times.current_minutes_since_start();
        let edit = EditState::new_at(index, time);

        if time > MAX_NEW_ITEM_MINUTES {
            Ok(InputState::Confirm(ConfirmState {
                message: "The current time for this sheet is beyond 48 hours. Continue anyway?"
                    .to_string(),
                action: PendingAction::BeginEdit(edit),
                selected: ConfirmChoice::No,
            }))
        } else {
            self.guard_mutation(
                PendingAction::BeginEdit(edit),
                self.timesheet_change_message(),
            )
        }
    }

    fn commit_edit(&mut self, edit: EditState) -> Option<EditState> {
        let EditState { kind, text, cursor } = edit;

        match kind {
            EditKind::Existing(index) => {
                self.record_change_snapshot();
                self.times.selected = index;
                if text.is_empty() {
                    self.times.remove_current();
                } else {
                    self.times.set_selected_text(text);
                }
                self.persist_state();
                None
            }
            EditKind::Time(index) => match timesheet::parse_minutes(&text) {
                Ok(time) => {
                    self.record_change_snapshot();
                    self.times.set_selected_time(time);
                    self.persist_state();
                    None
                }
                Err(_) => Some(EditState {
                    kind: EditKind::Time(index),
                    text,
                    cursor,
                }),
            },
            EditKind::NewAt { index, time } => {
                if text.is_empty() {
                    self.persist_state();
                    None
                } else {
                    self.record_change_snapshot();
                    let item = TimePoint::new(&text, time);
                    self.times.insert_at(item, index);
                    self.persist_state();
                    None
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
        let edit = match &self.input_state {
            InputState::Editing(edit) => Some((
                edit.text.clone(),
                edit.cursor,
                edit.popup_title(),
                edit.anchor(),
            )),
            _ => None,
        };
        let confirm = match &self.input_state {
            InputState::Confirm(confirm) => Some((confirm.message.clone(), confirm.selected)),
            _ => None,
        };
        let frame_size = self.terminal.size()?;
        let frame_area = Rect::new(0, 0, frame_size.width, frame_size.height);
        let chunks = layout::layout(frame_area);
        self.frame_area = frame_area;
        self.list_area = chunks[0];

        self.terminal.draw(|frame| {
            frame.render_stateful_widget(timelist, chunks[0], &mut state);
            frame.render_widget(summary, chunks[1]);

            if let Some((text, cursor, title, anchor)) = edit.as_ref() {
                let popup_area = edit_area(frame_area, chunks[0], *anchor);
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

    fn record_change_snapshot(&mut self) {
        self.redo_history.clear();
        self.push_history();
    }

    fn push_history(&mut self) {
        if self.undo_history.len() == MAX_UNDO_SNAPSHOTS {
            self.undo_history.pop_front();
        }
        self.undo_history.push_back(self.times.clone());
    }

    fn undo_previous_edit(&mut self) -> Result<(), io::Error> {
        let Some(previous) = self.undo_history.pop_back() else {
            return Ok(());
        };

        let current = self.times.clone();
        self.redo_history.push_back(current);

        self.times = previous;
        self.input_state = InputState::Normal;
        self.sheet_locked = !self.times.is_today();
        self.persist_state();
        self.terminal.hide_cursor()
    }

    fn redo_previous_edit(&mut self) -> Result<(), io::Error> {
        let Some(next) = self.redo_history.pop_back() else {
            return Ok(());
        };

        let current = self.times.clone();
        self.undo_history.push_back(current);

        self.times = next;
        self.input_state = InputState::Normal;
        self.sheet_locked = !self.times.is_today();
        self.persist_state();
        self.terminal.hide_cursor()
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

fn format_time(minutes: i64) -> String {
    let hours = minutes.div_euclid(60);
    let minutes = minutes.rem_euclid(60);
    format!("{:02}:{:02}", hours, minutes)
}

fn list_index_for_click(list_area: Rect, row: u16, len: usize) -> Option<usize> {
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

fn contains(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height
}

fn cursor_for_click(text: &str, x: u16, text_x: u16) -> usize {
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
