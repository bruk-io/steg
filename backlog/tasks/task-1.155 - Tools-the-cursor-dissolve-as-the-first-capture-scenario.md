---
id: TASK-1.155
title: 'Tools: the cursor dissolve as the first capture scenario'
status: To Do
assignee: []
created_date: '2026-11-22T19:59:00Z'
updated_date: '2026-11-22T20:02:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.151
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split out of TASK-1.151 by its plan (22 November 2026): the harness and recipe (TASK-1.151) and this first real scenario have different proofs - the harness is proved on a trivial scenario with known pixels, this one by determinism and by a reviewer's judgement against a hand-made reference - and this cannot start until the harness merges. Carries TASK-1.151's acceptance criterion 3. Workers do not launch the app, inject input or capture the screen.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The cursor dissolve of TASK-1.127 is a scenario in the repository, as data: an explicit pane program with fixed content, a throwaway home, and a scripted timeline (idle cycles, a key press, focus loss and regain, the cursor over a character) on the tool's clock
- [ ] #2 just capture-scenario writes its stills of each step and its recording; two runs give byte-identical frames
- [ ] #3 A reviewer judges the recording against the engineer's hand recording of steg PR 95 side by side, not by pixel difference, and the notes say what was compared
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
22 Nov 2026 (@bruk.o), from TASK-1.151's planning: the dissolve's phase is computed from wall-clock time (CursorFadeState.cycle_started_at is a std::time::Instant and paint_cursor_fade reads its elapsed()), while only the wake goes through GPUI's executor timer. A harness that controls the executor's clock therefore cannot bring the dissolve to a chosen phase, so byte-identical frames need a product change in crates/app (take 'now' from a source the harness can set). This task's plan must name that change as its own line, with its own control, or split it out.
<!-- SECTION:NOTES:END -->
