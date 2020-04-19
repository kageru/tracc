use super::timesheet::TimeSheet;
use super::todolist::TodoList;
use std::default::Default;
use std::fmt;
use std::io::{self, Write};
use termion::event::Key;
use termion::input::TermRead;
use tui::backend::TermionBackend;
use tui::layout::*;
use tui::style::{Color, Style};
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
                    Key::Char(' ') => with_focused!(ListView::toggle_current),
                    Key::Char('d') => with_focused!(ListView::remove_current),
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
                .select(selected)
                .highlight_style(Style::default().fg(Color::LightGreen))
                .highlight_symbol(">")
        }

        let printable_todos = self.todos.printable();
        let top_focus = Some(self.todos.selected).filter(|_| self.focus == Focus::Top);
        let printable_times = self.times.printable();
        let bottom_focus = Some(self.times.selected).filter(|_| self.focus == Focus::Bottom);
        let total_time = self.times.sum_as_str();
        let times = self.times.time_by_tasks();

        self.terminal.draw(|mut frame| {
            let size = frame.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Percentage(40),
                        Constraint::Percentage(40),
                        Constraint::Percentage(20),
                    ]
                    .as_ref(),
                )
                .split(size);
            selectable_list(" t r a c c ", &printable_todos, top_focus)
                .render(&mut frame, chunks[0]);
            selectable_list(" 🕑 ", &printable_times, bottom_focus).render(&mut frame, chunks[1]);
            Paragraph::new(
                [
                    Text::raw(format!("Sum for today: {}\n", total_time)),
                    Text::raw(times),
                ]
                .iter(),
            )
            .wrap(true)
            .block(Block::default().borders(Borders::ALL))
            .render(&mut frame, chunks[2]);
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

pub trait ListView<T: fmt::Display + Clone> {
    // get properties of implementations
    fn selection_pointer(&mut self) -> &mut usize;
    fn list(&mut self) -> &mut Vec<T>;
    fn register(&mut self) -> &mut Option<T>;

    // specific input handling
    fn backspace(&mut self);
    fn append_to_current(&mut self, chr: char);
    fn normal_mode(&mut self);
    fn toggle_current(&mut self);

    // selection manipulation
    fn selection_up(&mut self) {
        *self.selection_pointer() = self.selection_pointer().saturating_sub(1);
    }

    fn selection_down(&mut self) {
        *self.selection_pointer() =
            (*self.selection_pointer() + 1).min(self.list().len().saturating_sub(1));
    }

    // adding/removing elements
    fn insert(&mut self, item: T, position: Option<usize>) {
        let pos = position.unwrap_or(*self.selection_pointer());
        if pos == self.list().len().saturating_sub(1) {
            self.list().push(item);
            *self.selection_pointer() = self.list().len() - 1;
        } else {
            self.list().insert(pos + 1, item);
            *self.selection_pointer() = pos + 1;
        }
    }

    fn remove_current(&mut self) {
        if self.list().is_empty() {
            return;
        }
        let index = *self.selection_pointer();
        *self.selection_pointer() = index.min(self.list().len().saturating_sub(2));
        *self.register() = self.list().remove(index).into();
    }

    fn paste(&mut self) {
        if let Some(item) = self.register().clone() {
            self.insert(item, None);
        }
    }

    // printing
    fn printable(&mut self) -> Vec<String> {
        self.list().iter().map(T::to_string).collect()
    }
}
