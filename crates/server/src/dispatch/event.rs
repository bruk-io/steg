//! The `event` noun's own entry point (TASK-1.46; doc-14): `event.watch`'s
//! reply (the tree at the moment a subscription begins, AC #1) and its own
//! filter (`--for`/`target`/`--all`, AC #3).
//!
//! Neither function here is ever reached through `dispatch::dispatch`'s own
//! routing (`mod.rs`'s own comment on the `("event", _)` arm says why):
//! `Runtime::subscribe` is the one caller, since answering `event.watch`
//! atomically needs the connection's own registered subscriber, which
//! `dispatch::dispatch`'s plain `&mut Tree` signature has no way to reach.

use steg_kernel::{Moment, Tree};
use steg_sdk::{ApiError, Request, SessionId, SessionTree, TabTree, TreeSnapshot};

use super::{Context, pane, param_bool, param_str, session, tab, target_session};

/// Builds the tree snapshot `event.watch` replies with (AC #1): every
/// session, each through [`session::record_at`], with its tabs embedded
/// (each through [`tab::record_for`]), each with its panes embedded (each
/// through [`pane::record_for`]) - reusing the exact record builders
/// `session.list`/`tab.list`/`pane.list` already use, never a second
/// definition of any of the three records.
#[must_use]
pub(crate) fn tree_snapshot(tree: &Tree, now: Moment) -> TreeSnapshot {
    let sessions = tree.sessions();
    let session_trees = (0..sessions.len())
        .map(|index| {
            let record = session::record_at(sessions, index);
            let tabs = sessions[index]
                .tabs()
                .iter()
                .enumerate()
                .map(|(position, tab)| {
                    let tab_record = tab::record_for(tree, tab, position + 1, now);
                    let panes = tab
                        .panes()
                        .iter()
                        .map(|pane| pane::record_for(tree, tab.id(), pane, now))
                        .collect();
                    TabTree {
                        tab: tab_record,
                        panes,
                    }
                })
                .collect();
            SessionTree {
                session: record,
                tabs,
            }
        })
        .collect();
    TreeSnapshot {
        sessions: session_trees,
    }
}

/// What `event.watch` declared for one connection (AC #3): which one event
/// name to keep (`None` for every event — `for` is a single closed-set
/// value, `ValueKind::Choice`, not a list), and which session(s) reach it
/// (`None` after `--all`: every session; `Some` after a plain `target`:
/// only that one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Filter {
    pub(crate) name: Option<String>,
    pub(crate) session: Option<SessionId>,
}

impl Filter {
    /// Whether an event named `name`, in `scope` (the session it belongs
    /// to), reaches a subscriber with this filter.
    pub(crate) fn matches(&self, name: &str, scope: SessionId) -> bool {
        let name_ok = self.name.as_deref().is_none_or(|only| only == name);
        let scope_ok = self.session.is_none_or(|session| session == scope);
        name_ok && scope_ok
    }
}

/// Resolves `event.watch`'s own filter (AC #3). `--all` is read first and,
/// when true, skips resolving a target session entirely - `target_session`
/// falls back to `context.default_session` when none is given, and every
/// connection today has no context at all (`Context::none`), so resolving
/// it unconditionally would refuse the one path that actually works before
/// `STEG_SESSION` exists on a connection (a future task's gap, not
/// invented here, not worked around either: `--all` simply never needs it).
///
/// # Errors
/// Whatever `target_session` gives when `--all` is false and no session
/// can be resolved.
pub(crate) fn resolve_filter(
    tree: &Tree,
    context: &Context,
    request: &Request,
) -> Result<Filter, ApiError> {
    let all = param_bool(request, "all").unwrap_or(false);
    let name = param_str(request, "for").map(str::to_owned);
    let session = if all {
        None
    } else {
        Some(target_session(tree, context, request)?)
    };
    Ok(Filter { name, session })
}

#[cfg(test)]
mod tests {
    use steg_kernel::{Moment, Tree};
    use steg_sdk::{Direction, PaneKind, Program, SessionId};

    use super::{Filter, resolve_filter, tree_snapshot};
    use crate::dispatch::Context;

    fn moment(seconds: u64) -> Moment {
        Moment::new(std::time::Duration::from_secs(seconds))
    }

    #[test]
    fn a_filter_with_no_name_and_no_session_matches_anything() {
        let filter = Filter {
            name: None,
            session: None,
        };
        assert!(filter.matches("pane.closed", SessionId::from_raw(1)));
        assert!(filter.matches("session.created", SessionId::from_raw(2)));
    }

    #[test]
    fn a_filter_narrowed_by_name_refuses_every_other_name() {
        let filter = Filter {
            name: Some("pane.closed".to_string()),
            session: None,
        };
        assert!(filter.matches("pane.closed", SessionId::from_raw(1)));
        assert!(!filter.matches("pane.created", SessionId::from_raw(1)));
    }

    #[test]
    fn a_filter_narrowed_by_session_refuses_every_other_session() {
        let filter = Filter {
            name: None,
            session: Some(SessionId::from_raw(1)),
        };
        assert!(filter.matches("pane.closed", SessionId::from_raw(1)));
        assert!(!filter.matches("pane.closed", SessionId::from_raw(2)));
    }

    #[test]
    fn resolve_filter_with_all_skips_resolving_a_target_session_entirely() {
        let tree = Tree::new();
        let context = Context::none();
        let request = steg_sdk::Request {
            method: "event.watch".to_string(),
            params: serde_json::json!({ "all": true })
                .as_object()
                .unwrap()
                .clone(),
        };
        let filter = resolve_filter(&tree, &context, &request).unwrap();
        assert_eq!(filter.session, None);
    }

    #[test]
    fn resolve_filter_with_no_target_and_no_context_refuses() {
        let tree = Tree::new();
        let context = Context::none();
        let request = steg_sdk::Request {
            method: "event.watch".to_string(),
            params: serde_json::Map::new(),
        };
        assert!(resolve_filter(&tree, &context, &request).is_err());
    }

    #[test]
    fn resolve_filter_reads_the_for_name() {
        let mut tree = Tree::new();
        let session = tree.new_session(None).unwrap();
        let context = Context {
            default_session: Some(session),
            default_tab: None,
            default_pane: None,
        };
        let request = steg_sdk::Request {
            method: "event.watch".to_string(),
            params: serde_json::json!({ "for": "pane.closed" })
                .as_object()
                .unwrap()
                .clone(),
        };
        let filter = resolve_filter(&tree, &context, &request).unwrap();
        assert_eq!(filter.name.as_deref(), Some("pane.closed"));
        assert_eq!(filter.session, Some(session));
    }

    #[test]
    fn an_empty_tree_snapshots_to_no_sessions() {
        let tree = Tree::new();
        let snapshot = tree_snapshot(&tree, moment(0));
        assert_eq!(snapshot.sessions, Vec::new());
    }

    /// AC #1: one session, one tab, two panes - every level embedded, not
    /// a flat list the caller has to stitch back together.
    #[test]
    fn a_session_with_a_split_tab_snapshots_with_both_panes_nested_under_their_tab() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(Some("work".to_string())).unwrap();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        tree.start_program(
            first_pane,
            Program::Command {
                argv: vec!["htop".to_string()],
                cwd: "/tmp".to_string(),
            },
        )
        .unwrap();
        tree.split_pane(first_pane, Direction::Right, PaneKind::Terminal)
            .unwrap();

        let snapshot = tree_snapshot(&tree, moment(0));

        assert_eq!(snapshot.sessions.len(), 1);
        let session = &snapshot.sessions[0];
        assert_eq!(session.session.label.as_deref(), Some("work"));
        assert_eq!(session.tabs.len(), 1);
        let tab = &session.tabs[0];
        assert_eq!(tab.panes.len(), 2, "both panes are nested under their tab");
        assert!(
            tab.panes.iter().any(|pane| pane.id == first_pane),
            "the original pane is one of them"
        );
    }

    /// The pane-identity question (TASK-1.46's own plan, section 2),
    /// proven against a real, multi-pane tree rather than only the sdk's
    /// own unit-level fixture: every id a tab's `layout` names is among
    /// that same tab's own `panes`.
    #[test]
    fn every_layout_leaf_names_a_pane_that_is_in_the_tabs_own_panes() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        tree.split_pane(first_pane, Direction::Right, PaneKind::Terminal)
            .unwrap();

        let snapshot = tree_snapshot(&tree, moment(0));
        let tab = &snapshot.sessions[0].tabs[0];
        let pane_ids: Vec<_> = tab.panes.iter().map(|pane| pane.id).collect();
        for leaf in tab.tab.layout.leaves() {
            assert!(
                pane_ids.contains(&leaf),
                "layout names {leaf}, not among this tab's own panes {pane_ids:?}"
            );
        }
    }
}
