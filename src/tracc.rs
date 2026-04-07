use super::layout;
use super::timesheet::{self, TimePoint, TimeSheet};
use crossterm::event::{self, Event, KeyCode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Clear, ListState, Paragraph, Wrap};
use std::{
    fs,
    io::{self, Write},
    path::Path,
};

type Terminal = ratatui::Terminal<CrosstermBackend<io::Stdout>>;

pub enum Mode {
    Insert,
    Normal,
}

enum EditKind {
    Existing(usize),
    NewAt(usize),
}

struct EditState {
    kind: EditKind,
    text: String,
    cursor: usize,
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

    fn new_at(index: usize) -> Self {
        Self {
            kind: EditKind::NewAt(index),
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
}

impl Tracc {
    pub fn new(terminal: Terminal) -> Self {
        let path = timesheet::storage_path();
        Self {
            times: TimeSheet::open_or_create(&path),
            terminal,
            input_mode: Mode::Normal,
            edit: None,
        }
    }

    pub fn run(&mut self) -> Result<(), io::Error> {
        loop {
            self.refresh()?;
            let input = read_key()?;
            match self.input_mode {
                Mode::Normal => match input {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('j') => self.times.selection_down(),
                    KeyCode::Char('k') => self.times.selection_up(),
                    KeyCode::Char('G') => self.times.selection_first(),
                    KeyCode::Char('g') => {
                        if read_key()? == KeyCode::Char('g') {
                            self.times.selection_last();
                        }
                    }
                    KeyCode::Char('o') => {
                        let index = self.times.insertion_index_for_now();
                        self.begin_edit(EditState::new_at(index))?
                    }
                    KeyCode::Char('a') | KeyCode::Char('A') => {
                        let selected = self.times.selected;
                        let text = self.times.selected_text();
                        self.begin_edit(EditState::existing(selected, text))?;
                    }
                    KeyCode::Char(' ') => (),
                    // Subtract only 1 minute because the number is truncated to the next multiple
                    // of 5 afterwards, so this is effectively a -5.
                    // See https://git.kageru.moe/kageru/tracc/issues/8
                    KeyCode::Char('-') => {
                        self.times.shift_current(-1);
                        self.persist_state();
                    }
                    KeyCode::Char('+') => {
                        self.times.shift_current(5);
                        self.persist_state();
                    }
                    // dd
                    KeyCode::Char('d') => {
                        if read_key()? == KeyCode::Char('d') {
                            self.times.remove_current();
                        }
                        self.persist_state();
                    }
                    // yy
                    KeyCode::Char('y') => {
                        if read_key()? == KeyCode::Char('y') {
                            self.times.yank();
                        }
                    }
                    KeyCode::Char('p') => {
                        self.times.paste();
                        self.persist_state();
                    }
                    _ => (),
                },
                Mode::Insert => match input {
                    KeyCode::Enter => {
                        self.commit_edit();
                        self.set_mode(Mode::Normal)?;
                        self.persist_state();
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
        self.persist_state();
        Ok(())
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

    fn edit_mut(&mut self) -> &mut EditState {
        self.edit.as_mut().expect("edit mode without edit state")
    }

    fn commit_edit(&mut self) {
        let Some(edit) = self.edit.take() else {
            return;
        };

        match edit.kind {
            EditKind::Existing(index) => {
                self.times.selected = index;
                if edit.text.is_empty() {
                    self.times.remove_current();
                } else {
                    self.times.set_selected_text(edit.text);
                }
            }
            EditKind::NewAt(index) => {
                if edit.text.is_empty() {
                    return;
                }
                let item = TimePoint::new(&edit.text);
                self.times.insert_at(item, index);
            }
        }
    }

    fn refresh(&mut self) -> Result<(), io::Error> {
        let summary_content = format!(
            "Sum for today: {}\n{}\n\n{}",
            self.times.sum_as_str(),
            self.times.pause_time(),
            self.times.time_by_tasks()
        );
        let summary = Paragraph::new(summary_content)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL));
        let times = self.times.printable();
        let timelist = layout::selectable_list(" 🕑 ", &times);
        let mut state = ListState::default();
        let selected = self.times.selected;
        state.select(Some(selected));
        let edit = self.edit.as_ref().map(|edit| {
            let title = match edit.kind {
                EditKind::Existing(_) => " edit item ",
                EditKind::NewAt(_) => " new item ",
            };
            let anchor = match edit.kind {
                EditKind::Existing(index) => index,
                EditKind::NewAt(index) => index.saturating_sub(1),
            };
            (edit.text.clone(), edit.cursor, title, anchor)
        });

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
        })?;
        Ok(())
    }

    pub fn persist_state(&self) {
        fn write(path: &Path, content: &str) {
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
        write(&timesheet::storage_path(), &times_ser);
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
