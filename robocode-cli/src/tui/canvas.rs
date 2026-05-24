use super::text::{display_width, pad};

#[derive(Debug, Clone)]
pub(super) struct Frame {
    pub(super) width: usize,
    pub(super) height: usize,
    rows: Vec<Vec<char>>,
}

impl Frame {
    pub(super) fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            rows: vec![vec![' '; width]; height],
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
                self.rows[y][x] = pattern(x - col, y - row);
            }
        }
    }

    fn write_at(&mut self, row: usize, col: usize, value: &str) {
        if row >= self.height || col >= self.width {
            return;
        }
        let mut x = col;
        for ch in value.chars() {
            let width = display_width(ch);
            if width == 0 {
                continue;
            }
            if x >= self.width || x + width > self.width {
                break;
            }
            self.rows[row][x] = ch;
            if width > 1 {
                for covered in x + 1..x + width {
                    self.rows[row][covered] = '\0';
                }
            }
            x += width;
        }
    }
}

impl std::fmt::Display for Frame {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, row) in self.rows.iter().enumerate() {
            for ch in row {
                if *ch != '\0' {
                    formatter.write_str(&ch.to_string())?;
                }
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
}
