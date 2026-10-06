use super::App;
use crate::git::diff::{DiffLine, DiffLineKind};

#[derive(Debug, Default)]
pub(crate) struct PreviewLayout {
    headers: Vec<usize>,
    boundaries: Vec<usize>,
}

impl App {
    fn preview_layout(&self) -> &PreviewLayout {
        self.preview_layout.get_or_init(|| {
            let mut layout = PreviewLayout::default();
            for (index, line) in self.document.lines.iter().enumerate() {
                match line.kind {
                    DiffLineKind::FileHeader => {
                        layout.headers.push(index);
                        layout.boundaries.push(index);
                    }
                    DiffLineKind::FileFooter => layout.boundaries.push(index),
                    _ => {}
                }
            }
            layout
        })
    }

    pub(crate) fn preview_header_indices(&self) -> &[usize] {
        &self.preview_layout().headers
    }

    pub(crate) fn preview_header_at(&self, line_index: usize) -> Option<&DiffLine> {
        let boundaries = &self.preview_layout().boundaries;
        let end = boundaries.partition_point(|index| *index <= line_index);
        end.checked_sub(1)
            .and_then(|index| boundaries.get(index))
            .and_then(|index| self.document.lines.get(*index))
            .filter(|line| line.kind == DiffLineKind::FileHeader)
    }

    pub(super) fn rendered_preview_paths(&self) -> impl Iterator<Item = &str> + Clone {
        self.preview_header_indices()
            .iter()
            .filter_map(|index| self.document.lines.get(*index))
            .filter_map(|line| line.spans.first())
            .map(|span| span.text.split("  · ").next().unwrap_or(&span.text))
    }
}
