//! Candidate D2: what the server sends after the snapshot when it computes the screen itself.
//!
//! The important shape here is `Update::Resync`. A delta stream needs an escape hatch for the
//! case where it cannot describe the change: a client too slow to keep up (back-pressure), or
//! more lines scrolling between two ticks than the server can account for. That escape hatch is
//! the same mechanism in both cases, and candidate D1 needs it too, which is why D1 cannot
//! avoid the snapshot path.

use serde::{Deserialize, Serialize};

use crate::snapshot::{Cursor, Row, Snapshot};

/// One screen row that changed, by its index from the top of the screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowChange {
    pub index: u16,
    pub row: Row,
}

/// Everything that changed since the last update sent to this attachment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delta {
    pub seq: u64,
    pub rows: Vec<RowChange>,
    /// Rows that scrolled off the screen into scrollback, oldest first.
    pub scrollback_appended: Vec<Row>,
    /// Rows the server dropped off the far end of its scrollback limit (R15).
    pub scrollback_dropped: u32,
    /// How many rows have ever scrolled off, as of this update. Carried even in `screen` mode,
    /// where no rows are: it is what lets a client that fetches scrollback in pages line those
    /// pages up with the screen it is holding.
    pub rows_ever: u64,
    pub cursor: Cursor,
    pub alt_screen: bool,
}

/// What a `Deltas` attachment receives on its stream after the snapshot in its attach result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "update", rename_all = "kebab-case")]
pub enum Update {
    Delta(Delta),
    /// Throw away what you have and start again from this. Sent when the attachment fell behind
    /// or when the server cannot express the change as a delta.
    Resync {
        snapshot: Box<Snapshot>,
        reason: String,
    },
}

/// The screen rows that differ, by index. Pure, so the diff is proved without a terminal.
#[must_use]
pub fn diff_screen(before: &[Row], after: &[Row]) -> Vec<RowChange> {
    let mut changes = Vec::new();
    for (index, row) in after.iter().enumerate() {
        let same = before.get(index).is_some_and(|old| old == row);
        if !same {
            // A screen is 24 to 200 rows; the index fits a u16 with room to spare.
            changes.push(RowChange {
                index: index as u16,
                row: row.clone(),
            });
        }
    }
    changes
}

/// Apply an update to a client's own copy. The client holds plain rows and no emulator, which is
/// the whole point of D2.
pub fn apply(screen: &mut Vec<Row>, scrollback: &mut Vec<Row>, delta: &Delta) {
    for _ in 0..delta.scrollback_dropped {
        if scrollback.is_empty() {
            break;
        }
        scrollback.remove(0);
    }
    scrollback.extend(delta.scrollback_appended.iter().cloned());
    for change in &delta.rows {
        let index = usize::from(change.index);
        if index >= screen.len() {
            screen.resize(index + 1, Row::default());
        }
        screen[index] = change.row.clone();
    }
}
