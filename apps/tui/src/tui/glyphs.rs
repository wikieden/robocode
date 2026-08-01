#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Glyph {
    YourTurn,
    Run,
    Done,
    Skill,
    Wait,
    Gate,
    Fail,
    Warning,
}

impl Glyph {
    pub(super) const ALL: [Self; 8] = [
        Self::YourTurn,
        Self::Run,
        Self::Done,
        Self::Skill,
        Self::Wait,
        Self::Gate,
        Self::Fail,
        Self::Warning,
    ];

    pub(super) const fn unicode(self) -> &'static str {
        match self {
            Self::YourTurn => "◆",
            Self::Run => "▶",
            Self::Done => "✓",
            Self::Skill => "▣",
            Self::Wait => "◌",
            Self::Gate => "⏸",
            Self::Fail => "✗",
            Self::Warning => "⚠",
        }
    }

    pub(super) const fn ascii(self) -> &'static str {
        match self {
            Self::YourTurn => "!",
            Self::Run => ">",
            Self::Done => "+",
            Self::Skill => "#",
            Self::Wait => "o",
            Self::Gate => "=",
            Self::Fail => "x",
            Self::Warning => "!",
        }
    }

    pub(super) const fn render(self, unicode: bool) -> &'static str {
        if unicode {
            self.unicode()
        } else {
            self.ascii()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GlyphSet {
    pub(super) unicode: bool,
    pub(super) your_turn: &'static str,
    pub(super) run: &'static str,
    pub(super) done: &'static str,
    pub(super) skill: &'static str,
    pub(super) wait: &'static str,
    pub(super) gate: &'static str,
    pub(super) fail: &'static str,
    pub(super) warning: &'static str,
}

impl GlyphSet {
    pub(super) const fn new(unicode: bool) -> Self {
        Self {
            unicode,
            your_turn: Glyph::YourTurn.render(unicode),
            run: Glyph::Run.render(unicode),
            done: Glyph::Done.render(unicode),
            skill: Glyph::Skill.render(unicode),
            wait: Glyph::Wait.render(unicode),
            gate: Glyph::Gate.render(unicode),
            fail: Glyph::Fail.render(unicode),
            warning: Glyph::Warning.render(unicode),
        }
    }

    pub(super) fn activity_indicator(self, reduced_motion: bool, frame: usize) -> &'static str {
        const UNICODE_FRAMES: [&str; 4] = ["·", "∙", "•", "∙"];
        const ASCII_FRAMES: [&str; 4] = [".", "o", "O", "o"];
        if reduced_motion {
            self.run
        } else if self.unicode {
            UNICODE_FRAMES[frame % UNICODE_FRAMES.len()]
        } else {
            ASCII_FRAMES[frame % ASCII_FRAMES.len()]
        }
    }
}

pub(super) fn ascii_fallback(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for character in input.chars() {
        if let Some(glyph) = Glyph::ALL
            .iter()
            .find(|glyph| glyph.unicode().starts_with(character))
        {
            output.push_str(glyph.ascii());
            continue;
        }
        match character {
            character if character.is_ascii() => output.push(character),
            '─' | '━' | '┄' | '┈' => output.push('-'),
            '│' | '┃' | '┊' | '┆' => output.push('|'),
            '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' | '╭' | '╮' | '╰' | '╯' => {
                output.push('+')
            }
            '·' | '∙' | '•' | '◉' | '○' | '●' | '◐' | '◇' => output.push('.'),
            '✦' | '✣' | '⚙' => output.push('*'),
            '▓' | '█' | '▇' | '▆' | '▅' | '▄' | '▃' | '▂' | '▁' => {
                output.push('#')
            }
            '░' => output.push('.'),
            '→' | '›' => output.push('>'),
            '←' | '‹' => output.push('<'),
            '↑' => output.push('^'),
            '↓' => output.push('v'),
            '✕' => output.push('x'),
            '…' => output.push_str("..."),
            // Business text, paths, and Core facts are data rather than chrome.
            // Preserve unknown Unicode instead of corrupting localized content.
            _ => output.push(character),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{Glyph, GlyphSet};

    #[test]
    fn every_registered_glyph_has_a_single_cell_ascii_fallback() {
        for glyph in Glyph::ALL {
            let fallback = glyph.ascii();
            assert!(fallback.is_ascii(), "{glyph:?}: {fallback}");
            assert_eq!(fallback.chars().count(), 1, "{glyph:?}: {fallback}");
        }
    }

    #[test]
    fn registry_matches_the_design_status_vocabulary() {
        let unicode = GlyphSet::new(true);
        let ascii = GlyphSet::new(false);

        assert_eq!(unicode.your_turn, "◆");
        assert_eq!(unicode.run, "▶");
        assert_eq!(unicode.done, "✓");
        assert_eq!(unicode.skill, "▣");
        assert_eq!(unicode.wait, "◌");
        assert_eq!(unicode.gate, "⏸");
        assert_eq!(unicode.fail, "✗");
        assert_eq!(unicode.warning, "⚠");
        assert_eq!(ascii.run, ">");
        assert_eq!(ascii.done, "+");
        assert_eq!(ascii.fail, "x");
    }

    #[test]
    fn reduced_motion_uses_a_static_registered_indicator() {
        let glyphs = GlyphSet::new(true);

        assert_eq!(
            glyphs.activity_indicator(true, 0),
            glyphs.activity_indicator(true, 7)
        );
        assert_ne!(
            glyphs.activity_indicator(false, 0),
            glyphs.activity_indicator(false, 1)
        );
    }

    #[test]
    fn registered_unicode_glyphs_can_be_rewritten_for_ascii_terminals() {
        assert_eq!(super::ascii_fallback("◆ ▶ ✓ ▣ ◌ ⏸ ✗ ⚠"), "! > + # o = x !");
    }

    #[test]
    fn terminal_ascii_fallback_rewrites_chrome_but_preserves_business_text() {
        let rendered = super::ascii_fallback("┌─ TUI ┐ │ ┊ ✦ ◉ ● ◐ ◇ ✣ ⚙ ▓░ … → › ✕ 中文");

        assert!(rendered.contains("+- TUI +"));
        assert!(rendered.contains("..."));
        assert!(rendered.ends_with("中文"));
    }
}
