use super::text::pad;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone)]
pub(super) struct Frame {
    pub(super) width: usize,
    pub(super) height: usize,
    rows: Vec<Vec<Cell>>,
}

#[derive(Debug, Clone)]
struct Cell {
    text: String,
    owner: usize,
}

impl Cell {
    fn blank(column: usize) -> Self {
        Self {
            text: " ".to_string(),
            owner: column,
        }
    }
}

impl Frame {
    pub(super) fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            rows: (0..height)
                .map(|_| (0..width).map(Cell::blank).collect())
                .collect(),
        }
    }

    pub(super) fn write_block(&mut self, row: usize, col: usize, lines: &[String]) {
        for (offset, line) in lines.iter().enumerate() {
            self.write_at(row + offset, col, line);
        }
    }

    pub(super) fn write_line(&mut self, row: usize, line: &str) {
        self.write_at(row, 0, &pad(line, self.width));
    }

    #[allow(dead_code)]
    pub(super) fn fill_rect_pattern(
        &mut self,
        row: usize,
        col: usize,
        width: usize,
        height: usize,
        pattern: impl Fn(usize, usize) -> char,
    ) {
        for y in row..row.saturating_add(height).min(self.height) {
            for x in col..col.saturating_add(width).min(self.width) {
                self.write_at(y, x, &pattern(x - col, y - row).to_string());
            }
        }
    }

    fn write_at(&mut self, row: usize, col: usize, value: &str) {
        if row >= self.height || col >= self.width {
            return;
        }
        let mut x = col;
        for grapheme in value.graphemes(true) {
            // Transcript fields can contain embedded line controls. They must
            // not escape the fixed terminal row.
            if grapheme.chars().any(char::is_control) {
                continue;
            }
            let width = UnicodeWidthStr::width(grapheme);
            if width == 0 {
                // Genuine zero-width marks remain attached to the preceding
                // rendered cell.
                if x > col {
                    let owner = self.rows[row][x - 1].owner;
                    self.rows[row][owner].text.push_str(grapheme);
                }
                continue;
            }
            if x >= self.width || x + width > self.width {
                break;
            }
            for covered in x..x + width {
                self.clear_cluster_at(row, covered);
            }
            self.rows[row][x] = Cell {
                text: grapheme.to_string(),
                owner: x,
            };
            if width > 1 {
                for covered in x + 1..x + width {
                    self.rows[row][covered] = Cell {
                        text: String::new(),
                        owner: x,
                    };
                }
            }
            x += width;
        }
    }

    fn clear_cluster_at(&mut self, row: usize, column: usize) {
        let owner = self.rows[row][column].owner;
        let span = UnicodeWidthStr::width(self.rows[row][owner].text.as_str()).max(1);
        for covered in owner..owner.saturating_add(span).min(self.width) {
            self.rows[row][covered] = Cell::blank(covered);
        }
    }
}

impl std::fmt::Display for Frame {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, row) in self.rows.iter().enumerate() {
            for cell in row {
                formatter.write_str(&cell.text)?;
            }
            if index + 1 < self.rows.len() {
                formatter.write_str("\n")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::text::char_width;

    #[test]
    fn wide_characters_occupy_display_cells() {
        let mut frame = Frame::new(8, 1);
        frame.write_at(0, 0, "你");
        frame.write_at(0, 4, "│");

        let rendered = frame.to_string();
        let separator = rendered.find('│').expect("separator");

        assert_eq!(char_width(&rendered[..separator]), 4);
        assert_eq!(char_width(&rendered), 8);
    }

    #[test]
    fn wide_character_at_right_edge_does_not_overflow() {
        let mut frame = Frame::new(4, 1);
        frame.write_at(0, 3, "你");

        let rendered = frame.to_string();

        assert_eq!(rendered, "    ");
        assert_eq!(char_width(&rendered), 4);
    }

    #[test]
    fn emoji_modifier_cluster_occupies_one_rendered_cell_span() {
        let mut frame = Frame::new(6, 1);
        frame.write_at(0, 1, "👋🏻");

        let rendered = frame.to_string();

        assert!(rendered.contains("👋🏻"));
        assert_eq!(char_width(&rendered), 6);
    }

    #[test]
    fn embedded_line_controls_do_not_escape_the_fixed_frame_row() {
        let mut frame = Frame::new(12, 1);
        frame.write_at(0, 0, "first\nsecond");

        let rendered = frame.to_string();

        assert_eq!(rendered.lines().count(), 1);
        assert_eq!(char_width(&rendered), 12);
    }

    #[test]
    fn overwriting_any_wide_cluster_cell_clears_the_whole_previous_cluster() {
        for cluster in ["你", "👨‍👩‍👧‍👦"] {
            for overwrite_column in [1, 2] {
                let mut frame = Frame::new(6, 1);
                frame.write_at(0, 1, cluster);

                frame.write_at(0, overwrite_column, "X");
                let rendered = frame.to_string();

                assert!(
                    !rendered.contains(cluster),
                    "{cluster} at {overwrite_column}"
                );
                assert_eq!(char_width(&rendered), 6, "{cluster} at {overwrite_column}");
            }
        }
    }
}
