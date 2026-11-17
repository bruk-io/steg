//! Pixels to a terminal grid, with a floor, and the coalescing that keeps a window drag from
//! flooding `tab.declare-size` calls (TASK-1.121 plan, section 4). Pure state and functions -
//! no `gpui::Window`, no socket. Dimensions are plain pixel values (`f32`, matching
//! `gpui::Pixels`'s own representation, `geometry.rs:2573`); the real call site converts via
//! `f32::from(pixels)`.

/// A terminal's size in whole cells. Never zero in either dimension - `grid_size`'s own
/// floor is what guarantees that, upstream of the server's own refusal of a zero size
/// (TASK-1.114's plan, "For the engineer" point 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GridSize {
    pub(crate) columns: u16,
    pub(crate) rows: u16,
}

/// How many whole cells fit a viewport of `width` by `height` pixels, given a cell
/// `cell_width` by `line_height` pixels. Floors to one column and one row even when the
/// viewport is smaller than a single cell - a terminal always has at least one cell to draw
/// into, and the floor is what keeps a degenerate layout from ever computing the zero
/// dimension the server refuses.
///
/// # Panics
///
/// Panics if `cell_width` or `line_height` is not a finite, positive number. Both come from
/// a font's own shaped metrics, never from a person or the network, so a non-positive value
/// here is a bug in the caller, not an input this function is asked to handle.
pub(crate) fn grid_size(width: f32, height: f32, cell_width: f32, line_height: f32) -> GridSize {
    assert!(
        cell_width > 0.0 && cell_width.is_finite(),
        "cell_width must be a positive, finite number of pixels, got {cell_width}",
    );
    assert!(
        line_height > 0.0 && line_height.is_finite(),
        "line_height must be a positive, finite number of pixels, got {line_height}",
    );
    GridSize {
        columns: (width / cell_width).floor().max(1.0) as u16,
        rows: (height / line_height).floor().max(1.0) as u16,
    }
}

/// One step of the resize coalescer: given the size last actually declared and a freshly
/// computed size, decides whether a new `tab.declare-size` call is due. A pure state
/// transition driven only by the caller feeding it each resize event as it happens - no
/// timer, no debounce. `Some(computed)` exactly when `computed != declared`, which is what
/// keeps a drag within one cell's width or height from sending anything at all.
pub(crate) fn declare_if_changed(declared: GridSize, computed: GridSize) -> Option<GridSize> {
    if computed == declared {
        None
    } else {
        Some(computed)
    }
}

/// Runs the coalescer over a whole sequence of computed sizes, starting from `initial` (the
/// size already declared before this sequence began), returning the sizes it would actually
/// send, in order. A pure simulation of the real resize-driven loop (`AppWindow::on_resize`,
/// `main.rs`, which feeds `declare_if_changed` one size at a time and updates its own
/// `declared` state the same way) - this is that same logic run over a whole sequence, test
/// code only, which is what makes "bounded, and always ends on the sequence's own last
/// distinct value" a single, direct assertion rather than one step replayed by hand.
#[cfg(test)]
pub(crate) fn coalesce(
    initial: GridSize,
    computed: impl IntoIterator<Item = GridSize>,
) -> Vec<GridSize> {
    let mut declared = initial;
    let mut sent = Vec::new();
    for size in computed {
        if let Some(next) = declare_if_changed(declared, size) {
            declared = next;
            sent.push(next);
        }
    }
    sent
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(columns: u16, rows: u16) -> GridSize {
        GridSize { columns, rows }
    }

    #[test]
    fn a_viewport_many_cells_wide_and_tall_divides_evenly() {
        assert_eq!(grid_size(800.0, 480.0, 8.0, 16.0), size(100, 30));
    }

    #[test]
    fn a_viewport_that_does_not_divide_evenly_floors_the_remainder() {
        assert_eq!(grid_size(805.0, 487.0, 8.0, 16.0), size(100, 30));
    }

    #[test]
    fn a_viewport_smaller_than_one_cell_floors_to_one_column_and_one_row_not_zero() {
        assert_eq!(grid_size(3.0, 4.0, 8.0, 16.0), size(1, 1));
    }

    #[test]
    #[should_panic(expected = "cell_width must be a positive, finite number")]
    fn a_zero_cell_width_is_a_caller_bug_not_a_zero_grid() {
        grid_size(800.0, 480.0, 0.0, 16.0);
    }

    #[test]
    fn an_unchanged_computed_size_declares_nothing() {
        assert_eq!(declare_if_changed(size(80, 24), size(80, 24)), None);
    }

    #[test]
    fn a_changed_computed_size_declares_itself() {
        assert_eq!(
            declare_if_changed(size(80, 24), size(81, 24)),
            Some(size(81, 24))
        );
    }

    /// The bounded-declarations proof: a drag that passes through several viewport sizes
    /// rounding to the same cell pair, then crosses into a new one, then settles on its own
    /// final size - the coalescer emits exactly the distinct sizes actually crossed, never
    /// one per input, and always ends on the sequence's own last value.
    #[test]
    fn a_drag_emits_only_the_distinct_sizes_it_crosses_and_always_ends_on_the_last_one() {
        let drag = [
            size(80, 24),
            size(80, 24),
            size(80, 24), // sub-cell jitter: no new size yet
            size(81, 24), // crosses one column
            size(81, 24), // settles there briefly
            size(81, 25), // crosses one row too - the final size
        ];
        let sent = coalesce(size(80, 24), drag);
        assert_eq!(sent, vec![size(81, 24), size(81, 25)]);
        assert_eq!(sent.last(), Some(&size(81, 25)));
    }
}
