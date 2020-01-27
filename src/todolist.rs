use serde::{Deserialize, Serialize};
use serde_json::from_reader;
use std::fs::File;
use std::fmt;
use std::io::BufReader;
use crate::tracc::ListView;

pub struct TodoList {
    pub todos: Vec<Todo>,
    pub selected: usize,
    pub register: Option<Todo>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Todo {
    // We use owned strings here because they’re easier to manipulate when editing.
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
    File::open(path)
        .ok()
        .map(|f| BufReader::new(f))
        .and_then(|r| from_reader(r).ok())
}

impl TodoList {
    pub fn open_or_create(path: &str) -> Self {
        TodoList {
            todos: read_todos(path).unwrap_or(vec![Todo::new("This is a list entry")]),
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
    fn selection_down(&mut self) {
        self.selected = (self.selected + 1).min(self.todos.len().saturating_sub(1));
    }

    fn selection_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn insert<P>(&mut self, todo: Todo, position: P) where P: Into<Option<usize>> {
        let pos = position.into().unwrap_or(self.selected);
        if pos == self.todos.len().saturating_sub(1) {
            self.todos.push(todo);
            self.selected = self.todos.len() - 1;
        } else {
            self.todos.insert(pos + 1, todo);
            self.selected = pos + 1;
        }
    }

    fn remove_current(&mut self) {
        if self.todos.is_empty() {
            return;
        }
        let index = self.selected;
        self.selected = index.min(self.todos.len().saturating_sub(2));
        self.register = self.todos.remove(index).into();
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

    fn printable(&self) -> Vec<String> {
        self.todos.iter().map(Todo::to_string).collect()
    }

    fn paste(&mut self) {
        if self.register.is_some() {
            // Is there a better way?
            self.insert(self.register.as_ref().unwrap().clone(), None);
        }
    }
}
