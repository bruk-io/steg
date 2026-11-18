//! The one definition of steg's API (decision-1).
//!
//! [`description`] is the single source every consumer reads: the
//! dispatch task's server validates and dispatches from it, a future CLI
//! (TASK-1.49) builds its whole command tree from it, the reference docs
//! are rendered from it, and a Python plugin gets it as JSON. Nothing
//! downstream names a method, a parameter or a flag of its own.
//!
//! Every method doc-2 section 8 lists for the first release is here —
//! `version` and `completion` are not API methods (doc-2 section 2) and are
//! not. Which methods the kernel can serve today and which cannot yet is a
//! dispatch question, carried on TASK-1.43's board task for the task that
//! builds dispatch; this module describes all 42 regardless, each with the
//! shape its `returns` promises, because `fields` alone already answers
//! R37 whether or not a Rust type backs it yet (`SessionRecord` and
//! `SettingRecord` are the ones that do, in `crate::record`, since
//! `session.list`/`session.show` and `setting.list`/`setting.show` are
//! the only methods whose record the kernel can fill today).
//!
//! Grouped by noun, as section 8 is, so a method is easy to find and a
//! review against that document is a direct comparison.

use super::shapes::{Description, ErrorCode, Field, Follows, Method, Returns, ValueKind};

mod errors;
mod event;
mod pane;
mod server;
mod session;
mod shared;
mod tab;

use errors::errors;
use event::{event_methods, events};
use pane::pane_methods;
use server::server_methods;
use session::session_methods;
use shared::{
    VERSION, field, flag, method, method_target, required_target, switch, target_of_kind,
};
use tab::tab_methods;

/// The whole API, as data.
#[must_use]
pub fn description() -> Description {
    let events = events();
    let event_names: Vec<String> = events.iter().map(|event| event.name.clone()).collect();

    let mut methods = Vec::new();
    methods.extend(session_methods());
    methods.extend(tab_methods());
    methods.extend(pane_methods());
    methods.extend(server_methods());
    methods.extend(event_methods(&event_names));
    methods.extend(client_methods());
    methods.extend(setting_methods());
    methods.extend(api_methods());

    Description {
        api_version: VERSION,
        methods,
        events,
        errors: errors(),
    }
}

// --- client -----------------------------------------------------------

fn client_fields() -> Vec<Field> {
    vec![
        field("id", ValueKind::Text, "The client's full identifier"),
        field(
            "kind",
            ValueKind::Choice(vec!["app".into(), "cli".into(), "plugin".into()]),
            "What kind of client it is",
        ),
        field("label", ValueKind::Text, "Its label, if it has one"),
        field(
            "sessions",
            ValueKind::Text,
            "The sessions it views, as a JSON array of identifiers",
        ),
        field(
            "last_active",
            ValueKind::Text,
            "When it was last active, as ISO 8601 in UTC",
        ),
    ]
}

fn client_methods() -> Vec<Method> {
    vec![
        Method {
            name: method("client", "list"),
            summary: "Every client connected to the server".into(),
            details: String::new(),
            params: vec![switch(
                "all",
                "Command line clients as well, not just the app's",
            )],
            returns: Returns::Records {
                fields: client_fields(),
            },
            errors: vec![ErrorCode::BadRequest],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("client", "show"),
            summary: "What a client is, and the sessions it views".into(),
            details: String::new(),
            params: vec![required_target("The client to show")],
            returns: Returns::Record {
                fields: client_fields(),
            },
            errors: vec![
                ErrorCode::BadRequest,
                ErrorCode::NoSuchTarget,
                ErrorCode::AmbiguousTarget,
            ],
            follows: Follows::Nothing,
            since: VERSION,
        },
    ]
}

// --- setting -----------------------------------------------------------

/// `setting.list`/`setting.show`'s own row: a name, its current value
/// (the override if one is set, else the default) and its default. Every
/// field is `Text`: a setting's value is never typed per entry on the
/// wire (`crate::setting`'s module doc; TASK-1.48's plan).
fn setting_fields() -> Vec<Field> {
    vec![
        field("name", ValueKind::Text, "The setting's name"),
        field(
            "value",
            ValueKind::Text,
            "Its current value: the override if one is set, else the default",
        ),
        field("default", ValueKind::Text, "Its value with no override"),
    ]
}

fn setting_methods() -> Vec<Method> {
    vec![
        Method {
            name: method("setting", "list"),
            summary: "Every setting, its current value and its default".into(),
            details: String::new(),
            params: Vec::new(),
            returns: Returns::Records {
                fields: setting_fields(),
            },
            errors: vec![ErrorCode::BadRequest],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("setting", "show"),
            summary: "One setting: its name, current value and default".into(),
            details: String::new(),
            params: vec![target_of_kind(
                ValueKind::Text,
                true,
                "The setting to show, by its name",
            )],
            returns: Returns::Record {
                fields: setting_fields(),
            },
            errors: vec![ErrorCode::BadRequest, ErrorCode::NoSuchTarget],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("setting", "set"),
            summary: "Change a setting's value".into(),
            details: "The setting is named by the positional and its new value is the \
                      flag, since here the property is the thing addressed (doc-2 \
                      section 8) — unlike `session set --label`, where the target is the \
                      session and the property is its own flag."
                .into(),
            params: vec![
                target_of_kind(ValueKind::Text, true, "The setting to change, by its name"),
                flag("value", ValueKind::Text, true, "The setting's new value"),
            ],
            returns: Returns::Nothing,
            errors: vec![ErrorCode::BadRequest, ErrorCode::NoSuchTarget],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("setting", "unset"),
            summary: "Clear a setting's override, reverting it to its default".into(),
            details: String::new(),
            params: vec![target_of_kind(
                ValueKind::Text,
                true,
                "The setting to change, by its name",
            )],
            returns: Returns::Nothing,
            errors: vec![ErrorCode::BadRequest, ErrorCode::NoSuchTarget],
            follows: Follows::Nothing,
            since: VERSION,
        },
    ]
}

// --- api -------------------------------------------------------------------

/// The projection `api.list`/`api.show` print in their flat text form: a
/// method's dotted name and its summary. `--json` carries the whole
/// `Method` (params, errors, `since`...), which has more fields than this
/// list — the one place in this description where a served method's
/// actual JSON has more keys than its declared `fields`, because the
/// value really is `Method`, not a record invented to match a projection.
/// Exempted from the `fields`-versus-type cross-check for exactly that
/// reason (see `crate::record`'s test module, and the dispatch task's
/// notes).
fn api_method_fields() -> Vec<Field> {
    vec![
        field("method", ValueKind::Text, "The method's dotted name"),
        field("summary", ValueKind::Text, "One line describing it"),
    ]
}

fn api_methods() -> Vec<Method> {
    vec![
        Method {
            name: method("api", "list"),
            summary: "Every method, one per line".into(),
            details: String::new(),
            params: Vec::new(),
            returns: Returns::Records {
                fields: api_method_fields(),
            },
            errors: vec![ErrorCode::BadRequest],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("api", "show"),
            summary: "One method in full, or the whole API with no target".into(),
            details: "With a method named, its parameters and documentation in full. With no \
                      target, the whole API description — every method, event and error, and \
                      the API's own version — in one answer, for a client that cannot import \
                      the Rust value (decision-1: \"served to anyone who asks\")."
                .into(),
            params: vec![target_of_kind(
                ValueKind::Text,
                false,
                "The method to show, by its dotted name. With none, the whole description",
            )],
            returns: Returns::Record {
                fields: api_method_fields(),
            },
            errors: vec![ErrorCode::BadRequest],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("api", "call"),
            summary: "Call any method by name".into(),
            details: "For an agent, and for a method a plugin adds before the CLI knows it. \
                      Never the documented way to do something that has a command of its own. \
                      `method` names which method to call; every other parameter given is that \
                      method's own, by its own declared names — nothing is nested under a second \
                      key, so a method's parameters are never named twice. The selector is \
                      `method`, not the usual `target`, precisely so that the called method's \
                      own `target` (where it has one, such as `pane.split` or `setting.show`) \
                      is a different key and travels through untouched (TASK-1.81)."
                .into(),
            params: vec![method_target("The method to call, by its dotted name")],
            returns: Returns::Text,
            errors: vec![ErrorCode::BadRequest],
            follows: Follows::Nothing,
            since: VERSION,
        },
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::description;
    use crate::description::shapes::Role;

    /// doc-2 section 8's table, as `(noun, verb)` pairs — the list this
    /// module is checked against. `version` and `completion` are not API
    /// methods (doc-2 section 2) and are not here.
    fn doc_2_section_8() -> Vec<(&'static str, &'static str)> {
        vec![
            ("session", "list"),
            ("session", "show"),
            ("session", "new"),
            ("session", "close"),
            ("session", "move"),
            ("session", "set"),
            ("session", "unset"),
            ("session", "focus"),
            ("tab", "list"),
            ("tab", "show"),
            ("tab", "new"),
            ("tab", "close"),
            ("tab", "move"),
            ("tab", "set"),
            ("tab", "unset"),
            ("tab", "focus"),
            ("tab", "declare-size"),
            ("pane", "list"),
            ("pane", "show"),
            ("pane", "split"),
            ("pane", "close"),
            ("pane", "move"),
            ("pane", "set"),
            ("pane", "unset"),
            ("pane", "focus"),
            ("pane", "resize"),
            ("pane", "swap"),
            ("pane", "send"),
            ("pane", "capture"),
            ("pane", "wait"),
            ("pane", "attach"),
            ("pane", "detach"),
            ("server", "show"),
            ("server", "start"),
            ("server", "stop"),
            ("event", "watch"),
            ("client", "list"),
            ("client", "show"),
            ("api", "list"),
            ("api", "show"),
            ("api", "call"),
            ("setting", "list"),
            ("setting", "show"),
            ("setting", "set"),
            ("setting", "unset"),
        ]
    }

    #[test]
    fn the_method_set_is_exactly_doc_2_section_8s_45_pairs() {
        let api = description();
        assert_eq!(
            api.methods.len(),
            45,
            "doc-2 section 8 lists 45 methods (TASK-1.114 added tab.declare-size)"
        );

        let described: HashSet<(String, String)> = api
            .methods
            .iter()
            .map(|m| (m.name.noun.clone(), m.name.verb.clone()))
            .collect();
        assert_eq!(described.len(), api.methods.len(), "no duplicate method");

        let expected: HashSet<(String, String)> = doc_2_section_8()
            .into_iter()
            .map(|(noun, verb)| (noun.to_string(), verb.to_string()))
            .collect();
        assert_eq!(described, expected);
    }

    #[test]
    fn no_method_or_param_summary_or_param_summary_is_empty() {
        let api = description();
        for method in &api.methods {
            assert!(
                !method.summary.is_empty(),
                "{} has no summary",
                method.name.dotted()
            );
            for param in &method.params {
                assert!(
                    !param.summary.is_empty(),
                    "{}'s {} has no summary",
                    method.name.dotted(),
                    param.name
                );
            }
        }
    }

    #[test]
    fn no_method_declares_a_cli_only_flag_as_a_parameter() {
        // decision-1 section 2: these belong to the CLI, not the API.
        const CLI_ONLY: &[&str] = &[
            "json",
            "quiet",
            "verbose",
            "no_color",
            "no_input",
            "help",
            "host",
            "autostart",
            "no_autostart",
        ];
        let api = description();
        for method in &api.methods {
            for param in &method.params {
                assert!(
                    !CLI_ONLY.contains(&param.name.as_str()),
                    "{} declares {}, which is a CLI-only flag",
                    method.name.dotted(),
                    param.name
                );
            }
        }
    }

    /// `api.call`'s own selector is named `method`, not the usual `target`,
    /// precisely so it never collides with a called method's own
    /// parameter of the same name (TASK-1.81). That only holds if no other
    /// method ever declares a parameter named `method` either — this is
    /// the invariant that keeps the flat merge in `api::call` safe for
    /// every method added later, the same way
    /// `no_method_declares_a_cli_only_flag_as_a_parameter` keeps the
    /// CLI-only flags safe.
    #[test]
    fn no_method_other_than_api_call_declares_a_parameter_named_method() {
        let api = description();
        for method in &api.methods {
            if method.name.dotted() == "api.call" {
                continue;
            }
            assert!(
                method.params.iter().all(|p| p.name != "method"),
                "{} declares a parameter named `method`, which would collide with api.call's \
                 own selector",
                method.name.dotted()
            );
        }
    }

    #[test]
    fn a_method_has_at_most_one_target_and_at_most_one_program() {
        let api = description();
        for method in &api.methods {
            assert!(
                method
                    .params
                    .iter()
                    .filter(|p| p.role == Role::Target)
                    .count()
                    <= 1,
                "{} has more than one positional",
                method.name.dotted()
            );
            assert!(
                method
                    .params
                    .iter()
                    .filter(|p| p.role == Role::Program)
                    .count()
                    <= 1,
                "{} has more than one program",
                method.name.dotted()
            );
        }
    }

    #[test]
    fn every_error_a_method_names_has_a_record() {
        let api = description();
        for method in &api.methods {
            for code in &method.errors {
                assert!(
                    api.errors.iter().any(|record| record.code == *code),
                    "{} names {code:?}, which has no record",
                    method.name.dotted()
                );
            }
        }
    }

    #[test]
    fn events_has_r38s_fourteen_m1_names_each_once() {
        let api = description();
        assert_eq!(api.events.len(), 14);
        let names: HashSet<&str> = api.events.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names.len(), api.events.len(), "no duplicate event name");
        for expected in [
            "session.created",
            "session.closed",
            "session.focused",
            "tab.created",
            "tab.closed",
            "tab.focused",
            "pane.created",
            "pane.closed",
            "pane.focused",
            "pane.cwd_changed",
            "pane.title_changed",
            "pane.bell",
            "pane.exited",
            "pane.status_changed",
        ] {
            assert!(names.contains(expected), "missing event {expected}");
        }
    }

    #[test]
    fn errors_has_twelve_codes_with_their_exit_statuses() {
        use crate::description::shapes::ErrorCode;

        let api = description();
        assert_eq!(api.errors.len(), 12);

        let status_of = |code: ErrorCode| {
            api.errors
                .iter()
                .find(|record| record.code == code)
                .unwrap_or_else(|| panic!("{code:?} has no record"))
                .exit_status
        };
        // doc-2 section 6's own table, for the ten errors it names.
        assert_eq!(status_of(ErrorCode::BadRequest), 2);
        assert_eq!(status_of(ErrorCode::NoSuchTarget), 3);
        assert_eq!(status_of(ErrorCode::AmbiguousTarget), 3);
        assert_eq!(status_of(ErrorCode::PositionOutOfRange), 2);
        assert_eq!(status_of(ErrorCode::ProgramRunning), 1);
        assert_eq!(status_of(ErrorCode::SplitTooSmall), 1);
        assert_eq!(status_of(ErrorCode::NoSplitInDirection), 1);
        assert_eq!(status_of(ErrorCode::PanesNotInSameTab), 1);
        assert_eq!(status_of(ErrorCode::AlreadyInTab), 1);
        assert_eq!(status_of(ErrorCode::NotImplementedYet), 1);
        // `EngineFailed` (TASK-1.75): not in doc-2 section 6's table at all
        // (it predates the terminal engine) - 1, "the command ran and
        // failed", matches a genuine runtime failure better than status 2
        // (bad usage) or 3 (a target problem). Flagged on the task for the
        // engineer to confirm or overrule.
        assert_eq!(status_of(ErrorCode::EngineFailed), 1);
        // `ProgramFailed` (TASK-1.76): the request was well-formed, the OS
        // refused to start the program - "the command ran and failed", the
        // same status `ProgramRunning`/`SplitTooSmall`/`EngineFailed`
        // already use, not status 2 (`BadRequest`'s own bucket for a
        // malformed request).
        assert_eq!(status_of(ErrorCode::ProgramFailed), 1);
    }

    /// AC #6: `ProgramNotStarted` and `ShellCannotRestart` (the kernel's
    /// own errors from `Tree::record_exit`/`restart_program`) have no
    /// dedicated `ErrorCode` at all, and stay that way honestly only if
    /// nothing in the description ever names a method that could produce
    /// them - there is no `pane.restart` (or any other verb for "run a
    /// command pane's program again", R97), so `Tree::restart_program`
    /// stays unreachable through dispatch, and `record_exit`'s own
    /// `ProgramNotStarted` is reachable only from this crate's own
    /// internal drain loop, never a described method at all.
    #[test]
    fn no_described_method_can_reach_restart_program_or_record_exit() {
        let api = description();
        assert!(
            api.methods.iter().all(|m| m.name.verb != "restart"),
            "a `restart` verb would need ProgramNotStarted/ShellCannotRestart \
             to finally get their own ErrorCode, which is not decided here"
        );
    }

    #[test]
    fn event_watch_for_is_drawn_from_the_event_names() {
        let api = description();
        let event_names: HashSet<&str> = api.events.iter().map(|e| e.name.as_str()).collect();

        let event_watch = api.method("event.watch").unwrap();
        let for_param = event_watch.params.iter().find(|p| p.name == "for").unwrap();
        let crate::description::shapes::ValueKind::Choice(words) = &for_param.value else {
            panic!("event.watch's for is a closed set");
        };
        assert_eq!(
            words.iter().map(String::as_str).collect::<HashSet<_>>(),
            event_names
        );
    }

    /// TASK-1.46 (AC #1, #4): `event.watch`'s own reply is the tree, a
    /// nested value too shaped to flatten into fields - named by its sdk
    /// type, not described as "any value".
    #[test]
    fn event_watch_replies_with_a_nested_tree_snapshot() {
        use crate::description::shapes::Returns;

        let api = description();
        let event_watch = api.method("event.watch").unwrap();
        let Returns::Nested { of, summary } = &event_watch.returns else {
            panic!(
                "event.watch's returns is not Nested: {:?}",
                event_watch.returns
            );
        };
        assert_eq!(of, "TreeSnapshot");
        assert!(!summary.is_empty());
    }

    /// TASK-1.46: `event.watch`'s reply is followed by events with no id,
    /// on the control stream the reply itself travelled on - never a new
    /// stream (that shape is `Follows::Attachment`, TASK-1.47's own case).
    /// The stream item's own fields are exactly what `returns` used to
    /// declare before this task moved them here.
    #[test]
    fn event_watch_is_followed_by_events_on_the_control_stream() {
        use crate::description::shapes::{Follows, Returns};

        let api = description();
        let event_watch = api.method("event.watch").unwrap();
        let Follows::Events(Returns::Records { fields }) = &event_watch.follows else {
            panic!(
                "event.watch's follows is not Events(Records): {:?}",
                event_watch.follows
            );
        };
        let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["event", "data"]);
    }

    /// Only `event.watch` (events, on the control stream) and `pane.attach`
    /// (TASK-1.47: screen updates, on a new stream) have anything following
    /// their reply - a forgotten `follows` on a future method is a wrong,
    /// silent default otherwise (`Follows::Nothing` has to be written, not
    /// assumed, since `Method` has no `Default`). Extended, not weakened,
    /// by TASK-1.47: the set of methods this allows is now two, named
    /// explicitly, rather than loosened to "any method may".
    #[test]
    fn only_event_watch_and_pane_attach_have_anything_following_their_reply() {
        use crate::description::shapes::Follows;

        const METHODS_WITH_A_FOLLOWING_STREAM: &[&str] = &["event.watch", "pane.attach"];

        let api = description();
        for method in &api.methods {
            let dotted = method.name.dotted();
            if METHODS_WITH_A_FOLLOWING_STREAM.contains(&dotted.as_str()) {
                assert_ne!(method.follows, Follows::Nothing, "{dotted}");
            } else {
                assert_eq!(method.follows, Follows::Nothing, "{dotted}");
            }
        }
    }

    /// A `Returns::Nested` names a real sdk type (decision-1: the single
    /// definition of its real shape), checked against a small, explicit
    /// allow-list rather than trusted by convention - a typo in `of`
    /// fails this, not just a human reader. Checks a method's own `returns`
    /// *and* its `follows`'s own nested item, if it has one (TASK-1.47:
    /// `pane.attach`'s `Follows::Attachment::item` is `Nested` too) - the
    /// same check, not a second one, since both are the identical "does
    /// this string name a real type" question.
    #[test]
    fn every_nested_reply_and_stream_item_names_a_real_sdk_type() {
        use crate::description::shapes::{Follows, Returns};

        const KNOWN_NESTED_TYPES: &[&str] = &["TreeSnapshot", "PaneAttached", "ScreenUpdate"];

        fn assert_known(of: &str, dotted: &str) {
            assert!(
                KNOWN_NESTED_TYPES.contains(&of),
                "{dotted} names {of}, which is not in the known sdk types"
            );
        }

        let api = description();
        for method in &api.methods {
            let dotted = method.name.dotted();
            if let Returns::Nested { of, .. } = &method.returns {
                assert_known(of, &dotted);
            }
            match &method.follows {
                Follows::Nothing | Follows::Events(_) => {}
                Follows::Attachment { item, .. } => {
                    if let Returns::Nested { of, .. } = item {
                        assert_known(of, &dotted);
                    }
                }
            }
        }
    }

    /// `Follows::Attachment::stream_id_field` names a field that really
    /// exists on the method's own `Nested` reply type - checked at run
    /// time against a real, serialised value of that type, not only by
    /// eye: a typo here, or a later rename of `PaneAttached::stream` that
    /// forgets this string, fails the build.
    #[test]
    fn pane_attachs_stream_id_field_names_a_real_field_of_its_own_reply() {
        use crate::description::shapes::Follows;
        use crate::screen::{PaneAttached, Screen};

        let api = description();
        let pane_attach = api.method("pane.attach").unwrap();
        let Follows::Attachment {
            stream_id_field, ..
        } = &pane_attach.follows
        else {
            panic!(
                "pane.attach's follows is not Attachment: {:?}",
                pane_attach.follows
            );
        };

        let sample = PaneAttached {
            stream: 1,
            snapshot: Screen {
                rows: Vec::new(),
                cursor: crate::screen::Cursor {
                    x: 0,
                    y: 0,
                    visible: true,
                    blinking: false,
                    shape: crate::screen::CursorShape::Block,
                },
                size: crate::layout::Size::new(80, 24),
            },
        };
        let value = serde_json::to_value(&sample).unwrap();
        let object = value.as_object().unwrap();
        assert!(
            object.contains_key(stream_id_field.as_str()),
            "stream_id_field names {stream_id_field:?}, which is not a field of PaneAttached: \
             {object:?}"
        );
    }

    /// doc-2 section 8 gives `pane wait`'s `--for` exactly one value in
    /// the first release, `exit` - not the dotted event names
    /// `event.watch`'s `--for` draws from. See the method's own `details`
    /// for why the two are not unified here.
    #[test]
    fn pane_wait_for_is_exactly_the_one_value_doc_2_section_8_gives_it() {
        let api = description();
        let pane_wait = api.method("pane.wait").unwrap();
        let for_param = pane_wait.params.iter().find(|p| p.name == "for").unwrap();
        assert_eq!(
            for_param.value,
            crate::description::shapes::ValueKind::Choice(vec!["exit".into()])
        );
    }

    /// doc-2 section 8's Target column, written out independently of the
    /// description: `None` for "none"; `Some((required, value))` for every
    /// other row, where `required` is whether doc-2's own text (section
    /// 4's context fallback, or `api show`'s stated "with none, the whole
    /// description") gives it a default, and `value` is `ValueKind::Target`
    /// for a session/tab/pane/client path and `ValueKind::Text` for the
    /// two whose target is a bare method name (`api show`/`api call`).
    fn doc_2_section_8_targets() -> Vec<(
        &'static str,
        Option<(bool, crate::description::shapes::ValueKind)>,
    )> {
        use crate::description::shapes::ValueKind;

        vec![
            ("session.list", None),
            ("session.show", Some((false, ValueKind::Target))),
            ("session.new", None),
            ("session.close", Some((false, ValueKind::Target))),
            ("session.move", Some((false, ValueKind::Target))),
            ("session.set", Some((false, ValueKind::Target))),
            ("session.unset", Some((false, ValueKind::Target))),
            ("session.focus", Some((false, ValueKind::Target))),
            ("tab.list", Some((false, ValueKind::Target))),
            ("tab.show", Some((false, ValueKind::Target))),
            ("tab.new", Some((false, ValueKind::Target))),
            ("tab.close", Some((false, ValueKind::Target))),
            ("tab.move", Some((false, ValueKind::Target))),
            ("tab.set", Some((false, ValueKind::Target))),
            ("tab.unset", Some((false, ValueKind::Target))),
            ("tab.focus", Some((false, ValueKind::Target))),
            ("tab.declare-size", Some((false, ValueKind::Target))),
            ("pane.list", Some((false, ValueKind::Target))),
            ("pane.show", Some((false, ValueKind::Target))),
            ("pane.split", Some((false, ValueKind::Target))),
            ("pane.close", Some((false, ValueKind::Target))),
            ("pane.move", Some((false, ValueKind::Target))),
            ("pane.set", Some((false, ValueKind::Target))),
            ("pane.unset", Some((false, ValueKind::Target))),
            ("pane.focus", Some((false, ValueKind::Target))),
            ("pane.resize", Some((false, ValueKind::Target))),
            ("pane.swap", Some((false, ValueKind::Target))),
            ("pane.send", Some((false, ValueKind::Target))),
            ("pane.capture", Some((false, ValueKind::Target))),
            ("pane.wait", Some((false, ValueKind::Target))),
            ("server.show", None),
            ("server.start", None),
            ("server.stop", None),
            ("event.watch", Some((false, ValueKind::Target))),
            ("client.list", None),
            ("client.show", Some((true, ValueKind::Target))),
            ("setting.list", None),
            ("setting.show", Some((true, ValueKind::Text))),
            ("setting.set", Some((true, ValueKind::Text))),
            ("setting.unset", Some((true, ValueKind::Text))),
            ("api.list", None),
            ("api.show", Some((false, ValueKind::Text))),
            ("api.call", Some((true, ValueKind::Text))),
        ]
    }

    /// The hole the review found: the completeness test above only checks
    /// the set of `(noun, verb)` pairs, which says nothing about whether a
    /// method's positional is actually `role: Target` (`api.show`/
    /// `api.call` had `role: Flag` instead, which a CLI generated from this
    /// description would turn into `--target <method>` rather than doc-2
    /// section 2's positional `steg api show <method>`). This test checks
    /// every method against doc-2 section 8's Target column directly,
    /// independently of whatever `target()`/`target_of_kind()` calls the
    /// method literals happen to make.
    #[test]
    fn every_method_has_a_target_exactly_as_doc_2_section_8_gives_it() {
        let api = description();
        for (dotted, expected) in doc_2_section_8_targets() {
            let described = api
                .method(dotted)
                .unwrap_or_else(|| panic!("{dotted} is not described"));
            match expected {
                None => assert!(
                    described.target().is_none(),
                    "{dotted}'s Target column is \"none\", so it should have no Target-role param"
                ),
                Some((required, value)) => {
                    let target = described
                        .target()
                        .unwrap_or_else(|| panic!("{dotted} should have a Target-role param"));
                    assert_eq!(target.required, required, "{dotted}'s target required-ness");
                    assert_eq!(target.value, value, "{dotted}'s target value kind");
                }
            }
        }
    }

    /// TASK-1.49's own fix: `unset --label` used to be a value-taking flag
    /// whose value `dispatch` discarded (`param_present`, not `param_str`),
    /// forcing a script to type one anyway. It is a switch now, like
    /// `pane.unset`'s own `zoomed` already is - `--label`/`--no-label` are
    /// both accepted and both clear it (`dispatch/pane.rs`'s own comment:
    /// "`unset` always clears, regardless of the switch's own boolean
    /// value"), with no value required at all. `session.set`/`tab.set`/
    /// `pane.set`'s own `label` stays `Flag`/`Text` - only `unset` changes,
    /// since `set` genuinely needs the new label's text.
    #[test]
    fn unset_s_label_is_a_switch_not_a_value_taking_flag() {
        let api = description();
        for dotted in ["session.unset", "tab.unset", "pane.unset"] {
            let method = api
                .method(dotted)
                .unwrap_or_else(|| panic!("{dotted} is not described"));
            let label = method
                .params
                .iter()
                .find(|p| p.name == "label")
                .unwrap_or_else(|| panic!("{dotted} has no label param"));
            assert_eq!(label.role, Role::Switch, "{dotted}'s label role");
            assert_eq!(
                label.value,
                crate::description::shapes::ValueKind::Boolean,
                "{dotted}'s label value kind"
            );
        }
    }
}
