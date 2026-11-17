//! Mapping a parameter's [`ValueKind`] to a clap value parser - one list,
//! used for parsing, help text and (independently, server-side,
//! `steg_sdk::description::validate`) validation, so a `Choice`'s accepted
//! words are never written down twice.

use clap::builder::{PossibleValuesParser, ValueParser};
use steg_sdk::ValueKind;

#[must_use]
pub(super) fn value_parser_for(kind: &ValueKind) -> ValueParser {
    match kind {
        ValueKind::Text | ValueKind::Target | ValueKind::Path | ValueKind::Argv => {
            ValueParser::string()
        }
        ValueKind::Integer => clap::value_parser!(i64).into(),
        ValueKind::Boolean => clap::value_parser!(bool),
        ValueKind::Choice(words) => PossibleValuesParser::new(words.clone()).into(),
    }
}
