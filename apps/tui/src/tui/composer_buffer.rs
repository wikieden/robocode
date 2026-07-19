use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(super) const MAX_VISIBLE_ROWS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct ComposerBuffer {
    text: String,
    // Every edit and motion operation leaves this byte offset on a grapheme
    // boundary; rendering may therefore slice without splitting user input.
    cursor: usize,
    desired_column: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CursorCell {
    pub(super) column: usize,
    pub(super) row: usize,
}

#[derive(Debug, Clone, Copy)]
struct VisualRow {
    start: usize,
    end: usize,
    soft_wrap: bool,
}

impl ComposerBuffer {
    pub(super) fn as_str(&self) -> &str {
        &self.text
    }

    pub(super) fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub(super) fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.desired_column = None;
    }

    pub(super) fn replace(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor = self.text.len();
        self.desired_column = None;
    }

    pub(super) fn insert(&mut self, value: &str) {
        self.text.insert_str(self.cursor, value);
        self.cursor += value.len();
        self.desired_column = None;
    }

    pub(super) fn paste(&mut self, value: &str) {
        let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
        self.insert(&normalized);
    }

    pub(super) fn backspace(&mut self) {
        let Some((previous, _)) = self.text[..self.cursor].grapheme_indices(true).next_back()
        else {
            return;
        };
        self.text.drain(previous..self.cursor);
        self.cursor = previous;
        self.desired_column = None;
    }

    pub(super) fn move_left(&mut self) {
        if let Some((previous, _)) = self.text[..self.cursor].grapheme_indices(true).next_back() {
            self.cursor = previous;
            self.desired_column = None;
        }
    }

    pub(super) fn move_right(&mut self) {
        if let Some(grapheme) = self.text[self.cursor..].graphemes(true).next() {
            self.cursor += grapheme.len();
            self.desired_column = None;
        }
    }

    pub(super) fn move_up(&mut self, width: usize) {
        self.move_vertical(width, -1);
    }

    pub(super) fn move_down(&mut self, width: usize) {
        self.move_vertical(width, 1);
    }

    pub(super) fn insert_newline(&mut self) {
        self.insert("\n");
    }

    pub(super) fn visible_rows(&self, width: usize) -> Vec<String> {
        let rows = self.visual_rows(width);
        let cursor_row = self.cursor_row(&rows);
        let start = cursor_row
            .saturating_add(1)
            .saturating_sub(MAX_VISIBLE_ROWS);
        rows[start..]
            .iter()
            .take(MAX_VISIBLE_ROWS)
            .map(|row| self.text[row.start..row.end].to_string())
            .collect()
    }

    pub(super) fn cursor_cell(&self, width: usize) -> CursorCell {
        let rows = self.visual_rows(width);
        let absolute_row = self.cursor_row(&rows);
        let viewport_start = absolute_row
            .saturating_add(1)
            .saturating_sub(MAX_VISIBLE_ROWS);
        let row = rows[absolute_row];
        CursorCell {
            column: UnicodeWidthStr::width(&self.text[row.start..self.cursor.min(row.end)]),
            row: absolute_row - viewport_start,
        }
    }

    pub(super) fn has_unclosed_code_fence(&self) -> bool {
        self.text.match_indices("```").count() % 2 == 1
    }

    fn move_vertical(&mut self, width: usize, direction: isize) {
        let rows = self.visual_rows(width);
        let current = self.cursor_row(&rows);
        let target = current.saturating_add_signed(direction).min(rows.len() - 1);
        if target == current {
            return;
        }
        let column = *self.desired_column.get_or_insert_with(|| {
            let row = rows[current];
            UnicodeWidthStr::width(&self.text[row.start..self.cursor.min(row.end)])
        });
        self.cursor = self.boundary_at_column(rows[target], column);
    }

    fn boundary_at_column(&self, row: VisualRow, desired: usize) -> usize {
        let mut boundary = row.start;
        let mut used = 0;
        for (offset, grapheme) in self.text[row.start..row.end].grapheme_indices(true) {
            let next = used + UnicodeWidthStr::width(grapheme);
            if next > desired {
                break;
            }
            boundary = row.start + offset + grapheme.len();
            used = next;
        }
        boundary
    }

    fn cursor_row(&self, rows: &[VisualRow]) -> usize {
        for (index, row) in rows.iter().enumerate() {
            if self.cursor < row.end
                || (self.cursor == row.end && (!row.soft_wrap || index + 1 == rows.len()))
            {
                return index;
            }
        }
        rows.len().saturating_sub(1)
    }

    fn visual_rows(&self, width: usize) -> Vec<VisualRow> {
        let width = width.max(1);
        let mut rows = Vec::new();
        let mut start = 0;
        let mut used = 0;

        for (index, grapheme) in self.text.grapheme_indices(true) {
            if grapheme == "\n" {
                rows.push(VisualRow {
                    start,
                    end: index,
                    soft_wrap: false,
                });
                start = index + grapheme.len();
                used = 0;
                continue;
            }

            let grapheme_width = UnicodeWidthStr::width(grapheme);
            if used > 0 && used + grapheme_width > width {
                rows.push(VisualRow {
                    start,
                    end: index,
                    soft_wrap: true,
                });
                start = index;
                used = 0;
            }
            used += grapheme_width;
        }

        rows.push(VisualRow {
            start,
            end: self.text.len(),
            soft_wrap: false,
        });
        rows
    }
}

impl From<&str> for ComposerBuffer {
    fn from(value: &str) -> Self {
        let mut buffer = Self::default();
        buffer.replace(value);
        buffer
    }
}

impl From<String> for ComposerBuffer {
    fn from(value: String) -> Self {
        let mut buffer = Self::default();
        buffer.replace(value);
        buffer
    }
}

impl PartialEq<&str> for ComposerBuffer {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_at_grapheme_boundaries_for_combining_and_emoji_clusters() {
        let mut buffer = ComposerBuffer::from("Ae\u{301}👍🏽👨‍👩‍👧‍👦Z");

        buffer.move_left();
        buffer.backspace();

        assert_eq!(buffer.as_str(), "Ae\u{301}👍🏽Z");
        assert_eq!(buffer.cursor_cell(40), CursorCell { column: 4, row: 0 });

        buffer.move_left();
        buffer.backspace();
        assert_eq!(buffer.as_str(), "A👍🏽Z");
        buffer.move_right();
        assert_eq!(buffer.cursor_cell(40).column, 3);
    }

    #[test]
    fn inserts_in_the_middle_without_splitting_a_grapheme() {
        let mut buffer = ComposerBuffer::from("你👍🏽好");
        buffer.move_left();
        buffer.move_left();
        buffer.insert("们");

        assert_eq!(buffer.as_str(), "你们👍🏽好");
        assert_eq!(buffer.cursor_cell(20).column, 4);
    }

    #[test]
    fn cjk_cells_wrap_at_double_width() {
        let buffer = ComposerBuffer::from("甲乙丙丁戊");

        assert_eq!(buffer.visible_rows(4), vec!["甲乙", "丙丁", "戊"]);
        assert_eq!(buffer.cursor_cell(4), CursorCell { column: 2, row: 2 });
    }

    #[test]
    fn vertical_motion_preserves_the_desired_display_column() {
        let mut buffer = ComposerBuffer::from("abcd\n你乙\nxyzw");
        buffer.move_left();
        buffer.move_left();
        assert_eq!(buffer.cursor_cell(20).column, 2);

        buffer.move_up(20);
        assert_eq!(buffer.cursor_cell(20), CursorCell { column: 2, row: 1 });
        buffer.move_up(20);
        assert_eq!(buffer.cursor_cell(20), CursorCell { column: 2, row: 0 });
        buffer.move_down(20);
        buffer.move_down(20);
        assert_eq!(buffer.cursor_cell(20), CursorCell { column: 2, row: 2 });
    }

    #[test]
    fn viewport_is_limited_to_eight_rows_and_tracks_the_cursor() {
        let buffer =
            ComposerBuffer::from("one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten");

        assert_eq!(buffer.visible_rows(20).len(), MAX_VISIBLE_ROWS);
        assert_eq!(
            buffer.visible_rows(20).first().map(String::as_str),
            Some("three")
        );
        assert_eq!(
            buffer.visible_rows(20).last().map(String::as_str),
            Some("ten")
        );
        assert_eq!(buffer.cursor_cell(20).row, 7);
    }

    #[test]
    fn paste_normalizes_line_endings_and_never_changes_cursor_semantics() {
        let mut buffer = ComposerBuffer::from("ab");
        buffer.move_left();
        buffer.paste("X\r\nY\rZ");

        assert_eq!(buffer.as_str(), "aX\nY\nZb");
        assert_eq!(buffer.cursor_cell(20), CursorCell { column: 1, row: 2 });
    }

    #[test]
    fn detects_unclosed_triple_backtick_fence() {
        let mut buffer = ComposerBuffer::from("```rust\nfn main() {}");
        assert!(buffer.has_unclosed_code_fence());
        buffer.insert("\n```");
        assert!(!buffer.has_unclosed_code_fence());
    }

    #[test]
    fn unicode_dependencies_treat_graphemes_and_cells_as_the_source_of_truth() {
        let graphemes = UnicodeSegmentation::graphemes("e\u{301}👍🏽", true).collect::<Vec<_>>();
        assert_eq!(graphemes, vec!["e\u{301}", "👍🏽"]);
        assert_eq!(UnicodeWidthStr::width("你"), 2);
    }
}
