use super::layout;
use super::timesheet::TimeSheet;
use ratatui::backend::TermionBackend;
use ratatui::widgets::{Block, Borders, ListState, Paragraph, Wrap};
use std::default::Default;
use std::io::{self, Write};
use termion::event::Key;
use termion::input::TermRead;

type Terminal = ratatui::Terminal<TermionBackend<termion::raw::RawTerminal<io::Stdout>>>;
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
        let mut inputs = io::stdin().keys();
        loop {
            self.refresh()?;
            // I need to find a better way to handle inputs. This is awful.
            let input = inputs.next().unwrap()?;
            match self.input_mode {
                Mode::Normal => match input {
                    Key::Char('q') => break,
                    Key::Char('j') => self.times.selection_down(),
                    Key::Char('k') => self.times.selection_up(),
                    Key::Char('G') => self.times.selection_first(),
                    // gg
                    Key::Char('g') => {
                        if let Some(Ok(Key::Char('g'))) = inputs.next() {
                            self.times.selection_last();
                        }
                    }
                    Key::Char('o') => {
                        self.times.insert(Default::default(), None);
                        self.set_mode(Mode::Insert)?;
                    }
                    Key::Char('a') | Key::Char('A') => self.set_mode(Mode::Insert)?,
                    Key::Char(' ') => (),
                    // Subtract only 1 minute because the number is truncated to the next multiple
                    // of 5 afterwards, so this is effectively a -5.
                    // See https://git.kageru.moe/kageru/tracc/issues/8
                    Key::Char('-') => {
                        self.times.shift_current(-1);
                        self.persist_state();
                    }
                    Key::Char('+') => {
                        self.times.shift_current(5);
                        self.persist_state();
                    }
                    // dd
                    Key::Char('d') => {
                        if let Some(Ok(Key::Char('d'))) = inputs.next() {
                            self.times.remove_current();
                        }
                        self.persist_state();
                    }
                    // yy
                    Key::Char('y') => {
                        if let Some(Ok(Key::Char('y'))) = inputs.next() {
                            self.times.yank();
                        }
                    }
                    Key::Char('p') => {
                        self.times.paste();
                        self.persist_state();
                    }
                    _ => (),
                },
                Mode::Insert => match input {
                    Key::Char('\n') | Key::Esc => {
                        self.set_mode(Mode::Normal)?;
                        self.persist_state();
                    }
                    Key::Backspace => self.times.backspace(),
                    Key::Char(x) => self.times.append_to_current(x),
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
