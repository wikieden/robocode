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
    let prefix: String = value.chars().take(head).collect();
    let suffix: String = value
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{prefix}~{suffix}")
}

fn display_width(value: char) -> usize {
    if value == '\0' || value.is_control() {
        0
    } else if is_wide(value) {
        2
    } else {
        1
    }
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
}
