use super::todolist::TodoList;
use super::timesheet::TimeSheet;
use std::default::Default;
use std::io::{self, Write};
use termion::event::Key;
use termion::input::TermRead;
use tui::backend::TermionBackend;
use tui::layout::*;
use tui::style::{Color, Style};
use tui::widgets::*;

type Terminal = tui::Terminal<TermionBackend<termion::raw::RawTerminal<io::Stdout>>>;
const JSON_PATH: &str = "tracc.json";

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
            todos: TodoList::open_or_create(JSON_PATH),
            times: TimeSheet::new(),
            terminal,
            input_mode: Mode::Normal,
            focus: Focus::Top,
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
                    Key::Char('j') => self.todos.selection_down(),
                    Key::Char('k') => self.todos.selection_up(),
                    Key::Char('o') => {
                        self.todos.insert(Default::default());
                        self.set_mode(Mode::Insert)?;
                    }
                    Key::Char('a') | Key::Char('A') => self.set_mode(Mode::Insert)?,
                    Key::Char(' ') => self.todos.toggle_current(),
                    // dd
                    Key::Char('d') => {
                        if let Key::Char('d') = inputs.next().unwrap()? {
                            self.todos.register = self.todos.remove_current()
                        }
                    }
                    Key::Char('p') => {
                        if self.todos.register.is_some() {
                            self.todos.insert(self.todos.register.clone().unwrap());
                        }
                    }
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
                    Key::Backspace => self.todos.current_pop(),
                    Key::Char(x) => self.todos.append_to_current(x),
                    _ => (),
                },
            };
        }
        self.terminal.clear()?;
        persist_todos(&self.todos, JSON_PATH);
        Ok(())
    }

    fn set_mode(&mut self, mode: Mode) -> Result<(), io::Error> {
        match mode {
            Mode::Insert => self.terminal.show_cursor()?,
            Mode::Normal => {
                self.todos.normal_mode();
                self.terminal.hide_cursor()?
            }
        };
        self.input_mode = mode;
        Ok(())
    }

    fn refresh(&mut self) -> Result<(), io::Error> {
        fn selectable_list<'a, C: AsRef<str>>(
            title: &'a str,
            content: &'a [C],
            selected: Option<usize>,
        ) -> SelectableList<'a> {
            SelectableList::default()
                .block(
                    Block::default()
                        .title(title)
                        .borders(Borders::TOP | Borders::RIGHT | Borders::LEFT),
                )
                .items(content)
                .select(selected.into())
                .highlight_style(Style::default().fg(Color::LightGreen))
                .highlight_symbol(">")
        }

        let printable_todos = self.todos.printable();
        let top_focus = Some(self.todos.selected).filter(|_| self.focus == Focus::Top);
        let printable_times = self.times.printable();
        let bottom_focus = Some(self.times.selected).filter(|_| self.focus == Focus::Bottom);

        self.terminal.draw(|mut frame| {
            let size = frame.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Percentage(42),
                        Constraint::Percentage(42),
                        Constraint::Percentage(16),
                    ]
                    .as_ref(),
                )
                .split(size);
            selectable_list(" t r a c c ", &printable_todos, top_focus)
                .render(&mut frame, chunks[0]);
            selectable_list(" 🕑 ", &printable_times, bottom_focus)
                .render(&mut frame, chunks[1]);
            Paragraph::new([Text::raw("Sum for today: 1:12")].iter())
                .block(Block::default().borders(Borders::ALL))
                .render(&mut frame, chunks[2]);
        })?;
        Ok(())
    }
}

fn persist_todos(todos: &TodoList, path: &str) {
    let string = serde_json::to_string(&todos.todos).unwrap();
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .ok()
        .or_else(|| panic!("Can’t save todos to JSON. Dumping raw data:\n{}", string))
        .map(|mut f| f.write(string.as_bytes()));
}
