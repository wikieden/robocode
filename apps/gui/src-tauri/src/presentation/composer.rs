#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ComposerDraft {
    pub text: String,
    pub is_composing: bool,
    undo: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComposerAction {
    None,
    Submit(String),
}

impl ComposerDraft {
    pub fn replace_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if text == self.text {
            return;
        }
        self.undo.push(std::mem::take(&mut self.text));
        self.text = text;
    }

    pub fn begin_composition(&mut self) {
        self.is_composing = true;
    }

    pub fn end_composition(&mut self) {
        self.is_composing = false;
    }

    pub fn handle_enter(&mut self, multiline: bool) -> ComposerAction {
        if self.is_composing {
            return ComposerAction::None;
        }
        if multiline {
            self.undo.push(self.text.clone());
            self.text.push('\n');
            return ComposerAction::None;
        }
        if self.text.trim().is_empty() {
            return ComposerAction::None;
        }
        let content = std::mem::take(&mut self.text);
        self.undo.clear();
        ComposerAction::Submit(content)
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.text = previous;
        true
    }
}
