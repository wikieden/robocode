use crate::{LocaleId, UiColorMode, UiDensity, UiMotion, UiSkin};

/// Partial personal UI preference update sent by a frontend.
///
/// Every field is a closed enum so serialized commands cannot carry free-form
/// theme names or secrets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UiPreferencePatch {
    pub locale: Option<LocaleId>,
    pub skin: Option<UiSkin>,
    pub mode: Option<UiColorMode>,
    pub density: Option<UiDensity>,
    pub motion: Option<UiMotion>,
}
