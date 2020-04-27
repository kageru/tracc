use crate::listview::ListView;
use serde::{Deserialize, Serialize};
use serde_json::from_reader;
use std::fmt;
use std::fs;
use std::io;

pub struct TodoList {
    pub todos: Vec<Todo>,
    pub selected: usize,
    pub register: Option<Todo>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
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

impl fmt::Display for Todo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}] {}", if self.done { 'x' } else { ' ' }, self.text)
    }
}

fn read_todos(path: &str) -> Option<Vec<Todo>> {
    fs::File::open(path)
        .ok()
        .map(io::BufReader::new)
        .and_then(|r| from_reader(r).ok())
}

impl TodoList {
    pub fn open_or_create(path: &str) -> Self {
        Self {
            todos: read_todos(path).unwrap_or_else(|| vec![Todo::new("This is a list entry")]),
            selected: 0,
            register: None,
        }
    }

    pub fn toggle_current(&mut self) {
        self.todos[self.selected].done = !self.todos[self.selected].done;
    }

    fn current(&self) -> &Todo {
        &self.todos[self.selected]
    }
}

impl ListView<Todo> for TodoList {
    fn selection_pointer(&mut self) -> &mut usize {
        &mut self.selected
    }

    fn list(&mut self) -> &mut Vec<Todo> {
        &mut self.todos
    }

    fn register(&mut self) -> &mut Option<Todo> {
        &mut self.register
    }

    fn normal_mode(&mut self) {
        if self.current().text.is_empty() {
            self.remove_current();
            self.selected = self.selected.saturating_sub(1);
        }
    }

    fn append_to_current(&mut self, chr: char) {
        self.todos[self.selected].text.push(chr);
    }

    fn backspace(&mut self) {
        self.todos[self.selected].text.pop();
    }
}
