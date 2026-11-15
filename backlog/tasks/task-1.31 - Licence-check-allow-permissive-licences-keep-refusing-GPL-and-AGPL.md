---
id: TASK-1.31
title: 'Licence check: allow permissive licences, keep refusing GPL and AGPL'
status: In Progress
assignee:
  - '@bruk.w1'
created_date: '2026-11-15T01:32:00Z'
updated_date: '2026-11-15T02:36:00Z'
labels:
  - size-2
milestone: m-0
dependencies: []
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.31-licence-check-allow-permissive-licences
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'c2afd9822006'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
R59 (doc-13) and decision-3. The repository's own licence check refuses the ghostty path today, because a build-only dependency of the crate that fetches ghostty's source is under CDLA-Permissive-2.0. Bruk accepted widening the policy on 2026-11-15.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The licence step of `just check` passes for a dependency graph that includes `ghostty-vt` and its build dependencies
- [ ] #2 A crate under GPL or AGPL is still refused, shown by a recorded run or a test
- [ ] #3 `deny.toml` names the licences it allows; it has no blanket exception and no skipped crate
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Confirm starting state: `deny.toml`'s `[licenses] allow` list has no `CDLA-Permissive-2.0`, no `[licenses.exceptions]`, no `[[licenses.clarify]]`, and no `--exclude`/`private.ignore` anywhere (already read; stays true until step 7). `boundaries.toml` restricts only `steg-kernel`; `crates/server` (steg-server) is unrestricted, so it is the host for every temporary probe dependency below - never steg-kernel.

2. AC#2 probe, GPL refusal, before any deny.toml change (so it proves today's policy, not the widened one): create a temporary local crate at `spikes/licence-probe-gpl/` - `Cargo.toml` with `package.license = "GPL-3.0-only"`, no dependencies, and an empty `src/lib.rs`. Not added to `[workspace] members`; cargo-deny reaches it through the dependency edge in step 3 regardless.

3. Add `licence-probe-gpl = { path = "../../spikes/licence-probe-gpl" }` to `crates/server/Cargo.toml`'s `[dependencies]`. Run `cargo fetch` first (so a resolution failure isn't mistaken for the licence refusal), then run `just deny` (the exact command the licence step of `just check` runs) and record it with `asdlc record-check`. Expect `licenses FAILED`, rejecting `licence-probe-gpl` under `GPL-3.0-only` as not explicitly allowed. This is AC#2's recorded-run evidence, true before and after step 7.

4. Revert step 3: remove the dependency line from `crates/server/Cargo.toml`, restore `Cargo.lock` (`git -C <worktree> checkout -- crates/server/Cargo.toml Cargo.lock`), delete `spikes/licence-probe-gpl/`. Confirm `git -C <worktree> status --porcelain` is clean before continuing.

5. AC#1 probe, ghostty-vt refusal, still before the deny.toml change: add `ghostty-vt = "0.8"` to `crates/server/Cargo.toml`'s `[dependencies]` (plain version, no `[workspace.dependencies]` entry needed for a probe). `cargo fetch`, then `just deny`, recorded with `asdlc record-check`. Expect `licenses FAILED`, naming `webpki-root-certs` under `CDLA-Permissive-2.0`, pulled in by `ureq` as a build dependency of `ghostty-vt-sys` - the same chain decision-3 section 6 recorded.

6. Edit `deny.toml`: add `"CDLA-Permissive-2.0"` to `[licenses] allow`, with a comment giving the reason and its authority - a permissive licence for data, carried only by `webpki-root-certs`, a build-only dependency of `ghostty-vt-sys`'s source downloader that never reaches steg's binary; widened per R59 (doc-13) and decision-3, decided by Bruk 2026-11-15. No `[licenses.exceptions]`, no `[[licenses.clarify]]`, no `--exclude`, no skip of any crate - the allow list is the only mechanism touched, which is what keeps AC#3 true. `MPL-2.0` stays exactly as TASK-1.5 left it (option-ext, via dirs, via gpui) - not touched, not revisited.

7. With `ghostty-vt` still temporarily in `crates/server/Cargo.toml` (from step 5) and the `deny.toml` edit in place, run `just deny` again, recorded with `asdlc record-check`. Expect `licenses ok`. This is AC#1's primary evidence: it is literally the licence step of `just check`, now passing for a graph that includes `ghostty-vt` and its build dependencies.

8. Corroborate AC#1 independently, against the already-vendored spike (its `Cargo.lock` is committed, so this fetches nothing new): `cargo deny --config deny.toml --manifest-path spikes/task-1.1-terminal-library/probe-ghostty/Cargo.toml check licenses`. Record this too. Expect `licenses ok` - the same command decision-3 section 6 ran and recorded as `licenses FAILED` before this task.

9. Revert the step 5 probe: remove the `ghostty-vt` line from `crates/server/Cargo.toml`, restore `Cargo.lock`. `git -C <worktree> status --porcelain` should now show only the `deny.toml` edit.

10. Run the full `just check` on that clean tree and record it. No Rust source changed, so `just test`'s selection is expected to report `none`; `fmt-check`, `lint`, `boundaries`, and `deny` are expected `ok`. This is the finishing check, not a probe - it must be the last one recorded and it must be clean before pushing.

11. Commit only the `deny.toml` change, with the worker trailer. Nothing from steps 2-5, 8-9 (the probe crate, the two temporary dependency lines, any `Cargo.lock` churn they caused) belongs in the diff - confirm with `git -C <worktree> status --porcelain` and `git -C <worktree> diff --stat` against `origin/main` before committing.

12. In the task notes: list the four recorded runs from steps 3, 5, 7 and 8 (GPL refusal, ghostty-vt refusal, ghostty-vt pass, spike corroboration) with their outcomes, and the final `just check` from step 10. Carry forward, as an open question for the engineer and not settled here: whether R59's "permissive" wording was meant to cover MPL-2.0's file-scoped copyleft (TASK-1.5's review flagged this; this task does not touch `MPL-2.0`).

13. Do not edit `backlog/decisions/decision-3`. Its "deny.toml still has to be changed" line goes stale once this lands; decision records are only changed by a dedicated pull request containing nothing else, never by a worker's task branch.

Risks: (a) cargo-deny's SPDX data may not recognise `CDLA-Permissive-2.0` by that exact identifier - step 7's command settles this empirically rather than by assumption. (b) `just deny` runs the real licence step standalone, not full `just check`, so clippy/boundaries never run against the probe dependencies and can't mask or short-circuit the result - full `just check` (lint before boundaries before deny) is reserved for step 10, on the clean tree, where no probe is present. (c) every probe targets `crates/server`, never `crates/kernel`, so none of them can trip `boundaries.toml`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-13 (R59) and decision-3 by @bruk.o: ok. It adds one named licence, no exception and no skipped crate, proves the GPL refusal with a recorded and reverted probe, and leaves decision-3 alone.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-15T02:36:00Z

---

Plan approved by @bruk (c2afd9822006, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
