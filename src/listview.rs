use std::fmt;

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
