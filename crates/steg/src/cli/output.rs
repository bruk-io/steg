//! The plain-text / `--json` / `-q` renderer (TASK-1.120): one pure
//! function from a method's declared [`Returns`] (steg-sdk's own
//! description) plus the raw reply [`Value`] to the bytes doc-2 section 6
//! wants - no server, no clap, no sdk connection types in [`render`]'s own
//! signature, so every case is a unit test with no process involved.
//! [`crate::cli::connect::run`] is the one caller; it writes [`render`]'s
//! bytes to stdout (or none, for an empty `Vec`) and keeps its own
//! `eprintln!`/exit-status job for the `Err` case.
//!
//! Matched exhaustively on `Returns`'s six variants - the workspace's own
//! `clippy::wildcard_enum_match_arm` lint (`#![deny(warnings)]` via
//! `just lint`) already refuses a catch-all arm here, so a seventh
//! `Returns` variant fails the build with no extra guard needed in this
//! module to make that true (R32's own "no catch-all" criterion).

use serde_json::{Map, Value};
use steg_sdk::{Field, Returns};

/// The server's reply did not have the shape its own method declared -
/// never a panic (see the module doc). Should never happen in practice
/// (one [`steg_sdk::Description`] compiled into both the server and this
/// binary); proved defensively in this module's own tests with a
/// hand-built mismatched [`Value`], never reachable through a real
/// server. One variant per shape a caller could tell apart (`.claude/
/// rules/rust.md`: "one variant per thing a caller can tell apart"),
/// each carrying the value that actually arrived as data, not folded
/// into a pre-built `String` - what actually declared each shape is
/// named by which [`render`] call site constructs the variant (the
/// doc comment on each), not repeated as a field here.
#[derive(Debug, PartialEq, Eq)]
pub enum RenderError {
    /// `Returns::Identifier`'s own reply did not carry a string field
    /// named `field` (always `"id"`, today).
    MissingStringField { field: &'static str, value: Value },
    /// `Returns::Record`'s own reply, or one element of
    /// `Returns::Records`'s own reply, was not a JSON object.
    NotAnObject { value: Value },
    /// `Returns::Records`'s own reply was not a JSON array.
    NotAnArray { value: Value },
    /// `Returns::Text`'s own reply was not a JSON string.
    NotAString { value: Value },
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (expected, value) = match self {
            Self::MissingStringField { field, value } => {
                (format!("a string field {field:?}"), value)
            }
            Self::NotAnObject { value } => ("a JSON object".to_owned(), value),
            Self::NotAnArray { value } => ("a JSON array".to_owned(), value),
            Self::NotAString { value } => ("a JSON string".to_owned(), value),
        };
        write!(
            f,
            "the server's reply did not match its own declared shape: expected {expected}, got \
             {value}"
        )
    }
}

impl std::error::Error for RenderError {}

/// Renders one method's reply to the bytes its three output forms
/// produce (doc-2 section 6): `json` is `--json`, `quiet` is `-q`/
/// `--quiet`. Matched exhaustively on `returns` - see the module doc.
pub fn render(
    returns: &Returns,
    value: &Value,
    json: bool,
    quiet: bool,
) -> Result<Vec<u8>, RenderError> {
    match returns {
        Returns::Nothing => render_nothing(),
        Returns::Identifier { .. } => render_identifier(value, json),
        Returns::Record { fields } => render_record(fields, value, json, quiet),
        Returns::Records { fields } => render_records(fields, value, json, quiet),
        Returns::Text => render_text(value, json, quiet),
        Returns::Nested { .. } => render_nested(value),
    }
}

/// `Returns::Nothing`: silent in every combination of `--json`/`-q`
/// (decision 3: "an action's reply prints nothing with `--json` too", not
/// today's pre-this-task raw passthrough, which no test ever locked in).
fn render_nothing() -> Result<Vec<u8>, RenderError> {
    Ok(Vec::new())
}

/// `Returns::Identifier`: the reply is `{"id": "..."}` (confirmed against
/// `cli_exit_statuses.rs`'s own existing JSON-round-trip test). `--json`
/// prints that object unchanged; otherwise (`-q` or not - doc-2 section 5:
/// "where it prints one new identifier, that is unchanged") the bare id
/// alone on a line.
fn render_identifier(value: &Value, json: bool) -> Result<Vec<u8>, RenderError> {
    if json {
        return Ok(json_line(value));
    }
    let id = string_field(value, "id")?;
    Ok(line(&id))
}

/// `Returns::Record`: one object. `--json` prints it unchanged; plain
/// text is one tab-separated line, `fields`-order columns (decision 6:
/// control characters replaced with a single space); `-q` reduces to
/// `fields[0]`'s own value alone (decision 4) - checked directly: `"id"`
/// is already `fields[0]` everywhere a record has one (`session`/`tab`/
/// `pane`/`client`'s own `_fields()` functions), so this is a
/// generalisation of "the full identifier," not a second rule. A record
/// with no declared fields at all prints nothing, in every form - not
/// reachable through any method described today, proved only by this
/// module's own unit tests with a hand-built empty `fields` list.
fn render_record(
    fields: &[Field],
    value: &Value,
    json: bool,
    quiet: bool,
) -> Result<Vec<u8>, RenderError> {
    if fields.is_empty() {
        return Ok(Vec::new());
    }
    let object = expect_object(value)?;
    if quiet {
        let first = field_value(object, &fields[0].name);
        return Ok(if json {
            json_line(&first)
        } else {
            line(&scalar_text(&first))
        });
    }
    if json {
        return Ok(json_line(value));
    }
    Ok(line(&record_row(fields, object)))
}

/// `Returns::Records`: one array of objects. `--json` prints it
/// unchanged; plain text is one tab-separated line per element, nothing
/// for zero elements; `-q` prints `fields[0]`'s value per line (one JSON
/// array of them together with `--json`, doc-2 section 5's own
/// orchestrator-filled note, generalised the same way `render_record`'s
/// own doc explains). Zero declared fields: nothing, for the same reason
/// and with the same caveat as `render_record`.
fn render_records(
    fields: &[Field],
    value: &Value,
    json: bool,
    quiet: bool,
) -> Result<Vec<u8>, RenderError> {
    if fields.is_empty() {
        return Ok(Vec::new());
    }
    let elements = expect_array(value)?;
    let objects: Vec<&Map<String, Value>> = elements
        .iter()
        .map(expect_object)
        .collect::<Result<_, _>>()?;

    if quiet {
        let firsts: Vec<Value> = objects
            .iter()
            .map(|object| field_value(object, &fields[0].name))
            .collect();
        return Ok(if json {
            json_line(&Value::Array(firsts))
        } else {
            lines(firsts.iter().map(scalar_text))
        });
    }
    if json {
        return Ok(json_line(value));
    }
    Ok(lines(
        objects.iter().map(|object| record_row(fields, object)),
    ))
}

/// `Returns::Text`: free text (`pane capture`'s own screen text is the
/// one method reaching this through the real tree - `api.call`'s own
/// `Returns::Text` never passes through this renderer at all, since
/// `main.rs` special-cases it to `api_call.rs::run` directly). Plain text
/// is the string's own bytes, unaltered - no control-character
/// replacement (decision 6 is about *fields in a tab-separated row*;
/// nothing here is a column to keep tab-separated) and no newline added
/// or removed, so a captured screen's own exact text survives. `--json`
/// prints the value (a JSON string) unchanged. `-q` has no reduction
/// (decision 5) - ignored, either way.
fn render_text(value: &Value, json: bool, _quiet: bool) -> Result<Vec<u8>, RenderError> {
    if json {
        return Ok(json_line(value));
    }
    let Value::String(text) = value else {
        return Err(RenderError::NotAString {
            value: value.clone(),
        });
    };
    Ok(text.clone().into_bytes())
}

/// `Returns::Nested`: prints as JSON unconditionally (decision 1) - the
/// reply has no `fields` to flatten into a tab-separated row at all
/// (`shapes.rs`'s own `Nested { of, summary }` carries none), so plain
/// text and `--json` are the same bytes for every `Nested` method
/// (`pane.attach`, `event.watch`'s own one-shot reply) - doc-2 section 3's
/// `pane attach` row already said this for one of them explicitly
/// ("as JSON", not gated on `--json`); decision 1 generalises it. `-q` has
/// nothing left to reduce once plain text already *is* the whole reply -
/// ignored, the same reasoning `render_text` already has for `Text`.
fn render_nested(value: &Value) -> Result<Vec<u8>, RenderError> {
    Ok(json_line(value))
}

/// `value` serialised compactly, with the one trailing newline every
/// `--json`/`Nested` output already carried before this task (the
/// pre-existing `println!("{value}")` this module replaces).
///
/// # Panics
/// Never in practice: serialising a [`Value`] that was itself built by
/// parsing valid JSON cannot fail - there is no custom `Serialize` impl
/// here that could return an error.
fn json_line(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_string(value)
        .unwrap_or_else(|err| panic!("serialising a serde_json::Value cannot fail: {err}"))
        .into_bytes();
    bytes.push(b'\n');
    bytes
}

/// One plain-text line, `text` followed by a single newline.
fn line(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(b'\n');
    bytes
}

/// Zero or more plain-text lines, each followed by a newline - the whole
/// `Vec` is empty for an empty iterator, never a single blank line
/// (`Returns::Records` with zero elements prints nothing, doc-2 section
/// 6's own "nothing printed for zero elements").
fn lines(texts: impl Iterator<Item = String>) -> Vec<u8> {
    let mut bytes = Vec::new();
    for text in texts {
        bytes.extend_from_slice(text.as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

/// One record's own tab-separated row, `fields`-order columns.
fn record_row(fields: &[Field], object: &Map<String, Value>) -> String {
    fields
        .iter()
        .map(|field| scalar_text(&field_value(object, &field.name)))
        .collect::<Vec<_>>()
        .join("\t")
}

/// `object`'s own value at `name`, or `Value::Null` when the key is
/// absent (`pane_fields.exit_code` before a program has ended, for
/// instance) - a missing field is an empty column, never a dropped one
/// (doc-2 section 6's fixed column count).
fn field_value(object: &Map<String, Value>, name: &str) -> Value {
    object.get(name).cloned().unwrap_or(Value::Null)
}

/// One field's own plain-text column - matches the real [`Value`]
/// variant directly, never the field's own declared `ValueKind`. This is
/// what renders `tab_fields`'s `layout`/`shown_status` and `pane_fields`'s
/// `status`/`size` correctly with no per-field-name special case: each is
/// declared `ValueKind::Text` but its real reply value is an object, an
/// array, or null (their own doc comments in `methods/tab.rs`/`pane.rs`
/// say so) - matching the real `Value` handles every one of those for
/// free. Decision 6: any `\t`/`\n`/`\r` inside the result becomes a
/// single space, so "tab-separated" and "one record per line" hold
/// unconditionally, not merely in the common case.
fn scalar_text(value: &Value) -> String {
    let raw = match value {
        Value::Null => String::new(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value)
            .unwrap_or_else(|err| panic!("serialising a serde_json::Value cannot fail: {err}")),
    };
    sanitize_for_plain_text(&raw)
}

fn sanitize_for_plain_text(raw: &str) -> String {
    raw.chars()
        .map(|character| match character {
            '\t' | '\n' | '\r' => ' ',
            other => other,
        })
        .collect()
}

/// `value.get(name)`'s own string, or a shape error naming both why
/// (`Returns::Identifier`'s own reply is declared as `{"id": "..."}"`,
/// checked here rather than assumed).
fn string_field(value: &Value, name: &'static str) -> Result<String, RenderError> {
    value
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| RenderError::MissingStringField {
            field: name,
            value: value.clone(),
        })
}

fn expect_object(value: &Value) -> Result<&Map<String, Value>, RenderError> {
    value.as_object().ok_or_else(|| RenderError::NotAnObject {
        value: value.clone(),
    })
}

fn expect_array(value: &Value) -> Result<&Vec<Value>, RenderError> {
    value.as_array().ok_or_else(|| RenderError::NotAnArray {
        value: value.clone(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use steg_sdk::{Field, Returns, ValueKind};

    use super::render;

    fn field(name: &str, kind: ValueKind) -> Field {
        Field {
            name: name.to_owned(),
            value: kind,
            summary: String::new(),
        }
    }

    fn id_label_fields() -> Vec<Field> {
        vec![
            field("id", ValueKind::Text),
            field("label", ValueKind::Text),
        ]
    }

    fn as_text(bytes: Vec<u8>) -> String {
        String::from_utf8(bytes).expect("render() only ever produces UTF-8 text")
    }

    // --- Nothing -----------------------------------------------------------

    #[test]
    fn nothing_prints_nothing_in_every_combination() {
        let value = Value::Null;
        for json in [false, true] {
            for quiet in [false, true] {
                let bytes = render(&Returns::Nothing, &value, json, quiet).unwrap();
                assert!(bytes.is_empty(), "json={json} quiet={quiet}: {bytes:?}");
            }
        }
    }

    // --- Identifier ----------------------------------------------------------

    #[test]
    fn identifier_plain_prints_the_bare_id_alone() {
        let returns = Returns::Identifier { of: "pane".into() };
        let value = json!({"id": "abc123"});
        let bytes = render(&returns, &value, false, false).unwrap();
        assert_eq!(as_text(bytes), "abc123\n");
    }

    #[test]
    fn identifier_json_prints_the_object_unchanged() {
        let returns = Returns::Identifier { of: "pane".into() };
        let value = json!({"id": "abc123"});
        let bytes = render(&returns, &value, true, false).unwrap();
        assert_eq!(as_text(bytes), "{\"id\":\"abc123\"}\n");
    }

    /// Decision 5's own wording ("that is unchanged"): `-q` makes no
    /// difference to `Identifier`, plain or `--json`.
    #[test]
    fn identifier_quiet_is_the_same_as_without_quiet_either_way() {
        let returns = Returns::Identifier { of: "pane".into() };
        let value = json!({"id": "abc123"});
        for json_flag in [false, true] {
            let with_quiet = render(&returns, &value, json_flag, true).unwrap();
            let without_quiet = render(&returns, &value, json_flag, false).unwrap();
            assert_eq!(with_quiet, without_quiet, "json={json_flag}");
        }
    }

    #[test]
    fn identifier_with_no_id_field_is_a_shape_error_not_a_panic() {
        let returns = Returns::Identifier { of: "pane".into() };
        let value = json!({"not_id": "abc123"});
        assert!(render(&returns, &value, false, false).is_err());
    }

    // --- Record --------------------------------------------------------------

    #[test]
    fn record_plain_is_one_tab_separated_line_in_fields_order() {
        let returns = Returns::Record {
            fields: id_label_fields(),
        };
        let value = json!({"id": "s1", "label": "work"});
        let bytes = render(&returns, &value, false, false).unwrap();
        assert_eq!(as_text(bytes), "s1\twork\n");
    }

    #[test]
    fn record_json_prints_the_object_unchanged() {
        let returns = Returns::Record {
            fields: id_label_fields(),
        };
        let value = json!({"id": "s1", "label": "work"});
        let bytes = render(&returns, &value, true, false).unwrap();
        assert_eq!(as_text(bytes), "{\"id\":\"s1\",\"label\":\"work\"}\n");
    }

    /// Decision 4: `-q` on a record reduces to `fields[0]`'s own value -
    /// here `"id"`, since it is declared first.
    #[test]
    fn record_quiet_prints_fields_zero_alone() {
        let returns = Returns::Record {
            fields: id_label_fields(),
        };
        let value = json!({"id": "s1", "label": "work"});
        let bytes = render(&returns, &value, false, true).unwrap();
        assert_eq!(as_text(bytes), "s1\n");
    }

    /// Decision 4 applied to a record whose first field is not literally
    /// named `"id"` (`setting_fields`'s own `"name"`, `pane.wait`'s own
    /// `"exit_code"`) - `-q` still reduces to `fields[0]`, no lookup for
    /// a field named `"id"` anywhere in this module.
    #[test]
    fn record_quiet_with_no_id_field_prints_the_first_declared_field() {
        let returns = Returns::Record {
            fields: vec![
                field("exit_code", ValueKind::Integer),
                field("program", ValueKind::Text),
            ],
        };
        let value = json!({"exit_code": 0, "program": "/bin/cat"});
        let bytes = render(&returns, &value, false, true).unwrap();
        assert_eq!(as_text(bytes), "0\n");
    }

    #[test]
    fn record_quiet_with_json_prints_fields_zero_as_json() {
        let returns = Returns::Record {
            fields: id_label_fields(),
        };
        let value = json!({"id": "s1", "label": "work"});
        let bytes = render(&returns, &value, true, true).unwrap();
        assert_eq!(as_text(bytes), "\"s1\"\n");
    }

    /// Decision 4's second clause, proved only here - no described method
    /// has zero fields today, so the real binary can never reach this.
    #[test]
    fn record_with_no_declared_fields_prints_nothing_in_every_combination() {
        let returns = Returns::Record { fields: Vec::new() };
        let value = json!({"anything": "at all"});
        for json_flag in [false, true] {
            for quiet in [false, true] {
                let bytes = render(&returns, &value, json_flag, quiet).unwrap();
                assert!(
                    bytes.is_empty(),
                    "json={json_flag} quiet={quiet}: {bytes:?}"
                );
            }
        }
    }

    #[test]
    fn record_with_a_non_object_reply_is_a_shape_error_not_a_panic() {
        let returns = Returns::Record {
            fields: id_label_fields(),
        };
        let value = json!(["not", "an", "object"]);
        assert!(render(&returns, &value, false, false).is_err());
    }

    // --- Records ---------------------------------------------------------------

    #[test]
    fn records_plain_is_one_line_per_record_nothing_for_zero() {
        let returns = Returns::Records {
            fields: id_label_fields(),
        };
        let empty = json!([]);
        assert!(render(&returns, &empty, false, false).unwrap().is_empty());

        let two = json!([
            {"id": "s1", "label": "work"},
            {"id": "s2", "label": "play"},
        ]);
        let bytes = render(&returns, &two, false, false).unwrap();
        assert_eq!(as_text(bytes), "s1\twork\ns2\tplay\n");
    }

    #[test]
    fn records_json_prints_the_array_unchanged() {
        let returns = Returns::Records {
            fields: id_label_fields(),
        };
        let value = json!([{"id": "s1", "label": "work"}]);
        let bytes = render(&returns, &value, true, false).unwrap();
        assert_eq!(as_text(bytes), "[{\"id\":\"s1\",\"label\":\"work\"}]\n");
    }

    #[test]
    fn records_quiet_prints_fields_zero_per_line() {
        let returns = Returns::Records {
            fields: id_label_fields(),
        };
        let value = json!([
            {"id": "s1", "label": "work"},
            {"id": "s2", "label": "play"},
        ]);
        let bytes = render(&returns, &value, false, true).unwrap();
        assert_eq!(as_text(bytes), "s1\ns2\n");
    }

    /// Decision 4 + doc-2 section 5's own orchestrator-filled note,
    /// combined: `-q` together with `--json` on a list is one JSON array
    /// of the reduced values.
    #[test]
    fn records_quiet_with_json_is_one_json_array_of_fields_zero() {
        let returns = Returns::Records {
            fields: id_label_fields(),
        };
        let value = json!([
            {"id": "s1", "label": "work"},
            {"id": "s2", "label": "play"},
        ]);
        let bytes = render(&returns, &value, true, true).unwrap();
        assert_eq!(as_text(bytes), "[\"s1\",\"s2\"]\n");
    }

    #[test]
    fn records_with_no_declared_fields_prints_nothing_in_every_combination() {
        let returns = Returns::Records { fields: Vec::new() };
        let value = json!([{"anything": "at all"}, {"more": "still nothing"}]);
        for json_flag in [false, true] {
            for quiet in [false, true] {
                let bytes = render(&returns, &value, json_flag, quiet).unwrap();
                assert!(
                    bytes.is_empty(),
                    "json={json_flag} quiet={quiet}: {bytes:?}"
                );
            }
        }
    }

    #[test]
    fn records_with_a_non_array_reply_is_a_shape_error_not_a_panic() {
        let returns = Returns::Records {
            fields: id_label_fields(),
        };
        let value = json!({"not": "an array"});
        assert!(render(&returns, &value, false, false).is_err());
    }

    #[test]
    fn records_with_a_non_object_element_is_a_shape_error_not_a_panic() {
        let returns = Returns::Records {
            fields: id_label_fields(),
        };
        let value = json!(["not an object"]);
        assert!(render(&returns, &value, false, false).is_err());
    }

    // --- The one field-value design that removes a whole class of special-casing ---

    /// `tab_fields`'s `layout`/`shown_status`, `pane_fields`'s `status`/
    /// `size`: declared `ValueKind::Text` but really structured JSON -
    /// rendered correctly with no per-field-name code, because this
    /// module matches the real `Value`, never the declared `ValueKind`.
    #[test]
    fn a_field_declared_text_but_really_structured_renders_its_own_compact_json() {
        let returns = Returns::Record {
            fields: vec![
                field("id", ValueKind::Text),
                field("layout", ValueKind::Text),
            ],
        };
        let value = json!({"id": "t1", "layout": {"split": "horizontal", "panes": ["p1", "p2"]}});
        let bytes = render(&returns, &value, false, false).unwrap();
        assert_eq!(
            as_text(bytes),
            "t1\t{\"split\":\"horizontal\",\"panes\":[\"p1\",\"p2\"]}\n"
        );
    }

    #[test]
    fn a_null_field_renders_as_an_empty_column_never_a_dropped_one() {
        let returns = Returns::Record {
            fields: vec![
                field("id", ValueKind::Text),
                field("exit_code", ValueKind::Integer),
            ],
        };
        // The key is absent entirely, not merely null - `pane.exit_code`
        // before the program has ended (doc-2 section 6's fixed column
        // count: a missing field is an empty column, never one fewer tab).
        let value = json!({"id": "p1"});
        let bytes = render(&returns, &value, false, false).unwrap();
        assert_eq!(as_text(bytes), "p1\t\n");
    }

    /// Decision 6: a literal tab, newline and carriage return inside a
    /// field's own value each become a single space in plain text, so
    /// "tab-separated" and "one record per line" hold unconditionally -
    /// `pane_fields.title`, for instance, comes from the program and is
    /// not otherwise bounded. A record (not `Records`) is enough to
    /// prove the substitution itself; `scalar_text` is shared by both.
    ///
    /// Negative control, performed by hand and reverted before
    /// committing (not left as a second `#[test]`): with
    /// `sanitize_for_plain_text` changed to return `raw.to_owned()`
    /// unconditionally (a no-op), this test fails - `label`'s own
    /// column keeps its literal `\t`/`\n`/`\r` instead of spaces, so the
    /// row has more than four tab-separated fields and the assertion
    /// below does not match.
    #[test]
    fn a_tab_newline_and_carriage_return_inside_a_field_become_single_spaces_in_plain_text() {
        let returns = Returns::Record {
            fields: vec![
                field("id", ValueKind::Text),
                field("label", ValueKind::Text),
            ],
        };
        let value = json!({"id": "p1", "label": "a\tb\nc\rd"});
        let bytes = render(&returns, &value, false, false).unwrap();
        assert_eq!(as_text(bytes), "p1\ta b c d\n");
    }

    /// Decision 6's other half: `--json` keeps the exact value, every
    /// control character included - `serde_json`'s own escaping is that
    /// format's business, not this module's to touch.
    #[test]
    fn a_tab_newline_and_carriage_return_inside_a_field_are_exact_under_json() {
        let returns = Returns::Record {
            fields: vec![
                field("id", ValueKind::Text),
                field("label", ValueKind::Text),
            ],
        };
        let value = json!({"id": "p1", "label": "a\tb\nc\rd"});
        let bytes = render(&returns, &value, true, false).unwrap();
        assert_eq!(
            as_text(bytes),
            "{\"id\":\"p1\",\"label\":\"a\\tb\\nc\\rd\"}\n"
        );
    }

    // --- Text ------------------------------------------------------------------

    #[test]
    fn text_plain_is_the_string_unaltered() {
        let value = json!("line one\nline two\n");
        let bytes = render(&Returns::Text, &value, false, false).unwrap();
        assert_eq!(as_text(bytes), "line one\nline two\n");
    }

    #[test]
    fn text_json_prints_the_string_value_unchanged() {
        let value = json!("line one\nline two\n");
        let bytes = render(&Returns::Text, &value, true, false).unwrap();
        assert_eq!(as_text(bytes), "\"line one\\nline two\\n\"\n");
    }

    /// Decision 5: `-q` leaves `Text` unchanged.
    #[test]
    fn text_quiet_is_the_same_as_without_quiet_either_way() {
        let value = json!("some text");
        for json_flag in [false, true] {
            let with_quiet = render(&Returns::Text, &value, json_flag, true).unwrap();
            let without_quiet = render(&Returns::Text, &value, json_flag, false).unwrap();
            assert_eq!(with_quiet, without_quiet, "json={json_flag}");
        }
    }

    #[test]
    fn text_with_a_non_string_reply_is_a_shape_error_not_a_panic() {
        let value = json!(42);
        assert!(render(&Returns::Text, &value, false, false).is_err());
    }

    // --- Nested ------------------------------------------------------------------

    /// Decision 1: plain text and `--json` are the same bytes for a
    /// `Nested` reply.
    #[test]
    fn nested_plain_and_json_are_identical_bytes() {
        let returns = Returns::Nested {
            of: "PaneAttached".into(),
            summary: String::new(),
        };
        let value = json!({"stream": "st1", "snapshot": {"rows": []}});
        let plain = render(&returns, &value, false, false).unwrap();
        let with_json = render(&returns, &value, true, false).unwrap();
        assert_eq!(plain, with_json);
        assert_eq!(
            as_text(plain),
            "{\"stream\":\"st1\",\"snapshot\":{\"rows\":[]}}\n"
        );
    }

    /// Decision 1 again: `-q` makes no difference either, for the same
    /// reason `Text` is unaffected by it.
    #[test]
    fn nested_quiet_makes_no_difference() {
        let returns = Returns::Nested {
            of: "TreeSnapshot".into(),
            summary: String::new(),
        };
        let value = json!({"sessions": []});
        for json_flag in [false, true] {
            let with_quiet = render(&returns, &value, json_flag, true).unwrap();
            let without_quiet = render(&returns, &value, json_flag, false).unwrap();
            assert_eq!(with_quiet, without_quiet, "json={json_flag}");
        }
    }

    // --- The negative control the plan names: a seventh `Returns` variant ---
    //
    // Not a test that can be written here - adding a variant to
    // `steg_sdk::description::shapes::Returns` and observing that
    // `render`'s own exhaustive match fails to build is the control the
    // plan names, performed by hand against a scratch edit of
    // `crates/sdk/src/description/shapes.rs`, confirmed, then reverted -
    // recorded on the task, not expressible as a `#[test]` in this crate.
}
