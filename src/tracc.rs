use super::layout;
use super::timesheet::TimeSheet;
use crossterm::event::{self, Event, KeyCode};
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::{Block, Borders, ListState, Paragraph, Wrap};
use std::default::Default;
use std::io::{self, Write};

type Terminal = ratatui::Terminal<CrosstermBackend<io::Stdout>>;
const JSON_PATH_TIME: &str = "tracc_time.json";

pub enum Mode {
    Insert,
    Normal,
}

pub struct Tracc {
    times: TimeSheet,
    terminal: Terminal,
    input_mode: Mode,
}

impl Tracc {
    pub fn new(terminal: Terminal) -> Self {
        Self {
            times: TimeSheet::open_or_create(JSON_PATH_TIME),
            terminal,
            input_mode: Mode::Normal,
        }
    }

    pub fn run(&mut self) -> Result<(), io::Error> {
        loop {
            self.refresh()?;
            // I need to find a better way to handle inputs. This is awful.
            let input = read_key()?;
            match self.input_mode {
                Mode::Normal => match input {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('j') => self.times.selection_down(),
                    KeyCode::Char('k') => self.times.selection_up(),
                    KeyCode::Char('G') => self.times.selection_first(),
                    // gg
                    KeyCode::Char('g') => {
                        if read_key()? == KeyCode::Char('g') {
                            self.times.selection_last();
                        }
                    }
                    KeyCode::Char('o') => {
                        self.times.insert(Default::default(), None);
                        self.set_mode(Mode::Insert)?;
                    }
                    KeyCode::Char('a') | KeyCode::Char('A') => self.set_mode(Mode::Insert)?,
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
                    KeyCode::Enter | KeyCode::Esc => {
                        self.set_mode(Mode::Normal)?;
                        self.persist_state();
                    }
                    KeyCode::Backspace => self.times.backspace(),
                    KeyCode::Char(x) => self.times.append_to_current(x),
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
                self.times.normal_mode();
                self.terminal.hide_cursor()?;
            }
        };
        self.input_mode = mode;
        Ok(())
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
        state.select(Some(self.times.selected));

        self.terminal.draw(|frame| {
            let chunks = layout::layout(frame.area());
            frame.render_stateful_widget(timelist, chunks[0], &mut state);
            frame.render_widget(summary, chunks[1]);
        })?;
        Ok(())
    }

    pub fn persist_state(&self) {
        fn write(path: &str, content: String) {
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(path)
                .ok()
                .or_else(|| panic!("Can’t save state to JSON. Dumping raw data:\n{}", content))
                .map(|mut f| f.write(content.as_bytes()));
        }
        let times_ser = serde_json::to_string(&self.times.times).unwrap();
        write(JSON_PATH_TIME, times_ser);
    }
}

fn read_key() -> Result<KeyCode, io::Error> {
    loop {
        if let Event::Key(key) = event::read()? {
            return Ok(key.code);
        }
    }
}
