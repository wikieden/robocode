use crate::app::ActionRecorder;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedTokens {
    pub bg_base: u32,
    pub fg_primary: u32,
    pub accent: u32,
    pub gold: u32,
}

include!(concat!(env!("OUT_DIR"), "/gpui_tokens.rs"));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Skin {
    AuroraDark,
    IceLight,
}

impl Skin {
    pub fn tokens(self) -> GeneratedTokens {
        match self {
            Self::AuroraDark => AURORA_DARK_TOKENS,
            Self::IceLight => ICE_LIGHT_TOKENS,
        }
    }

    fn as_action(self) -> &'static str {
        match self {
            Self::AuroraDark => "aurora-dark",
            Self::IceLight => "ice-light",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Density {
    Compact,
    Regular,
    Comfy,
}

impl Density {
    fn as_action(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Regular => "regular",
            Self::Comfy => "comfy",
        }
    }
}

pub struct ThemeModel {
    skin: Skin,
    density: Density,
    recorder: ActionRecorder,
}

impl ThemeModel {
    pub(crate) fn new(recorder: ActionRecorder) -> Self {
        Self {
            skin: Skin::AuroraDark,
            density: Density::Regular,
            recorder,
        }
    }

    pub fn skin(&self) -> Skin {
        self.skin
    }

    pub fn density(&self) -> Density {
        self.density
    }

    pub fn select(&mut self, skin: Skin, density: Density) {
        self.skin = skin;
        self.density = density;
        self.recorder.record(format!(
            "theme:{}:{}",
            skin.as_action(),
            density.as_action()
        ));
    }
}
