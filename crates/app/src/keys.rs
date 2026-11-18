//! The key-to-bytes mapping (TASK-1.121 plan, section 1): a pure function from a keystroke's
//! own public fields (`gpui::Keystroke`'s `key`, `key_char` and `modifiers`, read from
//! `platform/keystroke.rs:18-33` at the pinned 0.2.2) to the bytes a terminal program
//! expects, or `None` if nothing is sent. No `gpui::Window`, no socket: this module takes no
//! dependency on anything but `gpui::Modifiers`, which is itself plain data.
//!
//! Byte sequences below are the published xterm/ECMA-48 convention (xterm's own `ctlseqs`
//! documentation and the `xterm-256color` terminfo entry's normal, non-application cursor-
//! key mode) and the classic ASCII C0 control-code derivation every terminal emulator uses
//! for Control+letter/punctuation. Nothing here was read from or adapted from Zed's or any
//! other GPL terminal's source (doc-13 R59) - the convention is public, the code is fresh.
//!
//! Named keys are matched against exactly the strings GPUI's own macOS backend produces
//! (`platform/mac/events.rs:296-375`, confirmed against the pinned 0.2.2 source, not
//! guessed): `"enter"`, `"tab"`, `"backspace"`, `"escape"`, `"left"`/`"right"`/`"up"`/
//! `"down"`, `"home"`/`"end"`, `"pageup"`/`"pagedown"`, `"insert"`/`"delete"`, `"f1"`..`"f35"`.

use gpui::Modifiers;

/// Maps one keystroke to the bytes a terminal program expects on its stdin, or `None` if
/// nothing is sent (a Cmd/fn combination, or a key this task doesn't map).
///
/// Priority, in order - each one is why a later one is never reached for that keystroke:
///
/// 1. `modifiers.platform` (Cmd) or `modifiers.function` -> `None`, always: "Cmd combinations
///    never sent to the program" (TASK-1.121 plan). Cmd-V's own paste handling is a separate
///    case, handled one level out before this function is ever called, so this function
///    never special-cases `V`.
/// 2. `modifiers.control` with `key` a single ASCII letter or one of `[`/`\`/`]`/`^`/`_`/`?`/
///    `space` -> the C0 control-code byte. A control combination that doesn't match this
///    shape falls through to the named-key table (step 3) rather than being swallowed here -
///    `Ctrl-Left` reaches step 3 and sends the plain arrow bytes, modifier dropped, not a
///    byte this step claims for itself.
/// 3. A named, non-printable key -> its own fixed byte sequence, regardless of any modifier
///    this task doesn't specially encode (Control, Alt, Shift beyond Tab's own case) - "no
///    xterm modifier-CSI table is built" is a dropped modifier, not a dropped keystroke.
/// 4. Otherwise, if `key_char` is `Some(text)` (a plain printable key, or already composed by
///    the input method before this function ever saw it) -> `text`'s own UTF-8 bytes.
/// 5. Otherwise: `None` - `Ctrl` held with no recognised control byte and no named key (for
///    example `Ctrl-1`) has no established convention to send, so none is guessed at.
pub(crate) fn key_to_bytes(
    key: &str,
    key_char: Option<&str>,
    modifiers: Modifiers,
) -> Option<Vec<u8>> {
    if modifiers.platform || modifiers.function {
        return None;
    }

    if modifiers.control
        && let Some(byte) = control_byte(key)
    {
        return Some(vec![byte]);
    }

    if let Some(bytes) = named_key_bytes(key, modifiers.shift) {
        return Some(bytes);
    }

    if modifiers.control {
        return None;
    }

    key_char.map(|text| text.as_bytes().to_vec())
}

/// The C0 control-code byte for `key` held with Control, or `None` if `key` isn't on that
/// table - the classic derivation (a letter's own code point `& 0x1f`) plus the fixed set of
/// punctuation every terminal gives its own control byte: space (NUL), `[` (ESC, the same
/// byte the bare Escape key sends - a genuine, stated ambiguity inherited from the
/// convention itself), `\` (FS), `]` (GS), `^` (RS), `_` (US), `?` (DEL, the same byte plain
/// Backspace sends, the same kind of ambiguity).
fn control_byte(key: &str) -> Option<u8> {
    match key {
        "space" => Some(0x00),
        "[" => Some(0x1b),
        "\\" => Some(0x1c),
        "]" => Some(0x1d),
        "^" => Some(0x1e),
        "_" => Some(0x1f),
        "?" => Some(0x7f),
        _ => {
            let mut chars = key.chars();
            let only = chars.next()?;
            if chars.next().is_some() || !only.is_ascii_alphabetic() {
                return None;
            }
            Some((only.to_ascii_uppercase() as u8) & 0x1f)
        }
    }
}

/// The fixed byte sequence for a named, non-printable key, or `None` if `key` isn't one of
/// them (a plain printable key falls through to `key_char` instead, in `key_to_bytes`).
/// `shift` only ever changes one entry here (`"tab"`, whose shifted form is "back tab", CSI
/// `Z`) - every other named key's own modifiers are dropped, not encoded (TASK-1.121 plan,
/// section 1: "modifiers on an arrow are dropped, not encoded", generalised here to every
/// named key, since none of them has an xterm modifier-CSI table built yet either).
///
/// A thin delegation to `steg_sdk::named_key_bytes` (TASK-1.90, open question 2(a)): the
/// table itself now lives in the sdk, the one package both this app and the server are
/// allowed to depend on (doc-16), so `pane send --key` and this app's own keyboard handling
/// read one definition rather than two. This function's own name, signature and behaviour
/// are unchanged, so every test below still exercises it exactly as before.
fn named_key_bytes(key: &str, shift: bool) -> Option<Vec<u8>> {
    steg_sdk::named_key_bytes(key, shift)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modifiers(
        control: bool,
        alt: bool,
        shift: bool,
        platform: bool,
        function: bool,
    ) -> Modifiers {
        Modifiers {
            control,
            alt,
            shift,
            platform,
            function,
        }
    }

    fn none() -> Modifiers {
        Modifiers::none()
    }

    /// Every row named in the plan's own table (section 1), as one assertion each, plus a
    /// coverage check (below) that this list has no gaps against what it claims to prove.
    struct Row {
        name: &'static str,
        key: &'static str,
        key_char: Option<&'static str>,
        modifiers: Modifiers,
        expected: Option<&'static [u8]>,
    }

    fn table() -> Vec<Row> {
        vec![
            Row {
                name: "plain letter",
                key: "a",
                key_char: Some("a"),
                modifiers: none(),
                expected: Some(b"a"),
            },
            Row {
                name: "shifted letter",
                key: "a",
                key_char: Some("A"),
                modifiers: modifiers(false, false, true, false, false),
                expected: Some(b"A"),
            },
            Row {
                name: "enter",
                key: "enter",
                key_char: Some("\n"),
                modifiers: none(),
                expected: Some(b"\r"),
            },
            Row {
                name: "tab",
                key: "tab",
                key_char: Some("\t"),
                modifiers: none(),
                expected: Some(b"\t"),
            },
            Row {
                name: "shift-tab",
                key: "tab",
                key_char: None,
                modifiers: modifiers(false, false, true, false, false),
                expected: Some(b"\x1b[Z"),
            },
            Row {
                name: "backspace",
                key: "backspace",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x7f"),
            },
            Row {
                name: "escape",
                key: "escape",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b"),
            },
            Row {
                name: "left",
                key: "left",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[D"),
            },
            Row {
                name: "right",
                key: "right",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[C"),
            },
            Row {
                name: "up",
                key: "up",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[A"),
            },
            Row {
                name: "down",
                key: "down",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[B"),
            },
            Row {
                name: "home",
                key: "home",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[H"),
            },
            Row {
                name: "end",
                key: "end",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[F"),
            },
            Row {
                name: "pageup",
                key: "pageup",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[5~"),
            },
            Row {
                name: "pagedown",
                key: "pagedown",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[6~"),
            },
            Row {
                name: "insert",
                key: "insert",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[2~"),
            },
            Row {
                name: "delete",
                key: "delete",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[3~"),
            },
            Row {
                name: "f1",
                key: "f1",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1bOP"),
            },
            Row {
                name: "f2",
                key: "f2",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1bOQ"),
            },
            Row {
                name: "f3",
                key: "f3",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1bOR"),
            },
            Row {
                name: "f4",
                key: "f4",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1bOS"),
            },
            Row {
                name: "f5",
                key: "f5",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[15~"),
            },
            Row {
                name: "f6",
                key: "f6",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[17~"),
            },
            Row {
                name: "f7",
                key: "f7",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[18~"),
            },
            Row {
                name: "f8",
                key: "f8",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[19~"),
            },
            Row {
                name: "f9",
                key: "f9",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[20~"),
            },
            Row {
                name: "f10",
                key: "f10",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[21~"),
            },
            Row {
                name: "f11",
                key: "f11",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[23~"),
            },
            Row {
                name: "f12",
                key: "f12",
                key_char: None,
                modifiers: none(),
                expected: Some(b"\x1b[24~"),
            },
            Row {
                name: "f13 (dropped)",
                key: "f13",
                key_char: None,
                modifiers: none(),
                expected: None,
            },
            Row {
                name: "ctrl-a",
                key: "a",
                key_char: None,
                modifiers: modifiers(true, false, false, false, false),
                expected: Some(b"\x01"),
            },
            Row {
                name: "ctrl-z",
                key: "z",
                key_char: None,
                modifiers: modifiers(true, false, false, false, false),
                expected: Some(b"\x1a"),
            },
            Row {
                name: "ctrl-space",
                key: "space",
                key_char: Some(" "),
                modifiers: modifiers(true, false, false, false, false),
                expected: Some(b"\x00"),
            },
            Row {
                name: "ctrl-[",
                key: "[",
                key_char: None,
                modifiers: modifiers(true, false, false, false, false),
                expected: Some(b"\x1b"),
            },
            Row {
                name: "ctrl-\\",
                key: "\\",
                key_char: None,
                modifiers: modifiers(true, false, false, false, false),
                expected: Some(b"\x1c"),
            },
            Row {
                name: "ctrl-]",
                key: "]",
                key_char: None,
                modifiers: modifiers(true, false, false, false, false),
                expected: Some(b"\x1d"),
            },
            Row {
                name: "ctrl-^",
                key: "^",
                key_char: None,
                modifiers: modifiers(true, false, false, false, false),
                expected: Some(b"\x1e"),
            },
            Row {
                name: "ctrl-_",
                key: "_",
                key_char: None,
                modifiers: modifiers(true, false, false, false, false),
                expected: Some(b"\x1f"),
            },
            Row {
                name: "ctrl-?",
                key: "?",
                key_char: None,
                modifiers: modifiers(true, false, false, false, false),
                expected: Some(b"\x7f"),
            },
            Row {
                name: "ctrl-1 (dropped, no convention)",
                key: "1",
                key_char: None,
                modifiers: modifiers(true, false, false, false, false),
                expected: None,
            },
            Row {
                name: "ctrl-left (modifier dropped, arrow still sent)",
                key: "left",
                key_char: None,
                modifiers: modifiers(true, false, false, false, false),
                expected: Some(b"\x1b[D"),
            },
            Row {
                name: "cmd-anything (never sent)",
                key: "c",
                key_char: Some("c"),
                modifiers: modifiers(false, false, false, true, false),
                expected: None,
            },
            Row {
                name: "cmd-left (never sent, not even the arrow)",
                key: "left",
                key_char: None,
                modifiers: modifiers(false, false, false, true, false),
                expected: None,
            },
            Row {
                name: "fn-anything (dropped)",
                key: "f",
                key_char: Some("f"),
                modifiers: modifiers(false, false, false, false, true),
                expected: None,
            },
        ]
    }

    #[test]
    fn every_row_in_the_plans_own_table_maps_as_the_plan_says() {
        for row in table() {
            let actual = key_to_bytes(row.key, row.key_char, row.modifiers);
            assert_eq!(
                actual.as_deref(),
                row.expected,
                "row {:?}: key={:?} key_char={:?} modifiers={:?}",
                row.name,
                row.key,
                row.key_char,
                row.modifiers,
            );
        }
    }

    /// Every key string this test asks `named_key_bytes` itself about - a superset of what
    /// it currently maps (`f13` is deliberately included as a dropped boundary case, module
    /// doc's own "f13 and above -> None"), never a parallel claim about what the mapping
    /// *should* contain. The real question this test asks, per key, is only ever "does
    /// `named_key_bytes` say `Some` here, and does the table have a row proving it" - driven
    /// by the function's own real output, not a second, independently maintained list
    /// (review finding: the previous version compared this fixture's own hardcoded list
    /// against itself, so it could not fail when `named_key_bytes` lost or gained a key).
    const CANDIDATE_NAMED_KEYS: &[&str] = &[
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
        "f13",
    ];

    /// For every candidate key, `named_key_bytes`'s own real answer ("is this mapped at
    /// all") must agree with whether the table has a row proving it - a row whose own
    /// `expected` is `Some`, not merely a row that mentions the key (the existing "f13
    /// (dropped)" row names `key: "f13"` precisely to prove it is *not* mapped, and must not
    /// itself count as proof that it is). If `named_key_bytes` ever loses a mapping this
    /// table still claims, or gains one this table never proves, the two sides disagree and
    /// this fails - not a hand-maintained list repeating what the match arms already say.
    #[test]
    fn every_key_named_key_bytes_maps_has_a_row_proving_it_and_no_other_key_does() {
        let rows = table();
        for key in CANDIDATE_NAMED_KEYS {
            let really_mapped = named_key_bytes(key, false).is_some();
            let table_claims_it_is_mapped = rows
                .iter()
                .any(|row| row.key == *key && row.expected.is_some());
            assert_eq!(
                really_mapped, table_claims_it_is_mapped,
                "named_key_bytes({key:?}, false).is_some() = {really_mapped}, but the table's \
                 own claim that it is mapped = {table_claims_it_is_mapped} - these must agree",
            );
        }
    }
}
