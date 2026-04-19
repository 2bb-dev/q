use qcli_core::{Prompt, Queue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Queue,
    Composer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Char(char),
    Enter,
    Backspace,
    Tab,
    Up,
    Down,
    ShiftUp,
    ShiftDown,
    CtrlS,
    CtrlU,
    Esc,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    CopyToClipboard(String),
    Persist,
    Quit,
    Status(String),
}

pub struct App {
    pub queue: Queue,
    pub focus: Pane,
    pub selected: Option<usize>,
    pub composer: String,
    pub status: String,
}

impl App {
    pub fn new(queue: Queue) -> Self {
        let selected = if queue.is_empty() { None } else { Some(0) };
        Self {
            queue,
            focus: Pane::Queue,
            selected,
            composer: String::new(),
            status: String::new(),
        }
    }

    pub fn visible_prompts(&self) -> Vec<&Prompt> {
        self.queue.iter_pinned().chain(self.queue.iter_unpinned()).collect()
    }

    pub fn selected_prompt(&self) -> Option<&Prompt> {
        self.selected.and_then(|i| self.visible_prompts().into_iter().nth(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_app_has_queue_focus_and_selects_first_prompt() {
        let mut q = Queue::new();
        q.add_text("hello").unwrap();
        let app = App::new(q);
        assert_eq!(app.focus, Pane::Queue);
        assert_eq!(app.selected, Some(0));
        assert_eq!(app.composer, "");
    }

    #[test]
    fn empty_queue_has_no_selection() {
        let app = App::new(Queue::new());
        assert_eq!(app.selected, None);
    }
}
