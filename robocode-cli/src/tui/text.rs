const HORIZONTAL: char = '─';

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
    value.chars().map(display_width).sum()
}

pub(super) fn pad(value: &str, width: usize) -> String {
    let value = truncate(value, width);
    let padding = width.saturating_sub(char_width(&value));
    format!("{value}{}", " ".repeat(padding))
}

pub(super) fn truncate(value: &str, width: usize) -> String {
    let mut used = 0usize;
    let mut output = String::new();
    for value in value.chars() {
        let next = display_width(value);
        if used + next > width {
            break;
        }
        output.push(value);
        used += next;
    }
    output
}

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

pub(super) fn display_width(value: char) -> usize {
    if value == '\0' || value.is_control() {
        0
    } else if is_zero_width(value) {
        0
    } else if is_wide(value) {
        2
    } else {
        1
    }
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
    for ch in token.chars() {
        let ch_width = display_width(ch);
        if ch_width == 0 {
            current.push(ch);
            continue;
        }
        if used + ch_width > width && !current.is_empty() {
            rows.push(std::mem::take(current));
            used = 0;
        }
        if ch_width > width {
            continue;
        }
        current.push(ch);
        used += ch_width;
    }
    if used == width && !current.is_empty() {
        rows.push(std::mem::take(current));
    }
}

fn suffix_by_width(value: &str, width: usize) -> String {
    let mut used = 0usize;
    let mut chars = Vec::new();
    for ch in value.chars().rev() {
        let next = display_width(ch);
        if used + next > width {
            break;
        }
        chars.push(ch);
        used += next;
    }
    chars.into_iter().rev().collect()
}

fn is_zero_width(value: char) -> bool {
    matches!(
        value as u32,
        0x0300..=0x036F
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x206F
            | 0x20D0..=0x20FF
            | 0xFE00..=0xFE0F
            | 0x1F3FB..=0x1F3FF
            | 0xE0100..=0xE01EF
    )
}

fn is_wide(value: char) -> bool {
    matches!(
        value as u32,
        0x1100..=0x115F
            | 0x2329..=0x232A
            | 0x2E80..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE19
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1FAFF
            | 0x20000..=0x3FFFD
    )
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
