use crate::app::ActionRecorder;

pub struct ComposerModel {
    composing: bool,
    composition: String,
    value: String,
    recorder: ActionRecorder,
}

impl ComposerModel {
    pub(crate) fn new(recorder: ActionRecorder) -> Self {
        Self {
            composing: false,
            composition: String::new(),
            value: String::new(),
            recorder,
        }
    }

    pub fn draft(&self) -> &str {
        &self.value
    }

    pub fn begin_composition(&mut self) {
        self.composing = true;
        self.composition.clear();
        self.recorder.record("composition:start");
    }

    pub fn update_composition(&mut self, value: &str) {
        value.clone_into(&mut self.composition);
        self.recorder.record(format!("composition:update:{value}"));
    }

    pub fn commit_composition(&mut self) {
        self.value.push_str(&self.composition);
        self.recorder
            .record(format!("composition:commit:{}", self.composition));
        self.composition.clear();
        self.composing = false;
    }

    pub fn paste(&mut self, value: &str) {
        self.value.push_str(value);
        self.recorder
            .record(format!("paste:{}", value.replace('\n', "\\n")));
    }

    pub fn sync_from_framework(&mut self, value: &str) {
        value.clone_into(&mut self.value);
    }

    pub fn submit(&mut self) -> bool {
        if self.composing {
            return false;
        }
        self.recorder
            .record(format!("submit:{}", self.value.replace('\n', "\\n")));
        true
    }
}
