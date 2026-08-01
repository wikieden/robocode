pub(super) fn effective_layout_width(terminal_width: u16) -> usize {
    usize::from(terminal_width).max(1)
}
