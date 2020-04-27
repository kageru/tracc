use super::layout;
use super::listview::ListView;
use super::timesheet::TimeSheet;
use super::todolist::TodoList;
use std::default::Default;
use std::io::{self, Write};
use termion::event::Key;
use termion::input::TermRead;
use tui::backend::TermionBackend;
use tui::widgets::*;

type Terminal = tui::Terminal<TermionBackend<termion::raw::RawTerminal<io::Stdout>>>;
const JSON_PATH_TIME: &str = "tracc_time.json";
const JSON_PATH_TODO: &str = "tracc_todo.json";

pub enum Mode {
    Insert,
    Normal,
}

#[derive(PartialEq)]
enum Focus {
    Top,
    Bottom,
}

pub struct Tracc {
    todos: TodoList,
    times: TimeSheet,
    terminal: Terminal,
    input_mode: Mode,
    focus: Focus,
}

impl Tracc {
    pub fn new(terminal: Terminal) -> Self {
        Self {
            todos: TodoList::open_or_create(JSON_PATH_TODO),
            times: TimeSheet::open_or_create(JSON_PATH_TIME),
            terminal,
            input_mode: Mode::Normal,
            focus: Focus::Top,
        }
    }

    pub fn run(&mut self) -> Result<(), io::Error> {
        macro_rules! with_focused {
            ($action: expr $(, $arg: expr)*) => {
                match self.focus {
                    Focus::Top => $action(&mut self.todos, $($arg,)*),
                    Focus::Bottom => $action(&mut self.times, $($arg,)*),
                }
            };
        };

        let mut inputs = io::stdin().keys();
        loop {
            self.refresh()?;
            // I need to find a better way to handle inputs. This is awful.
            let input = inputs.next().unwrap()?;
            match self.input_mode {
                Mode::Normal => match input {
                    Key::Char('q') => break,
                    Key::Char('j') => with_focused!(ListView::selection_down),
                    Key::Char('k') => with_focused!(ListView::selection_up),
                    Key::Char('o') => {
                        with_focused!(ListView::insert, Default::default(), None);
                        self.set_mode(Mode::Insert)?;
                    }
                    Key::Char('a') | Key::Char('A') => self.set_mode(Mode::Insert)?,
                    Key::Char(' ') if self.focus == Focus::Top => self.todos.toggle_current(),
                    // dd
                    Key::Char('d') => {
                        if let Some(Ok(Key::Char('d'))) = inputs.next() {
                            with_focused!(ListView::remove_current);
                        }
                    }
                    // yy
                    Key::Char('y') => {
                        if let Some(Ok(Key::Char('y'))) = inputs.next() {
                            with_focused!(ListView::yank);
                        }
                    }
                    Key::Char('p') => with_focused!(ListView::paste),
                    Key::Char('\t') => {
                        self.focus = match self.focus {
                            Focus::Top => Focus::Bottom,
                            Focus::Bottom => Focus::Top,
                        }
                    }
                    _ => (),
                },
                Mode::Insert => match input {
                    Key::Char('\n') | Key::Esc => self.set_mode(Mode::Normal)?,
                    Key::Backspace => with_focused!(ListView::backspace),
                    Key::Char(x) => with_focused!(ListView::append_to_current, x),
                    _ => (),
                },
            };
        }
        self.terminal.clear()?;
        persist_state(&self.todos, &self.times);
        Ok(())
    }

    fn set_mode(&mut self, mode: Mode) -> Result<(), io::Error> {
        match mode {
            Mode::Insert => self.terminal.show_cursor()?,
            Mode::Normal => {
                self.todos.normal_mode();
                self.times.normal_mode();
                self.terminal.hide_cursor()?;
            }
        };
        self.input_mode = mode;
        Ok(())
    }

    fn refresh(&mut self) -> Result<(), io::Error> {
        let summary_content = [Text::raw(format!(
            "Sum for today: {}\n{}",
            self.times.sum_as_str(),
            self.times.time_by_tasks()
        ))];
        let mut summary = Paragraph::new(summary_content.iter())
            .wrap(true)
            .block(Block::default().borders(Borders::ALL));
        let todos = self.todos.printable();
        let mut todolist = layout::selectable_list(
            " t r a c c ",
            &todos,
            Some(self.todos.selected).filter(|_| self.focus == Focus::Top),
        );
        let times = self.times.printable();
        let mut timelist = layout::selectable_list(
            " 🕑 ",
            &times,
            Some(self.times.selected).filter(|_| self.focus == Focus::Bottom),
        );

        self.terminal.draw(|mut frame| {
            let chunks = layout::layout(frame.size());
            todolist.render(&mut frame, chunks[0]);
            timelist.render(&mut frame, chunks[1]);
            summary.render(&mut frame, chunks[2]);
        })?;
        Ok(())
    }
}

fn persist_state(todos: &TodoList, times: &TimeSheet) {
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
    let todos = serde_json::to_string(&todos.todos).unwrap();
    write(JSON_PATH_TODO, todos);
    let times = serde_json::to_string(&times.times).unwrap();
    write(JSON_PATH_TIME, times);
}
