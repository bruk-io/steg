//! Candidate D2's client: plain rows, no emulator.
//!
//! Everything this client knows came out of a snapshot and a stream of deltas. It parses no
//! escape sequences, which is the whole claim being tested: a client of this protocol needs no
//! terminal emulator, and so neither does a Python plugin (R42, R75).

use probe_wire::{Cursor, Row, Snapshot, Update, apply};

use crate::capture::{SCROLLBACK_LIMIT, lines};

pub struct ClientGrid {
    pub screen: Vec<Row>,
    pub scrollback: Vec<Row>,
    pub cursor: Cursor,
    pub cols: u16,
    pub rows: u16,
    pub seq: u64,
    pub resyncs: u64,
    pub deltas: u64,
    /// How many rows have ever scrolled off, as of the last update applied.
    pub rows_ever: u64,
    /// False in `screen` mode: the stream carries no scrollback, so the client holds only what
    /// the snapshot gave it and asks for a page when it wants more.
    pub track_scrollback: bool,
}

impl ClientGrid {
    pub fn from_snapshot(snapshot: &Snapshot, track_scrollback: bool) -> Self {
        Self {
            screen: snapshot.screen.clone(),
            scrollback: snapshot.scrollback.clone(),
            cursor: snapshot.cursor,
            cols: snapshot.cols,
            rows: snapshot.rows,
            seq: snapshot.seq,
            resyncs: 0,
            deltas: 0,
            rows_ever: snapshot.rows_ever,
            track_scrollback,
        }
    }

    pub fn apply_update(&mut self, update: &Update) {
        match update {
            Update::Delta(delta) => {
                apply(&mut self.screen, &mut self.scrollback, delta);
                self.cursor = delta.cursor;
                self.seq = delta.seq;
                self.rows_ever = delta.rows_ever;
                self.deltas += 1;
                while self.scrollback.len() > SCROLLBACK_LIMIT {
                    self.scrollback.remove(0);
                }
            }
            Update::Resync { snapshot, .. } => {
                self.screen = snapshot.screen.clone();
                // In `screen` mode the resync carries no scrollback, so whatever this client
                // holds stays until it asks for a page.
                if self.track_scrollback || !snapshot.scrollback.is_empty() {
                    self.scrollback = snapshot.scrollback.clone();
                }
                self.cursor = snapshot.cursor;
                self.cols = snapshot.cols;
                self.rows = snapshot.rows;
                self.seq = snapshot.seq;
                self.rows_ever = snapshot.rows_ever;
                self.resyncs += 1;
            }
        }
    }

    pub fn capture(&self, with_scrollback: bool) -> String {
        let with_scrollback = with_scrollback && self.track_scrollback;
        let start = self.scrollback.len().saturating_sub(SCROLLBACK_LIMIT);
        let scrollback = if with_scrollback {
            &self.scrollback[start..]
        } else {
            &[][..]
        };
        lines(scrollback.iter().chain(self.screen.iter()).map(Row::text))
    }

    pub fn screen_text(&self) -> String {
        lines(self.screen.iter().map(Row::text))
    }
}
