use crossterm::style::Color;
use viden_core::{ResolvedUiPreferences, TuiColorDepth, UiColorMode, UiSkin};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiTheme {
    pub(super) name: &'static str,
    pub(super) background: Color,
    pub(super) surface: Color,
    pub(super) overlay: Color,
    pub(super) chip: Color,
    pub(super) text: Color,
    pub(super) frame: Color,
    pub(super) title: Color,
    pub(super) accent: Color,
    pub(super) success: Color,
    pub(super) warning: Color,
    pub(super) error: Color,
    pub(super) muted: Color,
}

impl TuiTheme {
    pub(super) fn aurora() -> Self {
        Self {
            name: "aurora",
            background: Color::Rgb { r: 3, g: 12, b: 22 },
            surface: Color::Rgb { r: 5, g: 19, b: 35 },
            overlay: Color::Rgb {
                r: 11,
                g: 22,
                b: 43,
            },
            chip: Color::Rgb { r: 8, g: 35, b: 58 },
            text: Color::Rgb {
                r: 196,
                g: 224,
                b: 255,
            },
            frame: Color::Rgb {
                r: 28,
                g: 95,
                b: 132,
            },
            title: Color::Rgb {
                r: 25,
                g: 202,
                b: 255,
            },
            accent: Color::Rgb {
                r: 37,
                g: 166,
                b: 255,
            },
            success: Color::Rgb {
                r: 71,
                g: 214,
                b: 128,
            },
            warning: Color::Rgb {
                r: 241,
                g: 196,
                b: 15,
            },
            error: Color::Rgb {
                r: 255,
                g: 77,
                b: 109,
            },
            muted: Color::Rgb {
                r: 119,
                g: 151,
                b: 184,
            },
        }
    }

    #[cfg(test)]
    pub(super) fn aurora_cyan() -> Self {
        Self::aurora()
    }

    pub(super) fn amber() -> Self {
        Self {
            name: "amber",
            background: Color::Rgb { r: 18, g: 11, b: 7 },
            surface: Color::Rgb {
                r: 28,
                g: 17,
                b: 10,
            },
            overlay: Color::Rgb {
                r: 37,
                g: 24,
                b: 12,
            },
            chip: Color::Rgb {
                r: 50,
                g: 31,
                b: 14,
            },
            accent: Color::Rgb {
                r: 255,
                g: 176,
                b: 64,
            },
            title: Color::Rgb {
                r: 255,
                g: 207,
                b: 92,
            },
            frame: Color::Rgb {
                r: 140,
                g: 82,
                b: 38,
            },
            ..Self::aurora()
        }
    }

    pub(super) fn phosphor() -> Self {
        Self {
            name: "phosphor",
            background: Color::Rgb { r: 2, g: 12, b: 6 },
            surface: Color::Rgb { r: 4, g: 24, b: 11 },
            overlay: Color::Rgb { r: 7, g: 34, b: 16 },
            chip: Color::Rgb { r: 8, g: 43, b: 19 },
            accent: Color::Rgb {
                r: 94,
                g: 255,
                b: 135,
            },
            title: Color::Rgb {
                r: 153,
                g: 255,
                b: 177,
            },
            frame: Color::Rgb {
                r: 38,
                g: 139,
                b: 71,
            },
            ..Self::aurora()
        }
    }

    pub(super) fn ice() -> Self {
        Self {
            name: "ice",
            background: Color::Rgb { r: 7, g: 11, b: 16 },
            surface: Color::Rgb {
                r: 13,
                g: 18,
                b: 25,
            },
            overlay: Color::Rgb {
                r: 20,
                g: 26,
                b: 34,
            },
            chip: Color::Rgb {
                r: 29,
                g: 38,
                b: 48,
            },
            accent: Color::Rgb {
                r: 188,
                g: 211,
                b: 232,
            },
            title: Color::Rgb {
                r: 230,
                g: 239,
                b: 247,
            },
            frame: Color::Rgb {
                r: 100,
                g: 119,
                b: 136,
            },
            ..Self::aurora()
        }
    }

    pub(super) fn mono() -> Self {
        Self {
            name: "mono",
            background: Color::Rgb {
                r: 10,
                g: 10,
                b: 10,
            },
            surface: Color::Rgb {
                r: 18,
                g: 18,
                b: 18,
            },
            overlay: Color::Rgb {
                r: 26,
                g: 26,
                b: 26,
            },
            chip: Color::Rgb {
                r: 34,
                g: 34,
                b: 34,
            },
            text: Color::Rgb {
                r: 224,
                g: 224,
                b: 224,
            },
            frame: Color::Rgb {
                r: 118,
                g: 118,
                b: 118,
            },
            title: Color::White,
            accent: Color::Grey,
            ..Self::aurora()
        }
    }

    pub(super) fn builtins() -> [Self; 5] {
        [
            Self::aurora(),
            Self::ice(),
            Self::mono(),
            Self::amber(),
            Self::phosphor(),
        ]
    }

    pub(super) fn names() -> &'static [&'static str] {
        &["aurora", "ice", "mono", "amber", "phosphor"]
    }

    pub(super) fn named(name: &str) -> Self {
        let normalized = name.to_ascii_lowercase();
        let name = match normalized.as_str() {
            "aurora-cyan" => "aurora",
            "monochrome-ice" => "ice",
            "ember-gold" => "amber",
            "plasma-violet" => "phosphor",
            _ => normalized.as_str(),
        };
        Self::builtins()
            .into_iter()
            .find(|theme| theme.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(Self::aurora)
    }

    pub(super) fn is_known(name: &str) -> bool {
        Self::names()
            .iter()
            .any(|theme_name| theme_name.eq_ignore_ascii_case(name))
    }

    pub(super) fn from_name_or_env(name: Option<&str>) -> Self {
        name.map(Self::named).unwrap_or_else(Self::from_env)
    }

    pub(super) fn from_preferences(preferences: &ResolvedUiPreferences) -> Self {
        let mut theme = match preferences.skin {
            UiSkin::Aurora => Self::aurora(),
            UiSkin::Ice => Self::ice(),
            UiSkin::Mono => Self::mono(),
            UiSkin::Amber => Self::amber(),
            UiSkin::Phosphor => Self::phosphor(),
        };
        if preferences.mode == UiColorMode::Light {
            theme.background = Color::Rgb {
                r: 244,
                g: 248,
                b: 252,
            };
            theme.surface = Color::Rgb {
                r: 232,
                g: 239,
                b: 246,
            };
            theme.overlay = Color::Rgb {
                r: 222,
                g: 232,
                b: 241,
            };
            theme.chip = Color::Rgb {
                r: 211,
                g: 225,
                b: 237,
            };
            theme.text = Color::Rgb {
                r: 18,
                g: 36,
                b: 52,
            };
            theme.muted = Color::Rgb {
                r: 72,
                g: 93,
                b: 112,
            };
        }
        theme
    }

    pub(super) fn with_color_depth(mut self, depth: TuiColorDepth) -> Self {
        match depth {
            TuiColorDepth::Truecolor => self,
            TuiColorDepth::Ansi256 => {
                self.background = Color::AnsiValue(233);
                self.surface = Color::AnsiValue(234);
                self.overlay = Color::AnsiValue(235);
                self.chip = Color::AnsiValue(236);
                self.text = Color::AnsiValue(252);
                self.frame = Color::AnsiValue(67);
                self.title = Color::AnsiValue(81);
                self.accent = Color::AnsiValue(75);
                self.success = Color::AnsiValue(78);
                self.warning = Color::AnsiValue(220);
                self.error = Color::AnsiValue(204);
                self.muted = Color::AnsiValue(245);
                self
            }
            TuiColorDepth::Ansi16 => {
                self.background = Color::Black;
                self.surface = Color::Black;
                self.overlay = Color::DarkGrey;
                self.chip = Color::DarkGrey;
                self.text = Color::White;
                self.frame = Color::DarkCyan;
                self.title = Color::Cyan;
                self.accent = Color::Cyan;
                self.success = Color::Green;
                self.warning = Color::Yellow;
                self.error = Color::Red;
                self.muted = Color::Grey;
                self
            }
        }
    }

    pub(super) fn from_env() -> Self {
        std::env::var("VIDEN_TUI_THEME")
            .ok()
            .map(|name| Self::named(&name))
            .unwrap_or_else(Self::aurora)
    }

    pub(super) fn next(&self) -> Self {
        let themes = Self::builtins();
        let index = themes
            .iter()
            .position(|theme| theme.name == self.name)
            .unwrap_or(0);
        themes[(index + 1) % themes.len()].clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_themes_match_core_skin_ids() {
        let names: Vec<_> = TuiTheme::builtins()
            .into_iter()
            .map(|theme| theme.name)
            .collect();

        assert_eq!(names, vec!["aurora", "ice", "mono", "amber", "phosphor"]);
        assert_eq!(TuiTheme::named("unknown").name, "aurora");
        assert_eq!(TuiTheme::named("aurora").next().name, "ice");
    }

    #[test]
    fn core_color_mode_changes_the_effective_palette() {
        let dark = TuiTheme::from_preferences(&viden_core::ResolvedUiPreferences {
            locale: viden_core::LocaleId::En,
            skin: viden_core::UiSkin::Ice,
            mode: viden_core::UiColorMode::Dark,
            density: viden_core::UiDensity::Regular,
            motion: viden_core::UiMotion::System,
            diagnostics: Vec::new(),
        });
        let light = TuiTheme::from_preferences(&viden_core::ResolvedUiPreferences {
            mode: viden_core::UiColorMode::Light,
            ..viden_core::ResolvedUiPreferences {
                locale: viden_core::LocaleId::En,
                skin: viden_core::UiSkin::Ice,
                mode: viden_core::UiColorMode::Dark,
                density: viden_core::UiDensity::Regular,
                motion: viden_core::UiMotion::System,
                diagnostics: Vec::new(),
            }
        });

        assert_ne!(dark.background, light.background);
        assert_eq!(dark.name, "ice");
    }

    #[test]
    fn core_color_depth_selects_a_non_rgb_terminal_palette() {
        let theme = TuiTheme::aurora().with_color_depth(viden_core::TuiColorDepth::Ansi16);

        assert!(!matches!(theme.background, Color::Rgb { .. }));
        assert!(!matches!(theme.accent, Color::Rgb { .. }));
    }
}
