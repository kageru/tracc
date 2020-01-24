use super::Mode;
use serde::{Deserialize, Serialize};
use serde_json::from_reader;
use std::fs::File;
use std::io::{self, BufReader, Write};
use tui::backend::Backend;
use tui::Terminal;

pub struct Tracc {
    // We use owned strings here because they’re easier to manipulate when editing.
    pub todos: Vec<Todo>,
    pub selected: Option<usize>,
    pub mode: Mode,
}

#[derive(Serialize, Deserialize)]
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

const JSON_PATH: &str = "tracc.json";

fn read_todos() -> Option<Vec<Todo>> {
    File::open(JSON_PATH)
        .ok()
        .map(|f| BufReader::new(f))
        .and_then(|r| from_reader(r).ok())
}

impl Tracc {
    pub fn open_or_create() -> Self {
        Self {
            todos: read_todos().unwrap_or(vec![Todo::new("This is a list entry")]),
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
            }
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

    pub fn persist(self) {
        let string = serde_json::to_string(&self.todos).unwrap();
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(JSON_PATH)
            .ok()
            .or_else(|| panic!("Can’t save todos to JSON. Dumping raw data:\n{}", string))
            .map(|mut f| f.write(string.as_bytes()));
    }
}
