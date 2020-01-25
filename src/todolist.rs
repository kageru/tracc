use serde::{Deserialize, Serialize};
use serde_json::from_reader;
use std::fs::File;
use std::io::BufReader;

pub struct TodoList {
    // We use owned strings here because they’re easier to manipulate when editing.
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

    pub fn printable_todos(&self) -> Vec<String> {
        self.todos
            .iter()
            .map(|todo| format!("[{}] {}", if todo.done { 'x' } else { ' ' }, todo.text))
            .collect()
    }

    pub fn selection_down(&mut self) {
        self.selected = (self.selected + 1).min(self.todos.len().saturating_sub(1));
    }

    pub fn selection_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn insert(&mut self, todo: Todo) {
        if self.selected == self.todos.len().saturating_sub(1) {
            self.todos.push(todo);
            self.selected = self.todos.len() - 1;
        } else {
            self.todos.insert(self.selected + 1, todo);
            self.selected += 1;
        }
    }

    pub fn remove_current(&mut self) -> Option<Todo> {
        if self.todos.is_empty() {
            return None;
        }
        let index = self.selected;
        self.selected = index.min(self.todos.len().saturating_sub(2));
        return Some(self.todos.remove(index));
    }

    pub fn toggle_current(&mut self) {
        self.todos[self.selected].done = !self.todos[self.selected].done;
    }

    fn current(&self) -> &Todo {
        &self.todos[self.selected]
    }

    pub fn normal_mode(&mut self) {
        if self.current().text.is_empty() {
            self.remove_current();
            self.selected = self.selected.saturating_sub(1);
        }
    }

    pub fn append_to_current(&mut self, chr: char) {
        self.todos[self.selected].text.push(chr);
    }

    pub fn current_pop(&mut self) {
        self.todos[self.selected].text.pop();
    }
}
