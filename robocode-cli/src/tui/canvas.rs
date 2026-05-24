use super::text::pad;

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
        for (offset, ch) in value.chars().take(self.width - col).enumerate() {
            self.rows[row][col + offset] = ch;
        }
    }
}

impl std::fmt::Display for Frame {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, row) in self.rows.iter().enumerate() {
            for ch in row {
                formatter.write_str(&ch.to_string())?;
            }
            if index + 1 < self.rows.len() {
                formatter.write_str("\n")?;
            }
        }
        Ok(())
    }
}
