use crate::app::ActionRecorder;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApprovalChoice {
    AllowOnce,
    Deny,
}

impl ApprovalChoice {
    fn as_action(self) -> &'static str {
        match self {
            Self::AllowOnce => "allow-once",
            Self::Deny => "deny",
        }
    }
}

pub struct ApprovalDockModel {
    last_choice: Option<ApprovalChoice>,
    recorder: ActionRecorder,
}

impl ApprovalDockModel {
    pub(crate) fn new(recorder: ActionRecorder) -> Self {
        Self {
            last_choice: None,
            recorder,
        }
    }

    pub fn last_choice(&self) -> Option<ApprovalChoice> {
        self.last_choice
    }

    pub fn respond(&mut self, choice: ApprovalChoice) {
        self.last_choice = Some(choice);
        self.recorder
            .record(format!("approval:{}", choice.as_action()));
    }
}
