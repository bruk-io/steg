---
id: TASK-1.42
title: 'Kernel: clients, what each views, and the size a terminal takes'
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T16:58:00Z'
labels:
  - size-3
  - changes-requested
milestone: m-0
dependencies:
  - TASK-1.39
references:
  - task-1.42-kernel-clients-what-each-views-and-the
  - https://github.com/bruk-io/steg/pull/29
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-12
asdlc_branch: task-1.42-kernel-clients-what-each-views-and-the
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '571c32803ab2'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=1,nit=4'
asdlc_review_policies: ["review"]
asdlc_plan_matched: yes
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-15: R86, R89, R91. decision-5 and decision-6.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A client can connect and leave, and which sessions it views follows from what it is attached to
- [ ] #2 The latest client in a session is the last to send input or declare a new size
- [ ] #3 A terminal takes the size the latest client declared for it, and every other viewer is told that size
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Scope: kernel-only data model for clients, viewing, "latest", and the size a terminal
takes (doc-15 R86, R89, R91; decision-5; decision-6). Builds on main as merged
(terminal.rs/screen.rs from TASK-1.39's PR, and the sdk's serde/description/record
additions from TASK-1.43's PR). No server, no socket: this task returns plain data
(`SizeChanged`) that TASK-1.40's `PaneHandle::resize` and TASK-1.44/45/47's connection
and attachment streaming consume, but builds against none of them directly - there is
no code dependency on TASK-1.40 landing first, only a naming one (cited below, where
it closes a gap this plan previously left unnamed).

THE UNIT (settles brief point 4, flagged as this plan's one real decision):
doc-14 says "each attachment declares its own size... the size the terminal would
have in that client" (per pane, per attachment). But the kernel already on main holds
size per TAB (`Tab.size: Option<Size>`), and R100's floor is judged only from a tab
total through `Layout::leaf_sizes` (`tab.rs` `satisfies_floor_sizes`/`would_violate_
floor_inserting`). A client declaring per-pane sizes directly can't feed that floor
check (there is no total to invert leaf_sizes from) and can't keep R95's shared
proportions true (two clients' independent per-pane sets need not tile). So: what a
client declares IN is a whole tab's size in cells (the tab it is viewing, since R89/
decision-5 point 5 - all viewers of a session share one current tab). That becomes,
when the client is latest, the existing `Tab.size` via the existing `Tree::set_tab_
size`. What a client is told, OUT, per pane, is `Layout::leaf_sizes(tab.size)` -
exactly doc-14's "an update gains an optional size" per pane. doc-14's "per-attachment
size" is therefore the report direction, not the accept direction; nothing on the
wire doc changes, this is just naming which half of it this task builds. Flagging
this explicitly for review since a reviewer holding doc-14's words literally will
read a tab-level kernel API as contradicting it otherwise.

DATA MODEL
- sdk (`crates/sdk/src/ids.rs`): add `ClientId`, the full wire form the other three ids
  now have on main (landed after this plan's first draft, from TASK-1.43's work merging
  in): `from_raw`/`into_raw`, `Display` (hex, >=8 digits), `FromStr` parsing `Display`'s
  own string, and `Serialize`/`Deserialize` via `collect_str`/parse-from-string - exactly
  `SessionId`'s four impls, copied. Mirror its five tests too (round-trip through
  `into_raw`, `FromStr` parses what `Display` prints, rejects non-hex, serde round-trips
  through that same string, serde rejects non-hex, display width). Update the module doc
  comment ("a session, a tab, a pane" -> "..., a pane and a client").
- sdk (new `crates/sdk/src/client_kind.rs`, mirroring `pane_kind.rs` exactly, including its
  serde: `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]`,
  `#[serde(rename_all = "snake_case")]`, and a round-trip test): `ClientKind` enum `App`,
  `Cli`, `Plugin` (doc-14: the hello's `kind`). Exported from `lib.rs`. TASK-1.70 (sdk
  protocol logic, planned in parallel) also needs this type for its `ClientHello` and has
  been told to use this one rather than define its own - whichever of the two tasks lands
  second just imports it, no merge conflict on either side.
- kernel (new `crates/kernel/src/client.rs`): `Client { id: ClientId, kind: ClientKind,
  label: Option<String>, viewing: Vec<PaneId>, declared: Vec<(TabId, Size)>, activity:
  Vec<(SessionId, u64)> }` with read-only getters (`id`, `kind`, `label`, `viewing`).
  `viewing`/`declared`/`activity` stay crate-private, mutated only through `Tree`, to
  keep "who is latest" derivable in one place (`Tree`) rather than recomputable
  wrongly from two structs. Plain `Vec`, linear scan: matches every other collection
  in this crate (`Tab.panes`, `Session.tabs`), no new dependency, and these lists are
  at most a handful of entries.
  No label validation: `label::validate` exists for R10's target grammar (no `/`, not
  all-digits), and R10 does not make a client addressable by label at all (doc-15's
  table of addressable things is sessions/tabs/panes only). Reusing it for clients
  would invent a rule nothing asks for. Flagging this as a judgement call to confirm,
  not a silent decision: doc-14 just says the hello "gains... an optional label" with
  no format rule.
- kernel (`ids.rs`): add `Ids::next_client(&mut self) -> ClientId`, same shared
  counter as the other three (`next_raw`) - identity minting, not activity ordering,
  so this is exactly the existing pattern, no new design.
- kernel (`tree.rs`): add a private `activity: u64` field on `Tree` (not on `Ids`:
  activity order is "has this happened more recently than that", never serialised,
  never an identifier - conflating it with id minting would leak sequence semantics
  into identifiers that must never carry one). A private `fn tick(&mut self) -> u64`
  increments and returns it. This is not reading a clock - it is the kernel advancing
  its own state - so `crates/kernel/clippy.toml`'s disallowed list does not fire and
  no trait/double is needed for `just test`'s no-doubles rule.

DERIVED, NEVER STORED
- "Viewing": `Client.viewing` is attachment data (the smallest thing TASK-1.47 can
  drive - quoting doc-14: "'which sessions does this client view' is never sent by
  the client; the server reads it off the attachments"). There is no `views: Vec
  <SessionId>` field anywhere; `Tree::sessions_viewed(client)` derives it by mapping
  each viewed pane to its owning session (dedup).
- "Latest in a session": not stored either. `Tree::latest_client(session)` derives
  it as the client, among those currently viewing (`sessions_viewed` includes this
  session), with the highest `activity` entry for that session. `None` if nobody
  views it. This makes "what happens when the latest leaves" fall out for free: it is
  recomputed fresh from whoever is left, with no special-cased departure state to
  keep in sync. decision-6 does not say what happens when the latest client leaves;
  this plan decides it (fall back to the next most recently active remaining viewer,
  or no latest at all) and flags it as reversible, not as settled by the decision
  record.
- A declaration always wins: declaring a size is itself an activity event, so right
  after it, that client trivially holds the freshest tick of anyone - it is
  unconditionally the new latest, and its size is unconditionally applied. The one
  place a client's stored `declared` size matters without a fresh declare is when
  the latest changes for another reason (input elsewhere, or a departure): then the
  newly-latest client's last-declared size for each tab it views is what gets
  (re)applied, which is why `declared` has to be kept per client rather than computed
  only at declare-time. This is the one subtle clause worth its own test.

TREE API (house pattern: a "what happened" struct per operation, like `Focused`/
`ClosedPane`/`MovedPane`/`TabClosed`, so TASK-1.46 raises one event with no further
lookup - and `Candidate`'s own precedent for keeping wire-shape mapping out of the
kernel: "mapping a kernel error to a wire shape is the API description's job
(TASK-1.43), not this module's").

New struct in `tree.rs`:
  SizeChanged { tab: TabId, size: Size, panes: Vec<(PaneId, Size)>, viewers:
  Vec<ClientId> }
  - `panes` is `Layout::leaf_sizes(size)` - what the server resizes each pane's
    Terminal to and reports to each attachment (AC #3's "told that size").
  - `viewers` is every client currently viewing this tab, from `clients_viewing`.

New `Tree` methods:
  - `connect_client(kind, label) -> ClientId` - mints via `Ids::next_client`. Nothing
    about focus or any session/tab/pane touched (AC #1, decision-5 point 4; tested).
  - `clients(&self) -> &[Client]`, `client(&self, id) -> Option<&Client>`.
  - `disconnect_client(&mut self, id) -> Result<Vec<SizeChanged>, Error>` - removes
    the client, then for every session it was viewing, recomputes `latest_client` and
    reapplies that client's declared size for each tab it views there (shared helper
    below). `Error::ClientNotFound(ClientId)` if unknown.
  - `view_pane(&mut self, client, pane) -> Result<(), Error>` - adds `pane` to
    `client.viewing` if not already there (idempotent). No size, no activity: viewing
    alone is not "input or resize" (R91's own wording). `Error::ClientNotFound`,
    `Error::PaneNotFound`.
  - `stop_viewing(&mut self, client, pane) -> Result<Vec<SizeChanged>, Error>` -
    removes `pane`; if that leaves the client viewing nothing in `pane`'s session,
    runs the same recompute-and-reapply helper `disconnect_client` uses, scoped to
    that one session. `Error::ClientNotFound`, `Error::ClientNotViewingPane { client,
    pane }` if it wasn't viewing it.
  - `declare_size(&mut self, client, tab, size) -> Result<SizeChanged, Error>` -
    requires the client to already view some pane in `tab`'s session (else
    `Error::ClientNotViewingTab { client, tab }` - a client must attach before it
    declares, matching doc-14's "at attach and whenever..."); records `size` in
    `client.declared`; ticks activity for (client, tab's session); calls the existing
    `Tree::set_tab_size(tab, size)` unconditionally (AC #3, R100's last clause: a
    declaration never fails the floor, it only crops/pads - tested); returns the
    `SizeChanged`. `Error::TabNotFound`, `Error::ClientNotFound` also possible.
  - `note_input(&mut self, client, pane) -> Result<Option<SizeChanged>, Error>` -
    requires viewing (`Error::ClientNotViewingPane`); ticks activity for (client,
    pane's session); looks up this client's `declared` size for pane's tab; if none,
    or it already equals the tab's current size, returns `Ok(None)` (no-op); else
    applies it via `set_tab_size` and returns `Ok(Some(SizeChanged{..}))` - this is
    the "latest changed by input, not by a fresh declare" path (AC #2's "last to send
    input", tested).
  - `sessions_viewed(&self, client) -> Result<Vec<SessionId>, Error>`,
    `clients_viewing(&self, session) -> Result<Vec<ClientId>, Error>`,
    `latest_client(&self, session) -> Result<Option<ClientId>, Error>` - read-only
    derivations, exposed directly so tests (and later the server) can assert/query
    them without relying on a side effect's return value.
  - Private helper `fn reapply_latest(&mut self, session) -> Vec<SizeChanged>`: the
    recompute-and-reapply logic shared by `disconnect_client` and `stop_viewing`.
  - A private immutable `fn session_owning_pane(&self, pane) -> Option<SessionId>`
    (today only `tab_owning_pane_mut`/`session_owning_tab_mut` exist, both `_mut`,
    `tree.rs:174`/`208`). Used by `declare_size`, `note_input`, `sessions_viewed`.

VISIBILITY: `Tree::set_tab_size` (`tree.rs:575`) narrows from `pub` to `pub(crate)`.
Its own doc comment already names this task's caller ("the server... calls this
whenever it learns a new size"); now that caller is `declare_size`/`reapply_latest`
inside this same crate, and nothing outside the kernel should set a tab's size
except through a client's declared activity. Flagging this visibility change
explicitly since it narrows an existing public method - confirm with the engineer/
reviewer rather than treating it as a plain internal refactor.

Paired with that narrowing: add `Tab::size(&self) -> Option<Size>` (`tab.rs`, beside
`fill`/`current`), a public read-only getter - today there is only the setter. TASK-1.75
(server dispatch) flagged that it cannot serve `pane resize` without a way to read a tab's
current size (e.g. for `pane show`/`tab show`'s report), independent of this task's own
write path. Read-only, so it does not reopen who may set the size - `set_tab_size` stays
the only setter and still narrows to `pub(crate)` as above.

CLEANUP ON CLOSE: `Tree::close_pane`/`close_tab`/`close_session` (`tree.rs` ~193-289)
must also drop the closed pane(s) from every client's `viewing` and `declared` lists,
and run `reapply_latest` for any session whose latest client's view just changed as a
result. Existing tests for those three methods must keep passing unmodified; new
tests cover a client viewing a pane that gets closed out from under it.

PIXEL SIZE / TERMINAL WIRING (brief point 4, last clause): TASK-1.40's plan (approved by
@bruk.o) names the join point this plan could before only guess at. One thread per pane
owns the kernel `Terminal` for the pane's whole life, behind `PaneHandle`
(`crates/server/src/pane.rs`); `PaneHandle::resize(cols, rows)` "resizes the Terminal and
the pty master together, one call", and that plan names this task as its caller directly:
"TASK-1.42's job is feeding it a real client-declared size." So the join is: for every
`(pane, size)` pair in a `SizeChanged.panes`, the server calls that pane's
`PaneHandle::resize(size.columns(), size.rows())`. This task supplies `SizeChanged.panes`
itself; TASK-1.40 supplies the call it feeds into; whichever of TASK-1.44/45/47 wires a real
connection's declared size into `Tree::declare_size`/`note_input` is what runs both ends at
once. Nothing to build here beyond what is already in this plan - citing it so the plan
reads as landing in a known slot, not an unnamed one.
Pixel geometry itself (`engine.resize`'s width/height, for CSI 14/16/18 t and mode 2048) is
still not carried by either plan: `PaneHandle::resize`'s own signature is `(cols, rows)`,
no pixel parameters, and `Size` (`sdk/layout.rs:91`) stays explicitly cells-only ("never
pixels"), as R102/R105 require. That stays a genuinely open, unassigned gap after both
plans - flagged again rather than guessed at, since neither task closes it.

ORDER OF WORK
1. sdk: `ClientId` in `ids.rs` with its full wire form (`from_raw`/`into_raw`/`Display`/
   `FromStr`/`Serialize`/`Deserialize`) and its five mirrored tests; `ClientKind` in new
   `client_kind.rs` with its serde derives and round-trip test, mirroring `pane_kind.rs`.
   Export both from `lib.rs`.
2. kernel: `Ids::next_client`; new `client.rs` with `Client` and its getters; export
   `Client` from `lib.rs`.
3. kernel: `Tab::size(&self) -> Option<Size>` getter (`tab.rs`) - independent of
   everything else here, lands early so TASK-1.75 can pick it up as soon as this task's
   branch is up, rather than waiting on steps 4-7.
4. kernel: `Tree` gains the `clients: Vec<Client>` field and `activity: u64` counter
   plus `tick`; `connect_client`, `clients`, `client`, `disconnect_client` (with
   `reapply_latest` stubbed to "nothing to reapply yet" until step 6).
5. kernel: `view_pane`, `stop_viewing`, `sessions_viewed`, `clients_viewing`, and the
   private `session_owning_pane` helper. Tests for AC #1: connect, view, viewing
   derives the session list, stop_viewing/disconnect remove it, focus never moves.
6. kernel: `declare_size`, `note_input`, `latest_client`, `reapply_latest` filled in,
   narrow `set_tab_size` to `pub(crate)`. Tests for AC #2 and #3: latest tracks input
   and declares; a declaration always wins; departure falls back to the next viewer
   or to none; a smaller declared size never errors on the floor; `SizeChanged.panes`
   matches `leaf_sizes`; `viewers` lists every client on that tab.
7. Wire the close-pane/tab/session cleanup (above) and its tests.
8. `error.rs`: add `ClientNotFound`, `ClientNotViewingPane`, `ClientNotViewingTab`,
   with `Display`/`source` arms alongside the existing ones.
9. `just check` for the kernel and sdk crates; fix anything the workspace-wide lints
   (`boundaries.toml`, `clippy.toml`) flag.

RISKS
- The close-cascade touch points (step 7) are the part most likely to have a case
  this plan missed (e.g. a pane closing because it was the tab's last one, cascading
  to the session, while a client views exactly that pane) - budget time for it, not
  just the straight-line API.
- `pub(crate)` on `set_tab_size` could break a test outside this crate if one exists;
  confirmed by grep there is none today, but re-check after step 6.
- The doc-14 unit framing (this plan's main call) is the one point most likely to
  draw review pushback; said explicitly above so it is discussed before, not during,
  review.

PROOF (test -> AC clause)
- AC #1 "connect and leave": `connect_client` then `disconnect_client` round-trip;
  `clients()`/`client()` reflect it.
- AC #1 "viewing follows from attachment": `sessions_viewed` is empty before
  `view_pane`, non-empty after, empty again after `stop_viewing`/`disconnect_client`;
  no setter for "views" exists to call directly.
- AC #1 "focus never changes": one test exercises connect/view/declare/note_input/
  disconnect and asserts the tree's focus path (current_tab, current_pane) is
  unchanged throughout (R88, decision-5 point 4).
- AC #2 "latest is last input or declared size": `latest_client` flips to whichever
  client most recently called `declare_size` or `note_input` on a pane in that
  session; a client that only `view_pane`s (no declare, no input) never becomes
  latest; departure falls back to the next most recently active remaining viewer, or
  `None` with nobody left.
- AC #3 "terminal takes the latest's declared size": `declare_size`'s `SizeChanged.
  panes` matches `Layout::leaf_sizes(size)` for the tab's current layout; and, proving
  "a declaration always wins" directly: two clients each declare a different size on
  the same tab, in turn, and the tab's size is always the one most recently declared,
  regardless of magnitude (smaller or larger than the one before it).
- AC #3 "every other viewer is told": `SizeChanged.viewers` lists every client
  `view_pane`-ed onto that tab's panes, including ones that didn't trigger the
  change.
- R100 "a declaration never fails the floor": `declare_size` with a size smaller than
  R100's floor still succeeds and is applied (mirrors the existing `set_tab_size_
  smaller_always_succeeds_and_closes_nothing` test at `tree.rs:1589`).

Commands: `just check` (kernel + sdk affected), `just test`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Revised plan checked against doc-15 (R86, R89, R91), doc-14, decision-5 and decision-6 by @bruk.o: ok. A client is kernel data with an id and a kind defined once in the sdk; what it views is derived from the panes it is attached to; the latest client in a session is derived from a counter the kernel advances, never a clock; a client declares a whole tab's size in cells, which becomes the tab's size when that client is the latest, and every viewer is told each pane's size. When the latest client leaves, the next most recently active viewer's declared size applies. Tab::size is added as a getter and set_tab_size narrows to the crate. For the engineer: a client declares a tab's size, not a size per pane, and what happens when the latest leaves is this plan's choice. Pixel size stays a named gap for the server task that joins a pane to its terminal.

Check passed: `just check` exited 0 at 2026-11-15T16:30:48+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.42 on task-1.42-kernel-clients-what-each-views-and-the at c8dc7cdd8d3d:

```text
ok      format check (0.1s)
ok      lint (103.7s)
ok      boundaries (0.2s)
ok      licences (0.7s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.0s)
ok      test (steg-sdk) (1.3s)
ok      build (steg-kernel) (46.4s)
ok      test (steg-kernel) (1.0s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (0.4s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (22.9s)
ok      test (steg-app) (0.9s)
ok      tool tests (1.9s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Check passed: `just test` exited 0 at 2026-11-15T16:32:37+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.42 on task-1.42-kernel-clients-what-each-views-and-the at c8dc7cdd8d3d with uncommitted changes:

```text
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (42.3s)
```

Check passed: `just test` exited 0 at 2026-11-15T16:44:20+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.42 on task-1.42-kernel-clients-what-each-views-and-the at c8dc7cdd8d3d with uncommitted changes:

```text
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (0.4s)
```

Check FAILED: `just check` exited 1 at 2026-11-15T16:44:26+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.42 on task-1.42-kernel-clients-what-each-views-and-the at c8dc7cdd8d3d with uncommitted changes, last 40 of 64 lines:

```text
         assert_eq!(changed.tab, tab_id);
         assert_eq!(changed.size, steg_sdk::Size::new(80, 24));
         assert_eq!(
[31m-            tree.session(session_id).unwrap().tab(tab_id).unwrap().size(),
(B[m[32m+            tree.session(session_id)
(B[m[32m+                .unwrap()
(B[m[32m+                .tab(tab_id)
(B[m[32m+                .unwrap()
(B[m[32m+                .size(),
(B[m             Some(steg_sdk::Size::new(80, 24))
         );
     }
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.42/crates/kernel/src/tree.rs:2443:
         let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
         let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
         let second_pane = tree
[31m-            .split_pane(first_pane, steg_sdk::Direction::Right, steg_sdk::PaneKind::Terminal)
(B[m[32m+            .split_pane(
(B[m[32m+                first_pane,
(B[m[32m+                steg_sdk::Direction::Right,
(B[m[32m+                steg_sdk::PaneKind::Terminal,
(B[m[32m+            )
(B[m             .unwrap();
 
         let leaving = tree.connect_client(steg_sdk::ClientKind::App, None);
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.42/crates/kernel/src/tree.rs:2468:
         assert_eq!(tree.sessions_viewed(leaving).unwrap(), Vec::new());
         assert_eq!(tree.latest_client(session_id).unwrap(), Some(staying));
         assert_eq!(
[31m-            tree.session(session_id).unwrap().tab(tab_id).unwrap().size(),
(B[m[32m+            tree.session(session_id)
(B[m[32m+                .unwrap()
(B[m[32m+                .tab(tab_id)
(B[m[32m+                .unwrap()
(B[m[32m+                .size(),
(B[m             Some(steg_sdk::Size::new(100, 30)),
             "the surviving tab falls back to the remaining viewer's declared size"
         );
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 117 with exit code 1
```

Check FAILED: `just check` exited 101 at 2026-11-15T16:44:41+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.42 on task-1.42-kernel-clients-what-each-views-and-the at c8dc7cdd8d3d with uncommitted changes, last 40 of 65 lines:

```text
     |         ++

error: docs for function which may panic missing `# Panics` section
    --> crates/kernel/src/tree.rs:1111:5
     |
1111 | /     pub fn note_input(
1112 | |         &mut self,
1113 | |         client: ClientId,
1114 | |         pane: PaneId,
1115 | |     ) -> Result<Option<SizeChanged>, Error> {
     | |___________________________________________^
     |
note: first possible panic found here
    --> crates/kernel/src/tree.rs:1119:23
     |
1119 |           let session = self
     |  _______________________^
1120 | |             .session_id_owning_tab(tab)
1121 | |             .expect("a tab found above always has an owning session");
     | |_____________________________________________________________________^
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#missing_panics_doc

error: docs for function which may panic missing `# Panics` section
    --> crates/kernel/src/tree.rs:1200:5
     |
1200 |     pub fn latest_client(&self, session: SessionId) -> Result<Option<ClientId>, Error> {
     |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     |
note: first possible panic found here
    --> crates/kernel/src/tree.rs:1205:30
     |
1205 |                 let client = self.client(id).expect("just listed as a viewer");
     |                              ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#missing_panics_doc

error: could not compile `steg-kernel` (lib) due to 8 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `steg-kernel` (lib test) due to 8 previous errors
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-15T16:45:48+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.42 on task-1.42-kernel-clients-what-each-views-and-the at c8dc7cdd8d3d with uncommitted changes:

```text
ok      format check (0.1s)
ok      lint (0.6s)
ok      boundaries (0.1s)
ok      licences (0.6s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (1.2s)
ok      build (steg-sdk) (0.6s)
ok      test (steg-sdk) (1.1s)
ok      build (steg-kernel) (0.9s)
ok      test (steg-kernel) (1.2s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (0.4s)
ok      build (steg) (0.2s)
ok      test (steg) (0.3s)
ok      build (steg-app) (1.9s)
ok      test (steg-app) (1.0s)
ok      tool tests (1.1s)
ok      architecture (1.0s)
ok      recipe names (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-15T16:46:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.42 on task-1.42-kernel-clients-what-each-views-and-the at b99b85096db9:

```text
ok      format check (0.1s)
ok      lint (0.8s)
ok      boundaries (0.1s)
ok      licences (0.8s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (2.1s)
ok      build (steg-sdk) (0.3s)
ok      test (steg-sdk) (0.9s)
ok      build (steg-kernel) (0.5s)
ok      test (steg-kernel) (0.8s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (0.4s)
ok      build (steg) (0.2s)
ok      test (steg) (0.4s)
ok      build (steg-app) (1.0s)
ok      test (steg-app) (1.0s)
ok      tool tests (1.7s)
ok      architecture (1.5s)
ok      recipe names (0.1s)
```

Built as planned, no departures. ClientKind/ClientId landed first (no conflict with TASK-1.70, which has not yet merged).

Checks recorded on the task: just test (clean build, 0 new changes), just test (after full implementation, 172 kernel + 70 sdk tests passing), just check (full suite, format/lint/boundaries/licences/every package/architecture - all green), just check again after rebasing onto the latest origin/main (no code conflicts from TASK-1.40/1.70/1.74/1.75, which had only pushed board/task-file commits by then). CI on PR #29 passed.

Tests mapped to acceptance criteria:
- AC #1 (connect/leave; viewing follows from attachment): connect_client_then_disconnect_client_round_trips, disconnect_client_rejects_an_unknown_id, viewing_follows_from_attachment_not_a_separate_setter, view_pane_is_idempotent_and_rejects_an_unknown_pane, stop_viewing_rejects_a_pane_the_client_does_not_view, and connecting_viewing_declaring_and_disconnecting_never_change_focus (decision-5 point 4, R88).
- AC #2 (latest is last input or declared size): viewing_alone_never_makes_a_client_latest, latest_client_is_none_when_nobody_views_the_session, latest_client_tracks_whoever_last_declared_or_sent_input, latest_falls_back_to_the_next_most_recently_active_remaining_viewer_on_departure (also proves the reapplied size on fallback).
- AC #3 (terminal takes the latest's declared size; every other viewer is told): a_declaration_always_wins_regardless_of_magnitude, declare_size_reports_every_panes_absolute_size_and_every_viewer (SizeChanged.panes matches leaf_sizes, viewers lists every attached client), declare_size_never_fails_the_floor (R100's last clause), declare_size_requires_viewing_a_pane_in_the_tabs_session, note_input_reapplies_the_now_latest_clients_last_declared_size, note_input_is_a_no_op_when_the_client_has_no_declared_size, note_input_rejects_a_pane_the_client_does_not_view.
- Cleanup on close (the plan's named risk area): closing_a_pane_a_client_views_drops_the_attachment_and_reapplies_the_session, closing_a_tab_drops_its_declared_size_record_from_every_client, closing_a_session_drops_every_clients_attachment_and_activity_for_it.

Also added (per the plan's revision, approved alongside the build): Tab::size() public getter, paired with narrowing Tree::set_tab_size and Tab::set_size to pub(crate).
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T16:26:00Z

---

Plan approved by @bruk (571c32803ab2, size-3): its worker may build it.

---

author: @bruk.r1
created: 2026-11-15T16:49:00Z

---

@bruk.r1 is reviewing TASK-1.42: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r1
created: 2026-11-15T16:58:00Z

---

Review by @bruk.r1 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 4 nit. Cites: review. The diff matches the plan.

[Bugs, Important] crates/kernel/src/tree.rs:2216 connecting_viewing_declaring_and_disconnecting_never_change_focus builds a tree with exactly one tab and one pane, so session.current_tab() and tab.current() are structurally forced to that single id no matter what happens inside connect_client/view_pane/declare_size/note_input/disconnect_client. If any of those five operations internally called focus_pane or focus_tab, the before/after comparison would still pass, because focusing the only pane in the only tab lands back on the same id. This is the test the plan's PROOF section cites for R88/decision-5 point 4 ("focus never changes" across all five operations), and as written it cannot distinguish "focus never moved" from "focus moved and happened to return to the same place." A second tab and a second pane, with focus left on neither before the sequence runs, is needed to make this assertion real.

[Bugs, Nit] crates/kernel/src/tree.rs:1155 note_input's no-op branch when the client's declared size already equals the tab's current size (the "no spurious change reported" case the plan's RISKS section names) has no test exercising it.

[Bugs, Nit] crates/kernel/src/tree.rs:408-413 reapply_latest's branch where the newly-latest client has no declared size for a tab it views (the tab's size holds as it was) is implemented but untested; only the fallback-with-a-declared-size path is covered (latest_falls_back_to_the_next_most_recently_active_remaining_viewer_on_departure).

[Evidence, Nit] crates/kernel/src/tree.rs:2582 closing_a_session_drops_every_clients_attachment_and_activity_for_it uses one client on one pane; the orchestrator's named risk case (two clients viewing different tabs of the same session, closed out from under them) isn't directly exercised by any test, though reading close_session/close_tab/forget_*_everywhere confirms the cleanup is correct there too (retain-based, no dangling references).

[Bugs, Nit] crates/kernel/src/tree.rs:1029-1055 stop_viewing's doc comment says it recomputes the session's latest client only "if that leaves the client viewing nothing else in pane's session," but the code (line 1054) calls reapply_latest unconditionally. Harmless (the equal-size guard in reapply_latest makes the extra calls a no-op), but the comment doesn't describe what the code does.

Everything else checked clean: ClientId/ClientKind mirror SessionId/PaneKind with full wire forms and round-trip tests, malformed ids are rejected via Result not panic; the activity tick is a private kernel counter advanced only by declare_size/note_input (view_pane never ticks, so viewing alone never makes a client latest) and ties cannot occur since each tick is minted once from one incrementing u64; declare_size bypasses the floor unconditionally while split_pane's own floor check is untouched by this diff; SizeChanged.panes matches Layout::leaf_sizes and SizeChanged.viewers lists every attached client, both asserted directly; set_tab_size and Tab::set_size narrowed to pub(crate) with no caller outside the kernel crate; Tab::size() added as a public read-only getter. just check passes at the branch head (b99b850), matching the recorded evidence and PR #29's green CI run on the same SHA.

---
<!-- COMMENTS:END -->
