//! The reference docs, rendered from the description (R62).
//!
//! The same description that generates the commands generates this, so the docs cannot
//! describe a flag the CLI does not have. `.claude/rules/architecture.md` counts a method
//! documented by hand as a second definition; this is what stops there being one.

use std::fmt::Write as _;

use crate::description::{Description, Method, Param, Returns, Role, ValueKind};

/// The whole reference, as Markdown.
///
/// # Panics
///
/// Never: writing to a `String` cannot fail, and the `write!` results are discarded for
/// that reason.
pub fn reference(api: &Description) -> String {
    let mut page = String::new();
    let _ = writeln!(page, "# steg reference\n");
    let _ = writeln!(
        page,
        "API version {}.{}. Generated from the API description; do not edit.\n",
        api.api_version.major, api.api_version.minor
    );
    for noun in api.nouns() {
        let _ = writeln!(page, "## `steg {noun}`\n");
        for method in api.verbs_of(noun) {
            page.push_str(&method_page(method, api));
        }
    }
    page.push_str(&events_page(api));
    page.push_str(&errors_page(api));
    page
}

/// One method's page: what it does, how it is typed, what it answers with.
pub fn method_page(method: &Method, api: &Description) -> String {
    let mut page = String::new();
    let _ = writeln!(
        page,
        "### `steg {} {}`\n",
        method.name.noun, method.name.verb
    );
    let _ = writeln!(page, "{}\n", method.summary);
    let _ = writeln!(page, "```\n{}\n```\n", usage(method));
    let _ = writeln!(page, "{}\n", method.details);
    if !method.params.is_empty() {
        let _ = writeln!(page, "| Argument | Value | Required | Meaning |");
        let _ = writeln!(page, "|---|---|---|---|");
        for param in &method.params {
            let _ = writeln!(
                page,
                "| {} | {} | {} | {} |",
                argument(param),
                value_of(&param.value),
                if param.required { "yes" } else { "no" },
                param.summary
            );
        }
        page.push('\n');
    }
    let _ = writeln!(page, "**Result.** {}\n", result_of(&method.returns));
    if !method.errors.is_empty() {
        let _ = writeln!(page, "**Errors.**\n");
        for code in &method.errors {
            if let Some(record) = api.errors.iter().find(|r| r.code == *code) {
                let _ = writeln!(
                    page,
                    "- {} (exit status {})",
                    record.summary, record.exit_status
                );
            }
        }
        page.push('\n');
    }
    let _ = writeln!(
        page,
        "API method `{}`, since {}.{}.\n",
        method.name.dotted(),
        method.since.major,
        method.since.minor
    );
    page
}

/// The usage line, in the grammar of doc-2 section 2.
pub fn usage(method: &Method) -> String {
    let mut line = format!("steg {} {}", method.name.noun, method.name.verb);
    if let Some(target) = method.target() {
        line.push_str(if target.required {
            " <TARGET>"
        } else {
            " [TARGET]"
        });
    }
    for param in &method.params {
        match param.role {
            Role::Flag => {
                let flag = format!(
                    "--{} <{}>",
                    param.flag(),
                    param.value.placeholder(&param.name)
                );
                line.push(' ');
                line.push_str(&if param.required {
                    flag
                } else {
                    format!("[{flag}]")
                });
            }
            Role::Switch => {
                let _ = write!(line, " [--{} | --{}]", param.flag(), param.negated_flag());
            }
            Role::Program => line.push_str(" [-- PROGRAM ...]"),
            Role::Target => {}
        }
    }
    line
}

/// How a parameter is written on the command line.
fn argument(param: &Param) -> String {
    match param.role {
        Role::Target => "`<TARGET>`".to_owned(),
        Role::Flag => format!("`--{}`", param.flag()),
        Role::Switch => format!("`--{}`, `--{}`", param.flag(), param.negated_flag()),
        Role::Program => "`-- PROGRAM ...`".to_owned(),
    }
}

/// The word for a value's kind.
fn value_of(kind: &ValueKind) -> String {
    match kind {
        ValueKind::Text => "text".to_owned(),
        ValueKind::Integer => "a whole number".to_owned(),
        ValueKind::Boolean => "a switch".to_owned(),
        ValueKind::Choice(words) => format!("one of {}", words.join(", ")),
        ValueKind::Target => "a target".to_owned(),
        ValueKind::Path => "a path".to_owned(),
        ValueKind::Argv => "a program and its arguments".to_owned(),
    }
}

/// What the method prints on success, in doc-2 section 6's terms.
fn result_of(returns: &Returns) -> String {
    match returns {
        Returns::Nothing => "Nothing. An action that worked is silent.".to_owned(),
        Returns::Identifier { of } => {
            format!("The new {of}'s full identifier, alone on a line.")
        }
        Returns::Record { fields } => format!("One record: {}.", field_names(fields)),
        Returns::Records { fields } => {
            format!(
                "One record per line, tab-separated: {}.",
                field_names(fields)
            )
        }
        Returns::Text => "Text.".to_owned(),
    }
}

fn field_names(fields: &[crate::description::Field]) -> String {
    fields
        .iter()
        .map(|field| field.name.clone())
        .collect::<Vec<String>>()
        .join(", ")
}

/// The events a client can subscribe to (R38), from the same description.
fn events_page(api: &Description) -> String {
    let mut page = String::new();
    let _ = writeln!(page, "## Events\n");
    let _ = writeln!(page, "| Event | Fields | Meaning |");
    let _ = writeln!(page, "|---|---|---|");
    for event in &api.events {
        let _ = writeln!(
            page,
            "| `{}` | {} | {} |",
            event.name,
            field_names(&event.fields),
            event.summary
        );
    }
    page.push('\n');
    page
}

/// The exit statuses, from the same description rather than from doc-2 by hand.
fn errors_page(api: &Description) -> String {
    let mut page = String::new();
    let _ = writeln!(page, "## Errors and exit statuses\n");
    let _ = writeln!(page, "| Status | Error | Meaning |");
    let _ = writeln!(page, "|---|---|---|");
    for record in &api.errors {
        let _ = writeln!(
            page,
            "| {} | `{:?}` | {} |",
            record.exit_status, record.code, record.summary
        );
    }
    page.push('\n');
    page
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api;

    #[test]
    fn the_usage_line_follows_doc_2s_grammar() {
        let api = api::description();
        let method = api
            .method("pane.split")
            .expect("pane.split is in the definition");
        assert_eq!(
            usage(method),
            "steg pane split [TARGET] --direction <left|right|up|down> \
             [--working-directory <WORKING_DIRECTORY>] [--label <LABEL>] \
             [--focus | --no-focus] [-- PROGRAM ...]"
        );
    }

    #[test]
    fn a_methods_page_carries_its_documentation_its_flags_and_its_statuses() {
        let api = api::description();
        let method = api
            .method("pane.split")
            .expect("pane.split is in the definition");
        let page = method_page(method, &api);
        assert!(
            page.contains(&method.summary),
            "the summary reaches the docs"
        );
        assert!(
            page.contains("--working-directory"),
            "a flag reaches the docs"
        );
        assert!(
            page.contains("exit status 3"),
            "an error's status reaches the docs"
        );
        assert!(page.contains("`pane.split`"), "the API method is named");
    }

    #[test]
    fn the_reference_covers_the_events_and_the_statuses_too() {
        let page = reference(&api::description());
        assert!(
            page.contains("pane.closed"),
            "events are in the description, so in the docs"
        );
        assert!(page.contains("Errors and exit statuses"));
    }
}
