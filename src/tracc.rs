use super::todolist::TodoList;
use std::default::Default;
use std::io;
use termion::event::Key;
use termion::input::TermRead;
use tui::backend::TermionBackend;
use tui::style::{Color, Style};
use tui::widgets::*;

type Terminal = tui::Terminal<TermionBackend<termion::raw::RawTerminal<io::Stdout>>>;

pub enum Mode {
    Insert,
    Normal,
}

pub struct Tracc {
    todos: TodoList,
    terminal: Terminal,
    input_mode: Mode,
}

impl Tracc {
    pub fn new(terminal: Terminal) -> Self {
        Self {
            todos: TodoList::open_or_create(),
            terminal,
            input_mode: Mode::Normal,
        }
    }

    pub fn run(&mut self) -> Result<(), io::Error> {
        let mut inputs = io::stdin().keys();
        loop {
            refresh(&mut self.terminal, &self.todos)?;
            // I need to find a better way to handle inputs. This is awful.
            let input = inputs.next().unwrap()?;
            match self.input_mode {
                Mode::Normal => match input {
                    Key::Char('q') => {
                        self.todos.persist();
                        self.terminal.clear()?;
                        break;
                    }
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
}

fn refresh(terminal: &mut Terminal, todos: &TodoList) -> Result<(), io::Error> {
    terminal.draw(|mut frame| {
        let size = frame.size();
        let block = Block::default().title(" t r a c c ").borders(Borders::ALL);
        SelectableList::default()
            .block(block)
            .items(&todos.printable_todos())
            .select(Some(todos.selected))
            .highlight_style(Style::default().fg(Color::LightGreen))
            .highlight_symbol(">")
            .render(&mut frame, size);
    })?;
    Ok(())
}
