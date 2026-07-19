use crate::app::ActionRecorder;

pub struct TranscriptModel {
    anchor: Option<String>,
    new_output_count: usize,
    recorder: ActionRecorder,
}

impl TranscriptModel {
    pub(crate) fn new(recorder: ActionRecorder) -> Self {
        Self {
            anchor: None,
            new_output_count: 0,
            recorder,
        }
    }

    pub fn anchor(&self) -> Option<&str> {
        self.anchor.as_deref()
    }

    pub fn new_output_count(&self) -> usize {
        self.new_output_count
    }

    pub fn open_history_at(&mut self, row_id: &str) {
        self.anchor = Some(row_id.to_owned());
        self.recorder.record(format!("history:{row_id}"));
    }

    pub fn append_new_output(&mut self, row_id: &str) {
        self.new_output_count += 1;
        self.recorder.record(format!("output:{row_id}"));
    }
}
