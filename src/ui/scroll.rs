//! Vertical scrolling shared by the reader and the scan results.

/// A vertical scroll position that never passes the end of its content.
#[derive(Default)]
pub(super) struct Scroll {
    offset: u16,
    max: u16,
}

impl Scroll {
    pub(super) fn offset(&self) -> u16 {
        self.offset
    }

    pub(super) fn down(&mut self, lines: u16) {
        self.offset = self.offset.saturating_add(lines).min(self.max);
    }

    pub(super) fn up(&mut self, lines: u16) {
        self.offset = self.offset.saturating_sub(lines);
    }

    pub(super) fn top(&mut self) {
        self.offset = 0;
    }

    /// Called while drawing, once the content's height in the current layout is known.
    pub(super) fn limit(&mut self, max: usize) {
        self.max = max.min(usize::from(u16::MAX)) as u16;
        self.offset = self.offset.min(self.max);
    }
}

#[cfg(test)]
mod tests {
    use super::Scroll;

    #[test]
    fn scroll_stays_between_the_top_and_the_end_of_its_content() {
        let mut scroll = Scroll::default();
        scroll.down(5);
        assert_eq!(
            scroll.offset(),
            0,
            "nothing to scroll before content is measured"
        );
        scroll.limit(12);
        scroll.down(10);
        scroll.down(10);
        assert_eq!(scroll.offset(), 12);
        scroll.up(3);
        assert_eq!(scroll.offset(), 9);
        // Content that shrinks, or a taller viewport, pulls the position back in range.
        scroll.limit(4);
        assert_eq!(scroll.offset(), 4);
        scroll.up(10);
        assert_eq!(scroll.offset(), 0);
        scroll.down(2);
        scroll.top();
        assert_eq!(scroll.offset(), 0);
        scroll.limit(usize::MAX);
        scroll.down(u16::MAX);
        assert_eq!(scroll.offset(), u16::MAX);
    }
}
