pub(super) const PANE_WIDTH: f64 = 300.0;
pub(super) const CONTENT_INSET: f64 = 40.0;

pub(super) fn expanded(width: f64) -> bool {
    width >= 800.0
}

pub(super) fn stacked(width: f64) -> bool {
    let content = width - if expanded(width) { PANE_WIDTH } else { 0.0 } - 2.0 * CONTENT_INSET;
    content < 740.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_changes_at_800_effective_pixels() {
        assert!(!expanded(799.0));
        assert!(expanded(800.0));
    }

    #[test]
    fn rows_reflow_when_the_page_lacks_room() {
        assert!(stacked(640.0));
        assert!(stacked(700.0));
        assert!(stacked(799.0));
        assert!(stacked(800.0));
        assert!(stacked(1119.0));
        assert!(!stacked(1120.0));
        assert!(!stacked(1200.0));
    }
}
