/// GUI-local drafts only; persisted preferences remain Core-owned.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GuiPreferences {
    pub locale_draft: Option<String>,
    pub appearance_draft: Option<String>,
}
