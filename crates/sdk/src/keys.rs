//! The named-key table doc-2's `pane send --key` and the app's own
//! keyboard handling both need (TASK-1.90, open question 2(a)): one
//! table, in the one package both are allowed to depend on (doc-16's own
//! dependency table - `app` may depend on `sdk` and `ui` only, never the
//! kernel; `server` depends on the kernel and `sdk` - so `sdk` is the
//! only package that removes the duplication rather than moving it).
//!
//! This is a straight lift of `crates/app/src/keys.rs`'s own
//! `named_key_bytes` match arms, with the same byte sequences and the
//! same key names - a move, not a rewrite. `crates/app/src/keys.rs`'s
//! own `key_to_bytes` (the `gpui::Modifiers`/`key_char` priority logic)
//! and `control_byte` (the `Ctrl`+letter C0 derivation) stay in
//! `crates/app`, out of this task's scope: doc-2's `--key <name>` grammar
//! names no modifier syntax at all, so a shifted or control-held key
//! reaches a program only through the app's own real keystroke, never
//! through `pane send --key`.
//!
//! No I/O, no `gpui` type, in keeping with the sdk's own rules (doc-16
//! R102, R106).

/// The fixed byte sequence for a named, non-printable key, or `None` if
/// `name` isn't one of them. `shift` only ever changes one entry
/// (`"tab"`, whose shifted form is "back tab", CSI `Z`) - every other
/// named key's own modifiers are dropped, not encoded, the same choice
/// `crates/app/src/keys.rs`'s own copy of this function already made and
/// still documents for its own callers.
///
/// `shift` is always `false` from `pane send --key` (doc-2's own
/// grammar has no modifier syntax for it) - kept as a parameter, not
/// dropped, so this one function still serves the app's own real
/// Shift+Tab keystroke unchanged.
#[must_use]
pub fn named_key_bytes(name: &str, shift: bool) -> Option<Vec<u8>> {
    let bytes: &[u8] = match name {
        "enter" => b"\r",
        "tab" if shift => b"\x1b[Z",
        "tab" => b"\t",
        "backspace" => b"\x7f",
        "escape" => b"\x1b",
        "left" => b"\x1b[D",
        "right" => b"\x1b[C",
        "up" => b"\x1b[A",
        "down" => b"\x1b[B",
        "home" => b"\x1b[H",
        "end" => b"\x1b[F",
        "pageup" => b"\x1b[5~",
        "pagedown" => b"\x1b[6~",
        "insert" => b"\x1b[2~",
        "delete" => b"\x1b[3~",
        "f1" => b"\x1bOP",
        "f2" => b"\x1bOQ",
        "f3" => b"\x1bOR",
        "f4" => b"\x1bOS",
        "f5" => b"\x1b[15~",
        "f6" => b"\x1b[17~",
        "f7" => b"\x1b[18~",
        "f8" => b"\x1b[19~",
        "f9" => b"\x1b[20~",
        "f10" => b"\x1b[21~",
        "f11" => b"\x1b[23~",
        "f12" => b"\x1b[24~",
        _ => return None,
    };
    Some(bytes.to_vec())
}

/// Every name [`named_key_bytes`] maps (with `shift: false`, the only
/// form `pane send --key` can ever ask for) - the closed set doc-2's
/// `--key <name>` names, and the source `pane.send`'s own
/// `ValueKind::Choice` is built from (crates/sdk/src/description/methods/
/// pane.rs), so the description and this table can never drift apart.
/// Order matches doc-2's own listing.
#[must_use]
pub fn key_names() -> Vec<String> {
    [
        "enter",
        "tab",
        "backspace",
        "escape",
        "left",
        "right",
        "up",
        "down",
        "home",
        "end",
        "pageup",
        "pagedown",
        "insert",
        "delete",
        "f1",
        "f2",
        "f3",
        "f4",
        "f5",
        "f6",
        "f7",
        "f8",
        "f9",
        "f10",
        "f11",
        "f12",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::{key_names, named_key_bytes};

    /// Every name [`key_names`] lists really maps to something
    /// (`named_key_bytes(name, false)` is never `None` for one of them) -
    /// the same "no drift" guard `crates/app/src/keys.rs`'s own
    /// `every_key_named_key_bytes_maps_has_a_row_proving_it_and_no_other_
    /// key_does` keeps for its own copy, checked here from the other
    /// direction since this module's own public surface is the name list
    /// itself, not a hand-maintained candidate list to compare against.
    #[test]
    fn every_name_key_names_lists_really_maps_to_bytes() {
        for name in key_names() {
            assert!(
                named_key_bytes(&name, false).is_some(),
                "{name:?} is in key_names() but named_key_bytes maps nothing for it"
            );
        }
    }

    /// A name outside the list maps to nothing - the closed-set property
    /// `pane.send`'s own `ValueKind::Choice` relies on `key_names()` for;
    /// this function's own refusal is the second half of that same
    /// guarantee.
    #[test]
    fn an_unlisted_name_maps_to_nothing() {
        assert_eq!(named_key_bytes("not-a-real-key", false), None);
        assert_eq!(named_key_bytes("f13", false), None);
        assert_eq!(named_key_bytes("ctrl-c", false), None);
    }

    /// Every byte sequence, checked literally - not only "is Some", since
    /// a wrong escape sequence would still be "Some" and pass the test
    /// above. One assertion per key doc-2 lists, in the same order.
    #[test]
    fn every_named_key_maps_to_its_documented_bytes() {
        let cases: &[(&str, &[u8])] = &[
            ("enter", b"\r"),
            ("tab", b"\t"),
            ("backspace", b"\x7f"),
            ("escape", b"\x1b"),
            ("left", b"\x1b[D"),
            ("right", b"\x1b[C"),
            ("up", b"\x1b[A"),
            ("down", b"\x1b[B"),
            ("home", b"\x1b[H"),
            ("end", b"\x1b[F"),
            ("pageup", b"\x1b[5~"),
            ("pagedown", b"\x1b[6~"),
            ("insert", b"\x1b[2~"),
            ("delete", b"\x1b[3~"),
            ("f1", b"\x1bOP"),
            ("f2", b"\x1bOQ"),
            ("f3", b"\x1bOR"),
            ("f4", b"\x1bOS"),
            ("f5", b"\x1b[15~"),
            ("f6", b"\x1b[17~"),
            ("f7", b"\x1b[18~"),
            ("f8", b"\x1b[19~"),
            ("f9", b"\x1b[20~"),
            ("f10", b"\x1b[21~"),
            ("f11", b"\x1b[23~"),
            ("f12", b"\x1b[24~"),
        ];
        for (name, expected) in cases {
            assert_eq!(
                named_key_bytes(name, false).as_deref(),
                Some(*expected),
                "{name:?}"
            );
        }
    }

    /// `shift` only changes `"tab"` - every other named key's modifiers
    /// stay dropped, matching `crates/app/src/keys.rs`'s own documented
    /// choice for the identical table.
    #[test]
    fn shift_only_changes_tab() {
        assert_eq!(
            named_key_bytes("tab", true).as_deref(),
            Some(&b"\x1b[Z"[..])
        );
        assert_eq!(named_key_bytes("tab", false).as_deref(), Some(&b"\t"[..]));
        assert_eq!(
            named_key_bytes("left", true).as_deref(),
            named_key_bytes("left", false).as_deref(),
            "shift must not change a key other than tab"
        );
    }
}
