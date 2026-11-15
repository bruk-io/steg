//! Steg's kernel: session, tab and pane state, and target resolution.
//!
//! The kernel is logic only. It takes data and returns data, and does no I/O
//! of its own (`.claude/rules/architecture.md`). It depends on the sdk,
//! without the sdk's optional parts (R105), for the shapes that cross the
//! API — identifiers, the colour set and pane kinds (`steg_sdk`) — and
//! defines its own behaviour on top of them: minting an identifier,
//! choosing a session's colour, and the tree of sessions, tabs and panes
//! with the rules between them (doc-15).
//!
//! The terminal engine is ghostty's VT engine, through the `ghostty-vt`
//! bindings (doc-16, R92). The dependency does no I/O at runtime: feeding
//! bytes to it and reading cells back is pure computation. The network fetch
//! and the Zig link that building it needs happen before any of this crate's
//! own code runs, from a vendored, sha256-verified copy (`mise.toml`,
//! `.cargo/config.toml`, `third_party/README.md`), never from `just check`.
#![forbid(unsafe_code)]

mod client;
mod colour;
mod error;
mod ids;
mod label;
mod pane;
mod session;
mod tab;
mod target;
mod terminal;
mod tree;

pub use client::Client;
pub use error::Error;
pub use pane::Pane;
pub use session::{Session, TabClosed};
pub use tab::{DirectionalFocus, PaneClosed, Tab};
pub use target::Candidate;
pub use terminal::Terminal;
pub use tree::{ClosedPane, FocusMove, Focused, MovedPane, SizeChanged, Tree};

#[cfg(test)]
mod tests {
    use ghostty_vt::Terminal;
    use ghostty_vt::render::{CellIterator, RenderState, RowIterator};
    use ghostty_vt::terminal::Options;

    /// AC #3 for TASK-1.34: the vendored source and pinned Zig produce a
    /// working engine. A build proof, not the kernel's terminal model -
    /// TASK-1.39 owns scrollback, status and everything else a real pane
    /// needs. Feeds bytes in and reads the screen back, never the emulator's
    /// internals (`.claude/rules/testing-rust.md`). Single-threaded, which is
    /// also the only option: every ghostty handle is `!Send + !Sync`
    /// (decision-3 section 7).
    #[test]
    fn ghostty_vt_round_trips_bytes_to_the_screen() {
        let mut term = Terminal::new(Options {
            cols: 10,
            rows: 2,
            max_scrollback: 0,
        })
        .unwrap();
        term.vt_write(b"hello");

        let mut render = RenderState::new().unwrap();
        let snapshot = render.update(&term).unwrap();
        let mut row_iterator = RowIterator::new().unwrap();
        let mut row_iteration = row_iterator.update(&snapshot).unwrap();
        let first_row = row_iteration.next().expect("the first row exists");
        let mut cell_iterator = CellIterator::new().unwrap();
        let mut cell_iteration = cell_iterator.update(first_row).unwrap();

        let mut text = String::new();
        while let Some(cell) = cell_iteration.next() {
            cell.graphemes_utf8(&mut text).unwrap();
        }

        assert!(
            text.starts_with("hello"),
            "expected \"hello\", got {text:?}"
        );
    }
}
