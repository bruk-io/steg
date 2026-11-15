//! Resolving a session, a tab or a pane from a target's written form
//! (doc-15 R10; doc-2 section 4).
//!
//! The written form itself crosses the API as a plain `String` (doc-2
//! section 2: a method's target parameter is the positional, mechanically)
//! — there is no new sdk shape to define for it (doc-16 R105). The four
//! match kinds and their order are behaviour, not a shape decoded off the
//! wire, so they stay here. [`Candidate`] stays kernel-side for the same
//! reason `Error::SessionNotFound(SessionId)` does: mapping a kernel error
//! to a wire shape is the API description's job (TASK-1.43), not this
//! module's — `Candidate::id` is already the id's `Display` form, which is
//! what doc-2 section 6's `--json` contract calls a "full identifier", so
//! nothing is lost carrying it as a `String` now.
//!
//! A left-out leading part (doc-2 section 4: "A part left out comes from
//! the context") is not resolved here either. The context variables
//! (`STEG_SESSION`, `STEG_TAB`, `STEG_PANE`) "carry identifiers, never
//! labels", so filling one in is a plain lookup a caller can do itself,
//! with no matching and no ambiguity. [`Tree::resolve_tab_path`] and
//! [`Tree::resolve_pane_path`] take the caller's default id for any level
//! it chooses not to spell out, rather than reading a "current" tab or
//! pane the kernel tracks itself — so this module has no dependency on
//! focus (TASK-1.37).

use steg_sdk::{PaneId, SessionId, TabId};

use crate::error::Error;
use crate::pane::Pane;
use crate::session::Session;
use crate::tab::Tab;
use crate::tree::Tree;

/// One candidate shown when a target's part did not resolve to exactly
/// one thing: its full identifier and its label, if it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The candidate's full identifier, in its `Display` form.
    pub id: String,
    /// The candidate's label, if it has one.
    pub label: Option<String>,
}

/// What matching one part of a target against a set of candidates came to.
#[derive(Debug)]
enum Resolution<Id> {
    /// Exactly one candidate matched.
    Found(Id),
    /// Nothing matched, at any of the four kinds. Carries every candidate
    /// at this level, so a caller can show what it could have meant.
    NotFound(Vec<Candidate>),
    /// More than one candidate matched, at the first kind that matched at
    /// all. Carries only the candidates that matched.
    Ambiguous(Vec<Candidate>),
}

/// Matches one part of a target against a list of candidates, in the
/// order doc-15 R10 fixes: a full identifier, then a label (ignoring
/// case), then a one-based position, then an identifier suffix of four or
/// more characters — stopping at the first kind that matches.
///
/// "Stopping at the first kind that matches" means the first kind that
/// yields one *or more* candidates, not the first kind that yields
/// exactly one: R10 also says "An ambiguous target is an error, never a
/// guess", so a kind with more than one match stops the search as
/// ambiguous rather than falling through to a later kind in search of a
/// single answer.
///
/// Identifiers are matched case-sensitively, both as a full identifier and
/// as a suffix: the spec grants case-insensitivity only to labels (R10;
/// doc-2 section 4). A label is folded with `to_lowercase`, not
/// `eq_ignore_ascii_case`, so a non-ASCII label still folds correctly.
fn resolve<'a, T: 'a, Id>(
    part: &str,
    items: impl IntoIterator<Item = &'a T>,
    id_of: impl Fn(&T) -> Id,
    label_of: impl Fn(&T) -> Option<&str>,
) -> Resolution<Id>
where
    Id: std::fmt::Display,
{
    let items: Vec<&T> = items.into_iter().collect();
    // `matched` is a slice of `&T`, so `.iter()` yields `&&T`; `id_of` and
    // `label_of` both take `&T`, so each call derefs once.
    let candidates_of = |matched: &[&T]| -> Vec<Candidate> {
        matched
            .iter()
            .map(|item| Candidate {
                id: id_of(*item).to_string(),
                label: label_of(*item).map(str::to_string),
            })
            .collect()
    };

    // Kind 1: a full identifier.
    let by_id: Vec<&T> = items
        .iter()
        .copied()
        .filter(|item| id_of(item).to_string() == part)
        .collect();
    match by_id.len() {
        0 => {}
        1 => return Resolution::Found(id_of(by_id[0])),
        _ => return Resolution::Ambiguous(candidates_of(&by_id)),
    }

    // Kind 2: a label, ignoring case.
    let lower = part.to_lowercase();
    let by_label: Vec<&T> = items
        .iter()
        .copied()
        .filter(|item| label_of(item).is_some_and(|label| label.to_lowercase() == lower))
        .collect();
    match by_label.len() {
        0 => {}
        1 => return Resolution::Found(id_of(by_label[0])),
        _ => return Resolution::Ambiguous(candidates_of(&by_label)),
    }

    // Kind 3: a one-based position, in `items`' own order. For a session
    // this is `Tree::sessions()`'s order and for a tab `Session::tabs()`'s
    // (both already ordered, R8); for a pane this is `Tab::panes()`'s
    // order, which R112 fixes as the layout tree's leaves left to right, a
    // split's first child before its second.
    if let Ok(position) = part.parse::<usize>()
        && position >= 1
        && let Some(item) = items.get(position - 1)
    {
        return Resolution::Found(id_of(item));
    }

    // Kind 4: an identifier suffix of four or more characters.
    if part.len() >= 4 {
        let by_suffix: Vec<&T> = items
            .iter()
            .copied()
            .filter(|item| id_of(item).to_string().ends_with(part))
            .collect();
        match by_suffix.len() {
            0 => {}
            1 => return Resolution::Found(id_of(by_suffix[0])),
            _ => return Resolution::Ambiguous(candidates_of(&by_suffix)),
        }
    }

    Resolution::NotFound(candidates_of(&items))
}

fn to_result<Id>(part: &str, resolution: Resolution<Id>) -> Result<Id, Error> {
    match resolution {
        Resolution::Found(id) => Ok(id),
        Resolution::NotFound(candidates) => Err(Error::TargetNotFound {
            part: part.to_string(),
            candidates,
        }),
        Resolution::Ambiguous(candidates) => Err(Error::TargetAmbiguous {
            part: part.to_string(),
            candidates,
        }),
    }
}

/// Splits a target path on `/`, rejecting anything that cannot be a valid
/// path of up to `max` parts: more than `max`, or an empty part (`"a//b"`,
/// `"/a"`, `"a/"`, `""`) — bad usage (doc-2 section 6's status 2), never
/// "matched nothing" (status 3), because an empty part can never come
/// from a real id, label, position or suffix. `str::split` always yields
/// at least one part, even on `""` (which splits to `[""]`), so there is
/// no "no parts at all" case to check separately from the empty-part one.
fn split_path(path: &str, max: usize) -> Result<Vec<&str>, Error> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() > max || parts.iter().any(|part| part.is_empty()) {
        return Err(Error::InvalidTargetPath(path.to_string()));
    }
    Ok(parts)
}

impl Tree {
    /// Resolves a session from its own part of a target (doc-15 R10).
    ///
    /// # Errors
    ///
    /// Returns [`Error::TargetNotFound`] if `part` matches no session, or
    /// [`Error::TargetAmbiguous`] if it matches more than one.
    pub fn resolve_session(&self, part: &str) -> Result<SessionId, Error> {
        to_result(
            part,
            resolve(part, self.sessions(), Session::id, Session::label),
        )
    }

    /// Resolves a tab from a target path, `"[session/]tab"` (doc-2
    /// section 4). `default_session` is used when the session part is
    /// left out; filling an omitted part in from context — a client's own
    /// job (doc-2 section 4) — is not this method's.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidTargetPath`] if `path` has no parts, more
    /// than two, or an empty one; [`Error::SessionNotFound`] if an
    /// explicit session part, or `default_session`, names no session in
    /// the tree; or whatever [`Session::resolve_tab`] returns for the tab
    /// part.
    pub fn resolve_tab_path(&self, path: &str, default_session: SessionId) -> Result<TabId, Error> {
        let parts = split_path(path, 2)?;
        let (session_part, tab_part) = match parts.as_slice() {
            [tab] => (None, *tab),
            [session, tab] => (Some(*session), *tab),
            _ => unreachable!("split_path(path, 2) returns one or two parts"),
        };

        let session_id = match session_part {
            Some(part) => self.resolve_session(part)?,
            None => default_session,
        };
        let session = self
            .session(session_id)
            .ok_or(Error::SessionNotFound(session_id))?;

        session.resolve_tab(tab_part)
    }

    /// Resolves a pane from a target path, `"[session/][tab/]pane"`
    /// (doc-2 section 4). `default_session` and `default_tab` are used
    /// for parts left out, same as [`Tree::resolve_tab_path`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidTargetPath`] if `path` has no parts, more
    /// than three, or an empty one; [`Error::SessionNotFound`] or
    /// [`Error::TabNotFound`] if an explicit part, or a default, names
    /// nothing in the tree; or whatever [`Tab::resolve_pane`] returns for
    /// the pane part.
    pub fn resolve_pane_path(
        &self,
        path: &str,
        default_session: SessionId,
        default_tab: TabId,
    ) -> Result<PaneId, Error> {
        let parts = split_path(path, 3)?;
        let (session_part, tab_part, pane_part) = match parts.as_slice() {
            [pane] => (None, None, *pane),
            [tab, pane] => (None, Some(*tab), *pane),
            [session, tab, pane] => (Some(*session), Some(*tab), *pane),
            _ => unreachable!("split_path(path, 3) returns one to three parts"),
        };

        let session_id = match session_part {
            Some(part) => self.resolve_session(part)?,
            None => default_session,
        };
        let session = self
            .session(session_id)
            .ok_or(Error::SessionNotFound(session_id))?;

        let tab_id = match tab_part {
            Some(part) => session.resolve_tab(part)?,
            None => default_tab,
        };
        let tab = session.tab(tab_id).ok_or(Error::TabNotFound(tab_id))?;

        tab.resolve_pane(pane_part)
    }
}

impl Session {
    /// Resolves a tab from its own part of a target, among this
    /// session's tabs (doc-15 R10).
    ///
    /// # Errors
    ///
    /// Returns [`Error::TargetNotFound`] if `part` matches no tab in this
    /// session, or [`Error::TargetAmbiguous`] if it matches more than one.
    pub fn resolve_tab(&self, part: &str) -> Result<TabId, Error> {
        to_result(part, resolve(part, self.tabs(), Tab::id, Tab::label))
    }
}

impl Tab {
    /// Resolves a pane from its own part of a target, among this tab's
    /// panes (doc-15 R10; position is R112's leaf order).
    ///
    /// # Errors
    ///
    /// Returns [`Error::TargetNotFound`] if `part` matches no pane in this
    /// tab, or [`Error::TargetAmbiguous`] if it matches more than one.
    pub fn resolve_pane(&self, part: &str) -> Result<PaneId, Error> {
        to_result(part, resolve(part, self.panes(), Pane::id, Pane::label))
    }
}

#[cfg(test)]
mod tests {
    use super::{Resolution, resolve, split_path};
    use crate::error::Error;
    use crate::tree::Tree;
    use std::fmt;

    /// A minimal candidate for testing `resolve` directly, without
    /// building a `Session`, `Tab` or `Pane`: an id (a bare string, so
    /// tests can use whatever shape of "identifier" a case needs) and an
    /// optional label.
    struct Item {
        id: &'static str,
        label: Option<&'static str>,
    }

    #[derive(Debug)]
    struct ItemId(&'static str);

    impl fmt::Display for ItemId {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    fn id_of(item: &Item) -> ItemId {
        ItemId(item.id)
    }

    fn label_of(item: &Item) -> Option<&str> {
        item.label
    }

    fn found<T>(resolution: Resolution<T>) -> T {
        match resolution {
            Resolution::Found(id) => id,
            Resolution::NotFound(candidates) => {
                panic!("expected a match, got NotFound({candidates:?})")
            }
            Resolution::Ambiguous(candidates) => {
                panic!("expected a match, got Ambiguous({candidates:?})")
            }
        }
    }

    #[test]
    fn resolve_matches_a_full_identifier_before_any_other_kind() {
        let items = [
            Item {
                id: "00000001",
                label: Some("00000001"), // same text as another item's id
            },
            Item {
                id: "00000002",
                label: None,
            },
        ];

        let id = found(resolve("00000002", &items, id_of, label_of));

        assert_eq!(id.0, "00000002");
    }

    #[test]
    fn resolve_matches_a_label_case_insensitively_when_no_identifier_matches() {
        let items = [
            Item {
                id: "00000001",
                label: Some("Logs"),
            },
            Item {
                id: "00000002",
                label: Some("errors"),
            },
        ];

        let id = found(resolve("logs", &items, id_of, label_of));

        assert_eq!(id.0, "00000001");
    }

    #[test]
    fn resolve_matches_by_one_based_position_when_no_identifier_or_label_matches() {
        let items = [
            Item {
                id: "00000001",
                label: None,
            },
            Item {
                id: "00000002",
                label: None,
            },
        ];

        let id = found(resolve("2", &items, id_of, label_of));

        assert_eq!(id.0, "00000002");
    }

    #[test]
    fn resolve_matches_an_identifier_suffix_of_four_or_more_characters() {
        let items = [
            Item {
                id: "0000abcd",
                label: None,
            },
            Item {
                id: "00001234",
                label: None,
            },
        ];

        let id = found(resolve("abcd", &items, id_of, label_of));

        assert_eq!(id.0, "0000abcd");
    }

    #[test]
    fn resolve_reports_ambiguous_when_the_suffix_kind_matches_more_than_one() {
        let items = [
            Item {
                id: "10001234",
                label: None,
            },
            Item {
                id: "20001234",
                label: None,
            },
            Item {
                id: "00005678",
                label: None,
            },
        ];

        // Both of the first two ids end in "1234"; the third does not, so
        // it must not be in the result (nothing "later" is tried past an
        // ambiguous match, but the third item is a distractor at the same
        // kind, and must still be left out).
        match resolve("1234", &items, id_of, label_of) {
            Resolution::Ambiguous(candidates) => {
                assert_eq!(candidates.len(), 2);
                assert!(candidates.iter().any(|c| c.id == "10001234"));
                assert!(candidates.iter().any(|c| c.id == "20001234"));
            }
            other @ (Resolution::Found(_) | Resolution::NotFound(_)) => {
                panic!("expected Ambiguous, got a different resolution: {other:?}")
            }
        }
    }

    #[test]
    fn resolve_never_tries_a_suffix_shorter_than_four_characters() {
        let items = [Item {
            id: "0000abc",
            label: None,
        }];

        // "abc" is a three-character suffix of the only item's id, but is
        // too short to try as a suffix (kind four needs four or more) and
        // does not parse as a position either, so it is not found.
        match resolve("abc", &items, id_of, label_of) {
            Resolution::NotFound(candidates) => assert_eq!(candidates.len(), 1),
            other @ (Resolution::Found(_) | Resolution::Ambiguous(_)) => {
                panic!("expected NotFound, got a different resolution: {other:?}")
            }
        }
    }

    #[test]
    fn resolve_stops_at_label_even_when_an_ambiguous_label_match_would_also_match_by_suffix() {
        // Two items share the label "work", which is itself a valid
        // four-character suffix of a third item's id. If kind four (the
        // suffix) were ever reached, "work" would also match that third
        // item uniquely — but R10's label kind has already found two
        // matches by then, and stops the search as ambiguous rather than
        // falling through looking for the single suffix match.
        let items = [
            Item {
                id: "00000001",
                label: Some("work"),
            },
            Item {
                id: "00000002",
                label: Some("work"),
            },
            Item {
                id: "0000work",
                label: None,
            },
        ];

        match resolve("work", &items, id_of, label_of) {
            Resolution::Ambiguous(candidates) => assert_eq!(candidates.len(), 2),
            other @ (Resolution::Found(_) | Resolution::NotFound(_)) => {
                panic!("expected Ambiguous, got a different resolution: {other:?}")
            }
        }
    }

    #[test]
    fn resolve_reports_not_found_with_every_candidate_when_nothing_matches() {
        let items = [
            Item {
                id: "00000001",
                label: Some("logs"),
            },
            Item {
                id: "00000002",
                label: None,
            },
        ];

        match resolve("nonsense", &items, id_of, label_of) {
            Resolution::NotFound(candidates) => {
                assert_eq!(candidates.len(), 2);
                assert!(candidates.iter().any(|c| c.id == "00000001"));
                assert!(candidates.iter().any(|c| c.id == "00000002"));
            }
            other @ (Resolution::Found(_) | Resolution::Ambiguous(_)) => {
                panic!("expected NotFound, got a different resolution: {other:?}")
            }
        }
    }

    #[test]
    fn resolve_reports_ambiguous_with_only_the_matched_candidates_when_more_than_one_matches() {
        let items = [
            Item {
                id: "00000001",
                label: Some("logs"),
            },
            Item {
                id: "00000002",
                label: Some("logs"),
            },
            Item {
                id: "00000003",
                label: Some("errors"),
            },
        ];

        match resolve("logs", &items, id_of, label_of) {
            Resolution::Ambiguous(candidates) => {
                assert_eq!(candidates.len(), 2);
                assert!(
                    candidates
                        .iter()
                        .all(|c| c.label == Some("logs".to_string()))
                );
            }
            other @ (Resolution::Found(_) | Resolution::NotFound(_)) => {
                panic!("expected Ambiguous, got a different resolution: {other:?}")
            }
        }
    }

    #[test]
    fn split_path_rejects_a_path_with_more_parts_than_the_maximum() {
        assert_eq!(
            split_path("a/b/c/d", 3),
            Err(Error::InvalidTargetPath("a/b/c/d".to_string()))
        );
    }

    #[test]
    fn split_path_rejects_an_empty_segment() {
        assert_eq!(
            split_path("a//b", 3),
            Err(Error::InvalidTargetPath("a//b".to_string()))
        );
        assert_eq!(
            split_path("/a", 3),
            Err(Error::InvalidTargetPath("/a".to_string()))
        );
        assert_eq!(
            split_path("a/", 3),
            Err(Error::InvalidTargetPath("a/".to_string()))
        );
        assert_eq!(
            split_path("", 3),
            Err(Error::InvalidTargetPath(String::new()))
        );
    }

    #[test]
    fn split_path_accepts_up_to_the_maximum_number_of_parts() {
        assert_eq!(split_path("a/b/c", 3), Ok(vec!["a", "b", "c"]));
        assert_eq!(split_path("pane", 3), Ok(vec!["pane"]));
    }

    #[test]
    fn tree_resolve_session_resolves_by_label_and_errors_when_ambiguous() {
        let mut tree = Tree::new();
        let first = tree.new_session(Some("work".to_string())).unwrap();
        let _second = tree.new_session(Some("play".to_string())).unwrap();

        assert_eq!(tree.resolve_session("work").unwrap(), first);
        assert_eq!(
            tree.resolve_session("nonsense"),
            Err(Error::TargetNotFound {
                part: "nonsense".to_string(),
                candidates: vec![
                    super::Candidate {
                        id: first.to_string(),
                        label: Some("work".to_string()),
                    },
                    super::Candidate {
                        id: tree.sessions()[1].id().to_string(),
                        label: Some("play".to_string()),
                    },
                ],
            })
        );
    }

    #[test]
    fn session_resolve_tab_and_tab_resolve_pane_use_the_same_order() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let second_tab = tree.new_tab(session_id, Some("logs".to_string())).unwrap();
        let session = tree.session(session_id).unwrap();

        assert_eq!(session.resolve_tab("logs").unwrap(), second_tab);
        assert_eq!(session.resolve_tab("2").unwrap(), second_tab);

        let tab = session.tab(second_tab).unwrap();
        let pane_id = tab.panes()[0].id();
        assert_eq!(tab.resolve_pane("1").unwrap(), pane_id);
    }

    #[test]
    fn resolve_tab_path_falls_back_to_the_default_session_when_the_session_part_is_left_out() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let tab_id = tree.new_tab(session_id, Some("logs".to_string())).unwrap();

        assert_eq!(tree.resolve_tab_path("logs", session_id).unwrap(), tab_id);
    }

    #[test]
    fn resolve_tab_path_uses_an_explicit_session_part_over_the_default() {
        let mut tree = Tree::new();
        let other_session = tree.new_session(Some("other".to_string())).unwrap();
        let session_id = tree.new_session(Some("work".to_string())).unwrap();
        let tab_id = tree.new_tab(session_id, Some("logs".to_string())).unwrap();

        assert_eq!(
            tree.resolve_tab_path("work/logs", other_session).unwrap(),
            tab_id
        );
    }

    #[test]
    fn resolve_pane_path_falls_back_to_the_given_defaults_when_parts_are_left_out() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let tab_id = tree.new_tab(session_id, None).unwrap();
        let first_pane = tree
            .session(session_id)
            .unwrap()
            .tab(tab_id)
            .unwrap()
            .panes()[0]
            .id();
        let pane_id = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();
        tree.session_mut(session_id)
            .unwrap()
            .tab_mut(tab_id)
            .unwrap()
            .pane_mut(pane_id)
            .unwrap()
            .set_label(Some("logs".to_string()))
            .unwrap();

        assert_eq!(
            tree.resolve_pane_path("logs", session_id, tab_id).unwrap(),
            pane_id
        );
    }

    #[test]
    fn resolve_pane_path_rejects_a_path_with_the_wrong_number_of_segments() {
        // `split_path` rejects the path before any id is looked up, so
        // the default session and tab given here need not exist.
        let tree = Tree::new();
        let dummy_session = steg_sdk::SessionId::from_raw(0);
        let dummy_tab = steg_sdk::TabId::from_raw(0);

        assert_eq!(
            tree.resolve_pane_path("a/b/c/d", dummy_session, dummy_tab),
            Err(Error::InvalidTargetPath("a/b/c/d".to_string()))
        );
    }
}
