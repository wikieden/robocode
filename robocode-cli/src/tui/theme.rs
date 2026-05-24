use crossterm::style::Color;

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
    pub(super) fn aurora_cyan() -> Self {
        Self {
            name: "aurora-cyan",
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

    pub(super) fn ember_gold() -> Self {
        Self {
            name: "ember-gold",
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
            ..Self::aurora_cyan()
        }
    }

    pub(super) fn plasma_violet() -> Self {
        Self {
            name: "plasma-violet",
            background: Color::Rgb { r: 10, g: 7, b: 23 },
            surface: Color::Rgb {
                r: 17,
                g: 11,
                b: 38,
            },
            overlay: Color::Rgb {
                r: 27,
                g: 18,
                b: 54,
            },
            chip: Color::Rgb {
                r: 35,
                g: 22,
                b: 68,
            },
            accent: Color::Rgb {
                r: 177,
                g: 114,
                b: 255,
            },
            title: Color::Rgb {
                r: 217,
                g: 154,
                b: 255,
            },
            frame: Color::Rgb {
                r: 90,
                g: 63,
                b: 150,
            },
            ..Self::aurora_cyan()
        }
    }

    pub(super) fn monochrome_ice() -> Self {
        Self {
            name: "monochrome-ice",
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
            ..Self::aurora_cyan()
        }
    }

    pub(super) fn builtins() -> [Self; 4] {
        [
            Self::aurora_cyan(),
            Self::ember_gold(),
            Self::plasma_violet(),
            Self::monochrome_ice(),
        ]
    }

    pub(super) fn names() -> &'static [&'static str] {
        &[
            "aurora-cyan",
            "ember-gold",
            "plasma-violet",
            "monochrome-ice",
        ]
    }

    pub(super) fn named(name: &str) -> Self {
        Self::builtins()
            .into_iter()
            .find(|theme| theme.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(Self::aurora_cyan)
    }

    pub(super) fn is_known(name: &str) -> bool {
        Self::names()
            .iter()
            .any(|theme_name| theme_name.eq_ignore_ascii_case(name))
    }

    pub(super) fn from_name_or_env(name: Option<&str>) -> Self {
        name.map(Self::named).unwrap_or_else(Self::from_env)
    }

    pub(super) fn from_env() -> Self {
        std::env::var("ROBOCODE_TUI_THEME")
            .ok()
            .map(|name| Self::named(&name))
            .unwrap_or_else(Self::aurora_cyan)
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
    fn built_in_themes_cover_expected_visual_variants() {
        let names: Vec<_> = TuiTheme::builtins()
            .into_iter()
            .map(|theme| theme.name)
            .collect();

        assert_eq!(
            names,
            vec![
                "aurora-cyan",
                "ember-gold",
                "plasma-violet",
                "monochrome-ice"
            ]
        );
        assert_eq!(TuiTheme::named("unknown").name, "aurora-cyan");
        assert_eq!(TuiTheme::named("aurora-cyan").next().name, "ember-gold");
    }
}
