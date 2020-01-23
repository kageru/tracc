use super::Mode;
use std::io;
use tui::backend::Backend;
use tui::Terminal;

pub struct Tracc {
    // We use owned strings here because they’re easier to manipulate when editing.
    pub todos: Vec<Todo>,
    pub selected: Option<usize>,
    pub mode: Mode,
}

pub struct Todo {
    text: String,
    done: bool,
}

impl Todo {
    pub fn new(text: &str) -> Self {
        Todo {
            text: text.to_owned(),
            done: false,
        }
    }
}

impl Tracc {
    pub fn new() -> Self {
        Self {
            todos: vec![
                Todo::new("This is a list entry"),
                Todo::new("a second todo"),
                Todo::new("And a third"),
            ],
            selected: Some(0),
            mode: Mode::Normal,
        }
    }

    pub fn printable_todos(&self) -> Vec<String> {
        self.todos
            .iter()
            .map(|todo| format!("[{}] {}", if todo.done { 'x' } else { ' ' }, todo.text))
            .collect()
    }

    pub fn selection_down(&mut self) {
        self.selected = self.selected.map(|i| (i + 1).min(self.todos.len() - 1));
    }

    pub fn selection_up(&mut self) {
        self.selected = self.selected.map(|i| i.saturating_sub(1));
    }

    pub fn insert(&mut self) {
        self.todos.insert(self.selected.unwrap() + 1, Todo::new(""));
        self.selected = self.selected.map(|n| n + 1);
        self.mode = Mode::Normal;
    }

    pub fn remove_current(&mut self) {
        if let Some(n) = self.selected {
            self.todos.remove(n);
            self.selected = Some(n.min(self.todos.len() - 1));
        }
    }

    pub fn toggle_current(&mut self) {
        self.current().done = !self.current().done;
    }

    fn current(&mut self) -> &mut Todo {
        &mut self.todos[self.selected.unwrap()]
    }

    pub fn set_mode(
        &mut self,
        mode: Mode,
        term: &mut Terminal<impl Backend>,
    ) -> Result<(), io::Error> {
        match mode {
            Mode::Insert => term.show_cursor()?,
            Mode::Normal => {
                if self.current().text.is_empty() {
                    self.remove_current()
                }
                term.hide_cursor()? 
            },
        };
        self.mode = mode;
        Ok(())
    }

    pub fn append_to_current(&mut self, chr: char) {
        self.todos[self.selected.unwrap()].text.push(chr);
    }

    pub fn current_pop(&mut self) {
        self.todos[self.selected.unwrap()].text.pop();
    }
}
