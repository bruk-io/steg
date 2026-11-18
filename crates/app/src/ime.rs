//! The minimum state a terminal's input handler tracks for IME composition (TASK-1.121
//! plan, section 2). There is no editable text buffer here - unlike `gpui`'s own
//! `examples/input.rs` (`TextInput`, a real text field with a `content: SharedString`), a
//! terminal has no "range of the document" for the input method to read or replace into.
//! This tracks only what composition itself needs: whether one is open, and what to send
//! once it resolves. Marked (uncommitted) text is never sent to the program - only
//! `commit`'s own, resolved text is.
//!
//! **`marked_range_utf16` reports the composition's own real length, never a fixed
//! `0..0`** (review finding): AppKit uses `NSTextInputClient.markedRange` to compute the
//! `replacement_range` it passes into the *next* `setMarkedText:` call (confirmed in GPUI's
//! own pinned macOS backend, `platform/mac/window.rs:2177-2182`) - a composition longer
//! than one code unit (Pinyin/Hangul assembly, a longer emoji-picker candidate) reporting a
//! zero-width range here would have AppKit believe nothing is marked yet, risking
//! duplicated or garbled composition content on the next keystroke.

/// Whether an IME composition is open, and its own pending (uncommitted) text if so.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ImeState {
    marked: Option<String>,
}

impl ImeState {
    /// Marks `text` as the composition's own pending content, replacing any earlier marked
    /// text - `EntityInputHandler::replace_and_mark_text_in_range`'s own call. An empty
    /// `text` clears the mark, the same convention `setMarkedText` itself uses. Sends
    /// nothing: a composition's own pending text draws no preview and reaches no program
    /// (TASK-1.121 plan's own stated gap - a possible later follow-up for TASK-1.56's view).
    pub(crate) fn mark(&mut self, text: &str) {
        self.marked = if text.is_empty() {
            None
        } else {
            Some(text.to_owned())
        };
    }

    /// Clears the composition without committing it - `unmark_text`'s own call.
    pub(crate) fn unmark(&mut self) {
        self.marked = None;
    }

    /// The composition's own range, in UTF-16 code units (`EntityInputHandler`'s own
    /// convention) - `0..` the composition's own real length, never a fixed `0..0`
    /// regardless of how long it actually is (module doc). `None` when nothing is
    /// composing - `marked_text_range`'s own answer, and the one place GPUI's own macOS
    /// backend reads whether a composition is open at all: it routes the *next* keystroke
    /// to the input context instead of this app's raw key handler exactly when this is
    /// `Some` (`platform/mac/window.rs:1696-1699`) - the other half of the dedup rule in
    /// `keys.rs`'s own doc comment.
    #[must_use]
    pub(crate) fn marked_range_utf16(&self) -> Option<std::ops::Range<usize>> {
        self.marked
            .as_ref()
            .map(|text| 0..text.encode_utf16().count())
    }

    /// Resolves the composition - or a direct, non-composing insert, which takes the same
    /// path with no mark ever having been set - into the bytes to send: `text`'s own line
    /// endings normalised the same way Enter is encoded (`keys.rs`), and the mark cleared.
    /// The one path committed text ever takes, whether it came from a resolved composition
    /// or a plain `insertText` call with no composition open at all.
    pub(crate) fn commit(&mut self, text: &str) -> Vec<u8> {
        self.marked = None;
        normalize_line_endings(text).into_bytes()
    }
}

/// `\r\n` and `\n` both become `\r` - the same byte Enter itself sends (`keys.rs`), so a
/// multi-line paste or a composed string containing a line break reads to the program as if
/// each line had been typed and Enter pressed. Used by composed text here and, per the
/// plan's own "For the engineer" point 3, by Cmd-V paste too.
pub(crate) fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\r").replace('\n', "\r")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marking_text_opens_the_composition_and_sends_nothing() {
        let mut state = ImeState::default();
        assert!(state.marked_range_utf16().is_none());
        state.mark("e");
        assert!(state.marked_range_utf16().is_some());
    }

    #[test]
    fn marking_empty_text_closes_the_composition() {
        let mut state = ImeState::default();
        state.mark("e");
        state.mark("");
        assert!(state.marked_range_utf16().is_none());
    }

    #[test]
    fn unmarking_closes_the_composition() {
        let mut state = ImeState::default();
        state.mark("e");
        state.unmark();
        assert!(state.marked_range_utf16().is_none());
    }

    #[test]
    fn committing_a_resolved_composition_sends_its_own_bytes_and_closes_it() {
        let mut state = ImeState::default();
        state.mark("e");
        let sent = state.commit("é");
        assert_eq!(sent, "é".as_bytes());
        assert!(state.marked_range_utf16().is_none());
    }

    #[test]
    fn committing_with_no_composition_open_still_sends_the_text() {
        let mut state = ImeState::default();
        let sent = state.commit("x");
        assert_eq!(sent, b"x");
    }

    #[test]
    fn a_multi_line_commit_sends_carriage_returns_like_enter_does() {
        let mut state = ImeState::default();
        assert_eq!(state.commit("a\nb\r\nc"), b"a\rb\rc");
    }

    #[test]
    fn marking_a_single_unit_composition_reports_its_own_length_not_a_fixed_zero() {
        let mut state = ImeState::default();
        state.mark("e");
        assert_eq!(state.marked_range_utf16(), Some(0..1));
    }

    /// Review finding: a composition longer than one code unit must report its own real
    /// length - AppKit uses this to compute the next `setMarkedText:` call's own
    /// `replacement_range` (module doc). "你好" is two graphemes and, in UTF-16, two code
    /// units (both in the Basic Multilingual Plane) - the review's own named risk (Pinyin/
    /// Hangul assembly) reporting a fixed `0..0` regardless of length.
    #[test]
    fn marking_a_multi_character_composition_reports_its_real_utf16_length() {
        let mut state = ImeState::default();
        state.mark("你好");
        assert_eq!(state.marked_range_utf16(), Some(0..2));
    }

    #[test]
    fn no_composition_open_reports_no_range() {
        let state = ImeState::default();
        assert_eq!(state.marked_range_utf16(), None);
    }
}
