use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const HORIZONTAL: char = '─';

#[allow(dead_code)]
pub(super) fn top_border(width: usize) -> String {
    full_border(width, '┌', '┐')
}

pub(super) fn bottom_border(width: usize) -> String {
    full_border(width, '└', '┘')
}

pub(super) fn horizontal(width: usize) -> String {
    HORIZONTAL.to_string().repeat(width)
}

fn full_border(width: usize, left: char, right: char) -> String {
    format!("{left}{}{right}", horizontal(width.saturating_sub(2)))
}

pub(super) fn char_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

pub(super) fn pad(value: &str, width: usize) -> String {
    let value = truncate(value, width);
    let padding = width.saturating_sub(char_width(&value));
    format!("{value}{}", " ".repeat(padding))
}

pub(super) fn truncate(value: &str, width: usize) -> String {
    let mut used = 0usize;
    let mut output = String::new();
    for value in value.graphemes(true) {
        let next = UnicodeWidthStr::width(value);
        if used + next > width {
            break;
        }
        output.push_str(value);
        used += next;
    }
    output
}

#[allow(dead_code)]
pub(super) fn compact_middle(value: &str, width: usize) -> String {
    if char_width(value) <= width {
        return value.to_string();
    }
    if width <= 3 {
        return truncate(value, width);
    }
    let head = (width - 1) / 2;
    let tail = width - head - 1;
    let prefix = truncate(value, head);
    let suffix = suffix_by_width(value, tail);
    format!("{prefix}~{suffix}")
}

pub(super) fn wrap_words(content: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    if char_width(content) <= width {
        return vec![content.to_string()];
    }

    let mut rows = Vec::new();
    let mut current = String::new();
    for word in content.split_whitespace() {
        if char_width(word) > width {
            if !current.is_empty() {
                rows.push(current);
                current = String::new();
            }
            push_wrapped_token(&mut rows, &mut current, word, width);
            continue;
        }

        let pending_width = if current.is_empty() {
            char_width(word)
        } else {
            char_width(&current) + 1 + char_width(word)
        };
        if pending_width > width && !current.is_empty() {
            rows.push(current);
            current = word.to_string();
        } else if current.is_empty() {
            current = word.to_string();
        } else {
            current.push(' ');
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        rows.push(current);
    }
    if rows.is_empty() {
        rows.push(String::new());
    }
    rows
}

fn push_wrapped_token(rows: &mut Vec<String>, current: &mut String, token: &str, width: usize) {
    let mut used = 0usize;
    for grapheme in token.graphemes(true) {
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if grapheme_width == 0 {
            current.push_str(grapheme);
            continue;
        }
        if used + grapheme_width > width && !current.is_empty() {
            rows.push(std::mem::take(current));
            used = 0;
        }
        if grapheme_width > width {
            continue;
        }
        current.push_str(grapheme);
        used += grapheme_width;
    }
    if used == width && !current.is_empty() {
        rows.push(std::mem::take(current));
    }
}

#[allow(dead_code)]
fn suffix_by_width(value: &str, width: usize) -> String {
    let mut used = 0usize;
    let mut graphemes = Vec::new();
    for grapheme in value.graphemes(true).rev() {
        let next = UnicodeWidthStr::width(grapheme);
        if used + next > width {
            break;
        }
        graphemes.push(grapheme);
        used += next;
    }
    graphemes.into_iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cjk_chars_count_as_double_width() {
        assert_eq!(char_width("abc"), 3);
        assert_eq!(char_width("你好"), 4);
        assert_eq!(char_width("a你b"), 4);
    }

    #[test]
    fn pad_and_truncate_use_display_width() {
        assert_eq!(char_width(&pad("你好", 6)), 6);
        assert_eq!(truncate("你好abc", 5), "你好a");
        assert_eq!(truncate("你好abc", 3), "你");
    }

    #[test]
    fn emoji_modifiers_and_combining_marks_do_not_add_cells() {
        assert_eq!(char_width("👋🏻"), 2);
        assert_eq!(char_width("a\u{0301}"), 1);
        assert_eq!(char_width("你\u{FE0F}"), 2);
    }

    #[test]
    fn wraps_by_display_width_not_codepoint_count() {
        let rows = wrap_words("你好你好你好", 4);

        assert_eq!(rows, vec!["你好", "你好", "你好"]);
        assert!(rows.iter().all(|row| char_width(row) <= 4));
    }

    #[test]
    fn compact_middle_uses_display_width() {
        let compacted = compact_middle("路径/你好世界/config.toml", 12);

        assert!(char_width(&compacted) <= 12);
        assert!(compacted.contains('~'));
    }
}
