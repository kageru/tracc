use super::todolist::TodoList;
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

pub struct Tracc {
    todos: TodoList,
    terminal: Terminal,
    input_mode: Mode,
    top_panel_selected: bool,
}

impl Tracc {
    pub fn new(terminal: Terminal) -> Self {
        Self {
            todos: TodoList::open_or_create(JSON_PATH),
            terminal,
            input_mode: Mode::Normal,
            top_panel_selected: true,
        }
    }

    pub fn run(&mut self) -> Result<(), io::Error> {
        let mut inputs = io::stdin().keys();
        loop {
            refresh(&mut self.terminal, &self.todos, self.top_panel_selected)?;
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
                    Key::Char('\t') => self.top_panel_selected = !self.top_panel_selected,
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
}

fn refresh(terminal: &mut Terminal, todos: &TodoList, top_selected: bool) -> Result<(), io::Error> {
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

    terminal.draw(|mut frame| {
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
        selectable_list(
            " t r a c c ",
            &todos.printable(),
            Some(todos.selected).filter(|_| top_selected),
        )
        .render(&mut frame, chunks[0]);
        selectable_list(
            " 🕑 ",
            &["[08:23] start", "[09:35] end"],
            Some(0).filter(|_| !top_selected),
        )
        .render(&mut frame, chunks[1]);
        Paragraph::new([Text::raw("Sum for today: 1:12")].iter())
            .block(Block::default().borders(Borders::ALL))
            .render(&mut frame, chunks[2]);
    })?;
    Ok(())
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
