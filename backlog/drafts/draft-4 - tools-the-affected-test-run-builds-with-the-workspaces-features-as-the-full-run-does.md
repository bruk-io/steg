---
id: DRAFT-4
title: >-
  tools: the affected-test run builds with the workspace's features, as the full
  run does
status: Draft
assignee: []
created_date: '2026-11-21T20:31:00Z'
labels:
  - size-3
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by TASK-1.145. tools/src/steg_tools/affected.py selects the changed crates and their dependents and builds only those (cargo test -p ...). Cargo unifies features across whatever is in one build, so a selection that leaves out steg-app builds serde_json, and anything else gpui configures, with different features from the whole workspace. PR 87 (TASK-1.120) passed PR CI and two reviews and then failed main's just test-all for exactly this reason. TASK-1.145 removes that one case; the gap is general. Options the planner named: (a) always add -p steg-app to a non-empty selection, cheap but it names gpui's effect by hand; (b) run the selected tests from a workspace-wide build, for example cargo test --workspace --exclude the unselected crates, so features unify as in the full build, which changes the selection's contract and may cost build time. Recommended: (b), measured. check-packages (each crate built alone) keeps its own purpose and is not changed here. Decide whether PR CI should also simply run just test-all, and say why not if not.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A change confined to crates/steg has its tests built with the same resolved features as just test-all, shown by a test or recorded run that fails under today's selection and passes under the new one
- [ ] #2 The selection still runs only the changed crates' tests and their dependents' tests; the notes give the measured cost in time against today's selection
- [ ] #3 The tools' own tests cover the new command line the selection produces
<!-- AC:END -->
