---
id: TASK-1.48
title: 'Server: remember settings changed from the palette'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-15T19:11:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - TASK-1.75
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-18
asdlc_branch: task-1.48-server-remember-settings-changed-from
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '5a1e95a23b12'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-10: R22, R80. decision-9. In the first release there is no config file, so every setting is an override the server remembers.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A setting can be read, changed and listed with its current value through the API
- [ ] #2 A change applies at once and is still there after the app is quit and reopened
- [ ] #3 No config file is read or written
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Blocking question for the engineer

**Where does the override live: in memory for the life of the server process, or on disk?**
I read this against decision-9 and doc-8 rather than assuming a file, and the documents
point to *not* writing one, with one fact that cuts the other way:

- decision-9 #1: "The Python config is the only file. There is no separate settings file."
  #2: "applies at once and lasts for this run of the server."
- doc-12's own open question admits the gap: "R80 has the server remembering settings
  overrides in M1... This spec does not yet say where that state lives."
- doc-8 R2/R3: the server outlives the app, so AC #2's "quit and reopened" is satisfied by
  in-memory state as long as the *server* is still the one that was running — an app quit
  is not a server restart.
- The fact that argues for a file anyway: doc-8 R99 — an on-demand server with no sessions
  and no clients stops itself after 30 seconds. Quit the app with no sessions open, wait 30
  seconds, and an in-memory override is gone when the app (and so the server) comes back.
  Archived doc-7 D108 said "across restarts of the app," which is the R99 case exactly, but
  that line didn't survive the 2026-11-15 reorganisation into decision-9's accepted text.

I'm building the in-memory version below: it is what every accepted sentence says, it is
fully specified, and it needs no file format nobody has authorized. If the engineer decides
R99's stop-and-respawn case must survive, the added work is bounded and the plan says
exactly where it lands (see "If persistence turns out to be needed" under Risks) — it does
not reopen anything else here.

## Which setting, and the doc-2 change — two decisions this plan is making, named so they
can be overruled before building starts

1. **No accepted document names a single concrete setting by its wire name.** R23 (font,
   M1) and R24 (appearance, M1) are the only M1 requirements that read as settings, and R24
   is the cleaner first instance: a closed three-value choice (`system`/`light`/`dark`),
   fully decided, with no sub-fields. I'm seeding the registry with exactly this one,
   wire name `appearance`, so AC #1 has something real to read, change and list. R23's font
   settings are the obvious next entries, left to whichever task first needs them. If
   `appearance` isn't the engineer's first choice, or the wire name should be something
   else, that's a one-line change to `crates/sdk/src/setting.rs`'s registry, not a redesign.
2. **`setting` is a new noun, which doc-2 section 3's own table doesn't have.** Section 8
   says "generated from the API, so this table describes the API as much as the CLI" — so
   this task proposes the doc-2 edit alongside the code, as TASK-1.70 did for
   `server.start --on-demand` ("a small, explicit doc-2 change this plan proposes, for the
   engineer to approve separately"). No new verb: `list`/`show`/`set`/`unset` are already in
   doc-2's closed set. One inversion to flag: doc-2 section 5 says "a property is a flag
   named after the property," but here the property *is* the thing being read or changed, so
   it is the positional (the setting's name) and `--value` is the one flag — the opposite of
   `session set --label`. Proposed doc-2 edit:
   - Section 3's noun table gains `| setting | A value the server remembers, overriding its
     default until changed again | M1 |` (distinct from `config`, M2, which is the file).
   - Section 8 gains:
     ```
     | setting list  | none    | Every setting, its current value and its default |
     | setting show  | setting |                                                    |
     | setting set   | setting | --value <text>                                    |
     | setting unset | setting | Clears the override, reverting to the default     |
     ```

## Design

**sdk** (`crates/sdk`, no I/O, R101/R105 — the one place the list of settings is defined,
so the CLI, a future app and the docs all read it rather than keeping their own copy):

- New `crates/sdk/src/setting.rs`, mirroring `colour.rs`/`pane_kind.rs`'s shape-only style:
  - `pub struct Setting { pub name: &'static str, pub value: ValueKind, pub default:
    &'static str, pub summary: &'static str }` (`Debug, Clone, PartialEq, Eq`; no serde —
    this is Rust-side registry data, never serialised as-is, same reason `Param` literals in
    `methods.rs` aren't).
  - `pub fn settings() -> Vec<Setting>` returning exactly one entry today: `Setting { name:
    "appearance", value: ValueKind::Choice(vec!["system".into(), "light".into(),
    "dark".into()]), default: "system", summary: "Whether the app follows the system's
    light or dark setting, or is pinned to one (R24)" }`.
  - A setting's own `value` is restricted by convention to `Text`/`Choice`/`Integer`/
    `Boolean` — never `Target`/`Path`/`Argv`, which describe something resolved or executed,
    not a value given inline. Documented in the module, and enforced where `ValueKind` is
    matched exhaustively (kernel, below): an `unreachable!()` arm for those three, not a
    silent accept, so a future registry entry that gets this wrong fails loudly in its own
    tests rather than shipping.
- `crates/sdk/src/record.rs` gains `SettingRecord { pub name: String, pub value: String,
  pub default: String }` (`Serialize, Deserialize`, beside `SessionRecord`) — the wire shape
  `setting.list`/`setting.show` return. `value` and `default` are the setting's own wire
  text (see "every value is text" below), not re-typed per setting, so one struct serves
  every entry in the registry without matching on `ValueKind` to pick a JSON type.
- `crates/sdk/src/description/methods.rs`:
  - `setting_fields()`: `name` (Text), `value` (Text, "its current value: the override if
    one is set, else the default"), `default` (Text).
  - `setting_methods()`, inserted in `description()` after `client_methods()` and before
    `api_methods()`:
    - `setting.list`: no params, `Returns::Records { fields: setting_fields() }`, errors
      `[BadRequest]`.
    - `setting.show`: one param, `target_of_kind(ValueKind::Text, true, "The setting to
      show, by its name")` — `Text`, not `Target`: a setting's name isn't a session/tab/pane
      path, the same reason `api.show`'s target is `Text` and not `Target`. `Returns::Record
      { fields: setting_fields() }`, errors `[BadRequest, NoSuchTarget]`.
    - `setting.set`: the same required `target_of_kind(Text, true, ...)`, plus `flag("value",
      ValueKind::Text, true, "The setting's new value")`. `Returns::Nothing`, errors
      `[BadRequest, NoSuchTarget]`.
    - `setting.unset`: the same required target, no value. `Returns::Nothing`, errors
      `[BadRequest, NoSuchTarget]`.
  - **Every setting's value crosses the API as `Text`, never a typed `ValueKind` picked per
    setting.** There is no way to give one flag ( `value` on `setting.set`) a `ValueKind`
    that depends on which setting the request names — the description's shape is per-param,
    not per-value-of-another-param. This follows the precedent already in this file:
    `layout`, `status` and `shown_status` are `ValueKind::Text` placeholders for a shape
    that isn't a flat wire type yet. A boolean or integer setting (R23's font settings) is
    sent as the word `"true"`/`"false"` or a digit string; the registry's own `ValueKind`
    says how to parse it, checked by hand in the kernel (below), not by `validate`'s generic
    per-param check.
  - Tests updated mechanically for the new methods, no new assertions needed beyond the
    existing ones running over a longer list: `the_method_set_is_exactly_doc_2_section_8s_38_pairs`
    becomes `..._42_pairs`, with `doc_2_section_8()` gaining the four `("setting", verb)`
    pairs and `doc_2_section_8_targets()` gaining `("setting.list", None)`,
    `("setting.show"/"set"/"unset", Some((true, ValueKind::Text)))`.

**kernel** (`crates/kernel`, no I/O, `#![forbid(unsafe_code)]` — the rule: a name and a raw
value are either a real setting with a value its own kind accepts, or they're refused):

- New `crates/kernel/src/setting.rs`:
  - `pub struct Settings { overrides: std::collections::HashMap<String, String> }`
    (`Default`, built with `Settings::default()`/`Settings::new()`) — pure data: an override
    is its own wire text, same reason `SettingRecord`'s fields are text, so no conversion
    happens going in or coming out.
  - `pub fn value_of(&self, name: &str) -> Result<String, Error>` — the override if one is
    set, else the registry's own default; `Error::UnknownSetting` if `name` isn't in
    `steg_sdk::settings()`.
  - `pub fn set(&mut self, name: &str, value: &str) -> Result<(), Error>` — looks the name
    up, checks `value` against its declared `ValueKind` (a private `accepts(kind, value) ->
    bool`, matched exhaustively over `ValueKind`: `Text` always true, `Choice(words)` checks
    membership, `Integer` parses, `Boolean` is exactly `"true"`/`"false"`, `Target`/`Path`/
    `Argv` is `unreachable!("a setting's declared kind is never target, path or argv")`),
    then inserts the override. `Error::UnknownSetting` for a bad name, `Error::
    InvalidSettingValue { name, value }` for a value that fails the check.
  - `pub fn unset(&mut self, name: &str) -> Result<(), Error>` — same name check (still
    refuses an unknown name, same as every other noun's `unset`), then removes any override;
    unsetting an already-default setting is not an error, matching `pane.unset`'s "a
    property left out is unchanged."
  - Unit tests (no doubles): a fresh `Settings` reports the registry's own default for
    `appearance`; `set` changes what `value_of` returns at once; `set` with an unknown name
    is `UnknownSetting`, with `"neon"` (outside the closed set) is `InvalidSettingValue`;
    `unset` after `set` reverts to the default; `unset` of an unknown name is still
    `UnknownSetting`; `unset` of a never-set setting is `Ok(())` and changes nothing.
- `crates/kernel/src/error.rs`: two variants, `UnknownSetting(String)` and
  `InvalidSettingValue { name: String, value: String }`, each a `Display` arm (`"no setting
  named {name:?}"`, `"{value:?} is not a valid value for {name:?}"`) and a `source() ->
  None` arm, added to the existing no-wildcard match (so a thirteenth-and-fourteenth variant
  without an arm fails the build, same reason the match has no `_` today).
- `crates/kernel/src/lib.rs`: `mod setting;` and `pub use setting::Settings;`, alphabetically
  between `session` and `tab`.

**server** (`crates/server`, where I/O is allowed, and where the override actually lives for
the life of the running process — the remembering):

- New `crates/server/src/dispatch/setting.rs`, the `setting` noun's dispatch arms, mirroring
  `session.rs`'s shape:
  - `pub(super) const HANDLED: &[(&str, &str)] = &[("setting", "list"), ("setting", "show"),
    ("setting", "set"), ("setting", "unset")];`
  - `pub(super) fn dispatch(settings: &mut Settings, request: &Request, verb: &str) ->
    Result<Outcome, ApiError>` matching `list`/`show`/`set`/`unset`, each reading the
    `target` param as the setting's name via `super::param_str` (erroring `bad_request` if
    absent — `target` is `required: true` on every verb but `list`, so `validate` already
    refuses its absence before this runs; the `bad_request` here is defence for `list`'s
    call path, which never asks for one).
  - `list`: `steg_sdk::settings()` mapped through `settings.value_of` into
    `Outcome::SettingRecords`.
  - `show`: one name, mapped the same way into `Outcome::SettingRecord`, kernel errors
    through `map_kernel_error`.
  - `set`: reads `value` via `param_str` (`bad_request("needs value")` if absent, though
    `validate` already refuses this as required), calls `settings.set`, maps its error.
  - `unset`: calls `settings.unset`, maps its error.
- `crates/server/src/dispatch/outcome.rs`: `Outcome::SettingRecord(steg_sdk::SettingRecord)`
  and `Outcome::SettingRecords(Vec<steg_sdk::SettingRecord>)`, each an `into_value` arm via
  `serde_json::to_value` (same pattern as `SessionRecord`/`SessionRecords`), plus the two
  equivalent tests already there for those.
- `crates/server/src/dispatch/kernel_error.rs`: two arms, `Error::UnknownSetting(_) =>
  ApiError::new(ErrorCode::NoSuchTarget, ...)` (an unknown setting name is "the target
  matched nothing," the same reading `api.rs::show` already gives an unknown method name —
  status 3, not 2) and `Error::InvalidSettingValue { .. } => ApiError::new(ErrorCode::
  BadRequest, ...)` (`BadRequest`'s own declared summary is literally "...or a value outside
  a closed set" — exactly this case). No new `ErrorCode`: both already exist and already
  carry the right exit status (3 and 2). Two new unit tests, mirroring the file's existing
  ones.
- `crates/server/src/dispatch/mod.rs`:
  - `mod setting;` added to the module list.
  - `dispatch()`'s signature gains one parameter: `pub fn dispatch(tree: &mut Tree, settings:
    &mut Settings, description: &Description, context: &Context, request: &Request) ->
    Response`, with a new match arm `(noun, verb) if setting::HANDLED.contains(&(noun,
    verb)) => setting::dispatch(settings, request, verb)`.
  - `handled()`'s test-only list gains `setting::HANDLED`.
  - `api::call`'s one recursive call site (`super::dispatch(...)`) gains `settings` — see
    `api.rs` below.
  - A `ServerState { tree, settings }` struct was the alternative considered and rejected:
    there is no production caller of `dispatch()` yet (TASK-1.44/1.45 are unbuilt), only this
    crate's own tests, so a second explicit parameter costs nothing today and keeps every
    existing caller's intent visible at the call site, the same reason `Context` is already
    its own explicit argument rather than folded into `Tree`.
- `crates/server/src/dispatch/api.rs`: `call()`'s signature gains `settings: &mut Settings`,
  threaded into its one `super::dispatch(tree, settings, description, context, &inner)` call.

**Nothing changes in `boundaries.toml` or `deny.toml`.** No new dependency, in any crate:
`Settings`'s override map is `std::collections::HashMap<String, String>`, already available
without a `use` beyond `std`; `steg-server`'s existing `serde_json` allowance already covers
`SettingRecord`'s `into_value`.

## Order of work

1. `crates/sdk/src/setting.rs` (`Setting`, `settings()`, its own unit tests) and
   `crates/sdk/src/lib.rs`'s `mod`/`pub use` lines. Compiles and tests alone.
2. `crates/sdk/src/record.rs`'s `SettingRecord` and its tests (serde round-trip; a label-less-
   session-style "every field present" test; cross-check against the fields the methods
   below declare, once step 3 exists — write this test last in this step, filled in after 3).
3. `crates/sdk/src/description/methods.rs`'s `setting_fields()`/`setting_methods()`, wired
   into `description()`, and the four existing tests' updates (method count, the two
   doc-2-mirroring lists). Run `just test` for `steg-sdk` alone; this is the point every sdk
   test should pass with no kernel or server change yet.
4. `crates/kernel/src/error.rs`'s two variants and arms; `crates/kernel/src/setting.rs`
   (`Settings`, its methods, its unit tests); `crates/kernel/src/lib.rs`'s `mod`/`pub use`.
   `just test` for `steg-kernel` alone.
5. `crates/server/src/dispatch/kernel_error.rs`'s two arms and tests.
6. `crates/server/src/dispatch/setting.rs`, `outcome.rs`'s two variants, `mod.rs`'s signature
   and wiring change, `api.rs`'s threaded parameter.
7. `crates/server/tests/dispatch.rs`: extend `fresh()` to also return a fresh `Settings`
   (or add a one-line sibling helper used alongside it, whichever reads more plainly once
   written), add `&mut settings` to every existing `dispatch(...)` call (the compiler finds
   every site that needs it — this is mechanical, not a logic change to any existing test),
   then add the new `setting_*` tests (list/show/set/unset, each error case, the "set then
   read again proves at-once" test, and the "fresh `Tree`/`Context` over the same `Settings`"
   test standing in for "still there after the app is reopened" — see Proof).
8. `just check` for the whole workspace.

## Risks

- **The persistence question above is the real risk.** If the engineer says R99's
  stop-and-respawn case must survive, the bounded addition is: `crates/sdk/src/paths.rs`
  gains a second pure function (`settings_path`, under the *state* directory per doc-13 R60,
  not the config directory — the reasoning that answers "what distinguishes this from a
  config file" if it's asked again later: `$XDG_STATE_HOME/steg/settings.json`, falling back
  to `$HOME/.local/state/steg/settings.json`, mirroring `socket_path`'s own fallback
  ladder); `steg_kernel::Settings` gains `to_overrides()`/`from_overrides()` (plain
  `HashMap<String, String>` in and out, still no I/O in kernel); `steg-server` gains a thin
  `settings_store.rs` doing the actual `std::fs` read/write — read-on-start (missing file:
  empty overrides, not an error; unreadable or corrupt JSON: log and start with no overrides
  rather than refusing to start, "never lose settings silently, never refuse to start"; a
  newer, unknown top-level shape: same treatment, since nothing versions this file yet),
  write-after-every-`set`/`unset` via a temp-file-then-`rename` in the same directory
  (atomic on one filesystem, `std` only, no new dependency). Proof gains real files in a
  `tempfile`-free temp dir (`std::env::temp_dir()` plus a unique suffix, no new dependency)
  asserting a value written by one `Settings`/store pair is read back by a second one
  constructed fresh — the actual "survives a restart" proof this version is missing. This is
  **not built now**; it is scoped here so saying yes doesn't reopen the rest of the design.
- **The `dispatch()`/`api::call()` signature change touches every call site in
  `crates/server/tests/dispatch.rs`** (`fresh()` and ~46 call sites). It is mechanical — the
  compiler refuses to build with a site missed, so nothing is silently skipped — and changes
  no existing test's assertions, only its arity. Still worth naming as the one place this
  plan's diff is wide rather than deep.
- **`appearance` as the seed setting is this task's own choice**, tied to R24 but not a
  wire name any accepted document fixes. Cheap to change (one registry entry) if the
  engineer wants a different first instance or name.
- **The doc-2 edit is a second thing to approve**, same as TASK-1.70's `--on-demand`
  precedent; it's written out above so it can be approved or rejected without touching the
  code plan.
- **Every setting's value travels as `Text` on the wire** (see Design), which pushes
  per-setting type-checking into the kernel by hand rather than the generic `validate`.
  Accepted now because it costs nothing with one setting and avoids widening `ValueKind`
  (a change to every consumer, per that enum's own module doc) for a shape only one task
  needs so far.

## Proof

- **AC #1** ("read, changed, listed with its current value"): the server functional tests
  in step 7 — `setting list` returns the one registered setting with its default value when
  nothing has been set; `setting show appearance` returns the same record; `setting set
  appearance --value light` then `setting show appearance` (a second, independent `dispatch`
  call over the same `Settings`) returns `light`; `setting unset appearance` then `show`
  returns `system` again. Each paired with its refusal: `setting show`/`set`/`unset` with an
  unknown name is `ErrorCode::NoSuchTarget`; `setting set appearance --value neon` is
  `ErrorCode::BadRequest`. Each refusal traced down to its own kernel unit test (step 4) and
  its own `kernel_error.rs` mapping test (step 5), not asserted only at the top.
- **AC #2, "applies at once":** the same `set`-then-`show` test above, both calls against
  the one `Settings` value created once per test — there is no cache and no event to wait
  on, so the next read seeing the new value *is* "at once" at this layer. Pushing the change
  to a connected client (an event) is out of scope: doc-8 R38's fourteen M1 events are
  unchanged by this task, and a `pane.*`/`session.*`-style `setting.changed` event is
  TASK-1.46/TASK-1.59's to add once there is a palette to receive it.
- **AC #2, "still there after the app is quit and reopened":** proved only as far as this
  layer can reach with no socket or process lifecycle yet (TASK-1.44/1.45 unbuilt) — a test
  builds a fresh `Tree` and `Context` (standing in for a new client/window connecting) while
  reusing the *same* `Settings` value (standing in for "the server, which outlives the app,"
  doc-8 R2/R3), and asserts the override is still there. This is a limitation, named as one,
  not dressed up as the real proof: it cannot show the server itself surviving a restart,
  which is exactly the persistence question above. "Real files in a temp dir" from the
  brief is moot for this version — there is no file — and that is stated here rather than
  silently dropped.
- **AC #3** ("no config file is read or written"): true by construction — nothing this task
  adds touches `std::fs` anywhere, in any crate. For the kernel half, `crates/kernel/
  clippy.toml`'s `disallowed_types`/`disallowed_methods` (R102/R110's mechanism) make this a
  checked property, not an assertion to trust; `just check-boundaries`/`just check-packages`
  cover it the same way every other kernel module is already covered. For the server half,
  there is no lint forcing it (the server is allowed I/O generally), so this rests on the
  diff itself containing no file code, true today and reopened only if the persistence
  question above is answered yes.
- `just check` at the end of step 8 is the whole-workspace proof nothing else regressed:
  format, lint, the crate-boundary and licence checks (unaffected, see Design's last
  paragraph), the architecture model (unaffected — no new package, no new cross-package
  edge), and the justfile's own recipe-naming check.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-10 (R22, R80), decision-9, doc-2 and doc-16 by @bruk.o: ok to build as written if the engineer confirms three things the plan itself raises. (1) Overrides are kept in memory for the life of the server, which is what decision-9 point 2 says ('lasts for this run of the server') and point 1 requires ('there is no separate settings file'); the consequence is that an override is lost when the server stops, including an on-demand server's 30-second idle stop (doc-8 R99) and a reboot, so acceptance criterion 2 holds only while the server keeps running. (2) The one setting seeded is 'appearance' (system, light, dark; R24). (3) doc-2 gains a 'setting' noun with list, show, set and unset; the orchestrator makes that edit once approved. Layering matches doc-16: the list of settings in the sdk, the rule in the kernel with no I/O, the remembering in the server. No new dependency.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T19:11:00Z

---

Plan approved by @bruk (5a1e95a23b12, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
