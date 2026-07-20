use crossterm::style::Color;

use super::preferences::{ColorDepth, ColorMode, Skin};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Rgb {
    pub(super) red: u8,
    pub(super) green: u8,
    pub(super) blue: u8,
}

impl Rgb {
    pub(super) const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Palette {
    pub(super) skin: Skin,
    pub(super) mode: ColorMode,
    pub(super) bg_void: Rgb,
    pub(super) bg_base: Rgb,
    pub(super) bg_panel: Rgb,
    pub(super) bg_topbar: Rgb,
    pub(super) bg_elev: Rgb,
    pub(super) bg_sel: Rgb,
    pub(super) fg_primary: Rgb,
    pub(super) fg_secondary: Rgb,
    pub(super) fg_muted: Rgb,
    pub(super) fg_faint: Rgb,
    pub(super) accent: Rgb,
    pub(super) accent_bright: Rgb,
    pub(super) accent_dim: Rgb,
    pub(super) on_accent: Rgb,
    pub(super) gold: Rgb,
    pub(super) gold_bright: Rgb,
    pub(super) success: Rgb,
    pub(super) warning: Rgb,
    pub(super) error: Rgb,
    pub(super) progress: Rgb,
    pub(super) builtin: Rgb,
    pub(super) border: Rgb,
    pub(super) border_soft: Rgb,
    pub(super) border_active: Rgb,
    pub(super) page_bg: Rgb,
    pub(super) page_card: Rgb,
    pub(super) page_line: Rgb,
    pub(super) page_ink: Rgb,
    pub(super) page_ink_dim: Rgb,
    pub(super) page_accent: Rgb,
}

include!(concat!(env!("OUT_DIR"), "/appearance_tokens.rs"));

impl Palette {
    pub(super) fn all() -> &'static [Self; 8] {
        &GENERATED_PALETTES
    }

    pub(super) fn find(skin: Skin, mode: ColorMode) -> Option<&'static Self> {
        Self::all()
            .iter()
            .find(|palette| palette.skin == skin && palette.mode == mode)
    }

    #[cfg(test)]
    pub(super) fn key(&self) -> (Skin, ColorMode) {
        (self.skin, self.mode)
    }

    #[cfg(test)]
    pub(super) fn semantic_roles(&self) -> [(&'static str, Rgb); 30] {
        [
            ("bg-void", self.bg_void),
            ("bg-base", self.bg_base),
            ("bg-panel", self.bg_panel),
            ("bg-topbar", self.bg_topbar),
            ("bg-elev", self.bg_elev),
            ("bg-sel", self.bg_sel),
            ("fg-primary", self.fg_primary),
            ("fg-secondary", self.fg_secondary),
            ("fg-muted", self.fg_muted),
            ("fg-faint", self.fg_faint),
            ("accent", self.accent),
            ("accent-bright", self.accent_bright),
            ("accent-dim", self.accent_dim),
            ("on-accent", self.on_accent),
            ("gold", self.gold),
            ("gold-bright", self.gold_bright),
            ("success", self.success),
            ("warning", self.warning),
            ("error", self.error),
            ("progress", self.progress),
            ("builtin", self.builtin),
            ("border", self.border),
            ("border-soft", self.border_soft),
            ("border-active", self.border_active),
            ("page-bg", self.page_bg),
            ("page-card", self.page_card),
            ("page-line", self.page_line),
            ("page-ink", self.page_ink),
            ("page-ink-dim", self.page_ink_dim),
            ("page-accent", self.page_accent),
        ]
    }

    pub(super) fn for_depth(&self, depth: ColorDepth) -> TerminalPalette {
        let map = |rgb| terminal_color(rgb, depth);
        TerminalPalette {
            background: map(self.bg_base),
            surface: map(self.bg_panel),
            overlay: map(self.bg_elev),
            chip: map(self.bg_sel),
            text: map(self.fg_primary),
            secondary: map(self.fg_secondary),
            muted: map(self.fg_muted),
            faint: map(self.fg_faint),
            frame: map(self.border),
            title: map(self.accent_bright),
            accent: map(self.accent),
            accent_dim: map(self.accent_dim),
            on_accent: map(self.on_accent),
            gold: map(self.gold),
            success: map(self.success),
            warning: map(self.warning),
            error: map(self.error),
            progress: map(self.progress),
            builtin: map(self.builtin),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TerminalPalette {
    pub(super) background: Color,
    pub(super) surface: Color,
    pub(super) overlay: Color,
    pub(super) chip: Color,
    pub(super) text: Color,
    pub(super) secondary: Color,
    pub(super) muted: Color,
    pub(super) faint: Color,
    pub(super) frame: Color,
    pub(super) title: Color,
    pub(super) accent: Color,
    pub(super) accent_dim: Color,
    pub(super) on_accent: Color,
    pub(super) gold: Color,
    pub(super) success: Color,
    pub(super) warning: Color,
    pub(super) error: Color,
    pub(super) progress: Color,
    pub(super) builtin: Color,
}

fn terminal_color(rgb: Rgb, depth: ColorDepth) -> Color {
    match depth {
        ColorDepth::Auto | ColorDepth::Truecolor => Color::Rgb {
            r: rgb.red,
            g: rgb.green,
            b: rgb.blue,
        },
        ColorDepth::Ansi256 => Color::AnsiValue(ansi256(rgb)),
        ColorDepth::Ansi16 => ansi16(rgb),
    }
}

fn ansi256(rgb: Rgb) -> u8 {
    let cube = |value: u8| ((u16::from(value) * 5 + 127) / 255) as u8;
    let red = cube(rgb.red);
    let green = cube(rgb.green);
    let blue = cube(rgb.blue);
    16 + 36 * red + 6 * green + blue
}

fn ansi16(rgb: Rgb) -> Color {
    let max = rgb.red.max(rgb.green).max(rgb.blue);
    let min = rgb.red.min(rgb.green).min(rgb.blue);
    if max < 48 {
        return Color::Black;
    }
    if max.saturating_sub(min) < 28 {
        return if max > 220 {
            Color::White
        } else if max > 128 {
            Color::Grey
        } else {
            Color::DarkGrey
        };
    }
    let bright = max > 170;
    let red = rgb.red > 96;
    let green = rgb.green > 96;
    let blue = rgb.blue > 96;
    match (red, green, blue, bright) {
        (true, true, false, true) => Color::Yellow,
        (true, true, false, false) => Color::DarkYellow,
        (true, false, true, true) => Color::Magenta,
        (true, false, true, false) => Color::DarkMagenta,
        (false, true, true, true) => Color::Cyan,
        (false, true, true, false) => Color::DarkCyan,
        (true, false, false, true) => Color::Red,
        (true, false, false, false) => Color::DarkRed,
        (false, true, false, true) => Color::Green,
        (false, true, false, false) => Color::DarkGreen,
        (false, false, true, true) => Color::Blue,
        (false, false, true, false) => Color::DarkBlue,
        _ => Color::Grey,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::style::Color;

    use super::{Palette, Rgb};
    use crate::tui::preferences::{ColorDepth, ColorMode, Skin};

    #[test]
    fn generated_source_contains_exactly_eight_complete_semantic_palettes() {
        let palettes = Palette::all();

        assert_eq!(palettes.len(), 8);
        for palette in palettes {
            assert_eq!(palette.semantic_roles().len(), 30, "{:?}", palette.key());
        }
        assert_eq!(
            palettes
                .iter()
                .map(|palette| palette.key())
                .collect::<Vec<_>>(),
            vec![
                (Skin::Aurora, ColorMode::Dark),
                (Skin::Aurora, ColorMode::Light),
                (Skin::Ice, ColorMode::Dark),
                (Skin::Ice, ColorMode::Light),
                (Skin::Mono, ColorMode::Dark),
                (Skin::Mono, ColorMode::Light),
                (Skin::Amber, ColorMode::Dark),
                (Skin::Phosphor, ColorMode::Dark),
            ]
        );
    }

    #[test]
    fn generated_values_come_from_the_registered_css_blocks() {
        let aurora_dark = Palette::find(Skin::Aurora, ColorMode::Dark).unwrap();
        let ice_light = Palette::find(Skin::Ice, ColorMode::Light).unwrap();
        let phosphor_dark = Palette::find(Skin::Phosphor, ColorMode::Dark).unwrap();

        assert_eq!(aurora_dark.bg_void, Rgb::new(0x05, 0x09, 0x0f));
        assert_eq!(aurora_dark.accent, Rgb::new(0x34, 0xbd, 0xd9));
        assert_eq!(ice_light.bg_base, Rgb::new(0xf3, 0xf7, 0xfc));
        assert_eq!(ice_light.accent, Rgb::new(0x2d, 0x65, 0xd2));
        assert_eq!(phosphor_dark.error, Rgb::new(0xe0, 0x73, 0x4f));
    }

    #[test]
    fn color_depth_mapping_never_leaks_rgb_below_truecolor() {
        let palette = Palette::find(Skin::Ice, ColorMode::Light).unwrap();

        let truecolor = palette.for_depth(ColorDepth::Truecolor);
        let ansi256 = palette.for_depth(ColorDepth::Ansi256);
        let ansi16 = palette.for_depth(ColorDepth::Ansi16);

        assert!(matches!(truecolor.accent, Color::Rgb { .. }));
        for mapped in [ansi256, ansi16] {
            assert!(!matches!(mapped.background, Color::Rgb { .. }));
            assert!(!matches!(mapped.text, Color::Rgb { .. }));
            assert!(!matches!(mapped.accent, Color::Rgb { .. }));
            assert!(!matches!(mapped.warning, Color::Rgb { .. }));
        }
        assert_ne!(ansi256.accent, ansi16.accent);
    }

    #[test]
    fn all_eight_palettes_map_across_truecolor_ansi256_and_ansi16() {
        for palette in Palette::all() {
            for depth in [
                ColorDepth::Truecolor,
                ColorDepth::Ansi256,
                ColorDepth::Ansi16,
            ] {
                let mapped = palette.for_depth(depth);
                let colors = [
                    mapped.background,
                    mapped.surface,
                    mapped.overlay,
                    mapped.chip,
                    mapped.text,
                    mapped.secondary,
                    mapped.muted,
                    mapped.faint,
                    mapped.frame,
                    mapped.title,
                    mapped.accent,
                    mapped.accent_dim,
                    mapped.on_accent,
                    mapped.gold,
                    mapped.success,
                    mapped.warning,
                    mapped.error,
                    mapped.progress,
                    mapped.builtin,
                ];

                assert_eq!(
                    colors
                        .iter()
                        .filter(|color| matches!(color, Color::Rgb { .. }))
                        .count(),
                    usize::from(depth == ColorDepth::Truecolor) * colors.len(),
                    "palette {:?} at {depth:?}",
                    palette.key()
                );
            }
        }
    }
}
