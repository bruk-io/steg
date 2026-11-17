//! The minimum state a terminal's input handler tracks for IME composition (TASK-1.121
//! plan, section 2). There is no editable text buffer here - unlike `gpui`'s own
//! `examples/input.rs` (`TextInput`, a real text field with a `content: SharedString`), a
//! terminal has no "range of the document" for the input method to read or replace into.
//! This tracks only what composition itself needs: whether one is open, and what to send
//! once it resolves. Marked (uncommitted) text is never sent to the program - only
//! `commit`'s own, resolved text is.

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

    /// Whether a composition is open. This is what `EntityInputHandler::marked_text_range`
    /// answers from, and what makes GPUI's own macOS backend route the *next* keystroke to
    /// the input context instead of this app's raw key handler
    /// (`platform/mac/window.rs:1696-1699`) - the other half of the dedup rule in
    /// `keys.rs`'s own doc comment: GPUI never sends the same keystroke to both paths, and
    /// this is the state it reads to decide which one.
    pub(crate) fn is_composing(&self) -> bool {
        self.marked.is_some()
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
        assert!(!state.is_composing());
        state.mark("e");
        assert!(state.is_composing());
    }

    #[test]
    fn marking_empty_text_closes_the_composition() {
        let mut state = ImeState::default();
        state.mark("e");
        state.mark("");
        assert!(!state.is_composing());
    }

    #[test]
    fn unmarking_closes_the_composition() {
        let mut state = ImeState::default();
        state.mark("e");
        state.unmark();
        assert!(!state.is_composing());
    }

    #[test]
    fn committing_a_resolved_composition_sends_its_own_bytes_and_closes_it() {
        let mut state = ImeState::default();
        state.mark("e");
        let sent = state.commit("é");
        assert_eq!(sent, "é".as_bytes());
        assert!(!state.is_composing());
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
}
