use ratatui::layout::Rect;

pub(crate) struct ModalList<'a> {
    hits: &'a mut Vec<(Rect, usize)>,
    len: &'a mut usize,
    max_scroll: &'a mut usize,
    scroll: usize,
    free_scroll: bool,
}

impl<'a> ModalList<'a> {
    pub(crate) const fn new(
        hits: &'a mut Vec<(Rect, usize)>,
        len: &'a mut usize,
        max_scroll: &'a mut usize,
        scroll: usize,
        free_scroll: bool,
    ) -> Self {
        Self {
            hits,
            len,
            max_scroll,
            scroll,
            free_scroll,
        }
    }

    pub(crate) fn offset(&mut self, selected: usize, visible: usize, total: usize) -> usize {
        *self.len = total;
        let maximum = total.saturating_sub(visible);
        *self.max_scroll = maximum;
        if self.free_scroll {
            return self.scroll.min(maximum);
        }
        let lower = selected.saturating_sub(visible.saturating_sub(1));
        self.scroll.clamp(lower, selected).min(maximum)
    }

    pub(crate) fn hit(&mut self, area: Rect, index: usize) {
        self.hits.push((area, index));
    }
}
