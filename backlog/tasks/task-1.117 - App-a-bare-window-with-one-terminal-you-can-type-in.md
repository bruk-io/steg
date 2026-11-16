---
id: TASK-1.117
title: 'App: a bare window with one terminal you can type in'
status: To Do
assignee: []
created_date: '2026-11-16T12:54:00Z'
labels:
  - size-5
milestone: m-4
dependencies:
  - TASK-1.47
  - TASK-1.114
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A thin slice cut on 2026-11-16 at the engineer's request, to have something visible and usable early and to learn early whether GPUI can draw a terminal within the latency budget (doc-10's open question), before the components, the bar and the pane layout exist. Deliberately bare: one window, one session, one pane; no bar, no tabs, no splits, no palette, no settings, no selection or copy, no scrollback view, no events subscription (it makes its own session and attaches, so it needs TASK-1.47 and TASK-1.114 and not TASK-1.46). doc-9 R19; doc-10 R20, R22, R24; doc-13 R59 (the view is written fresh, nothing taken from GPL code); decision-3 section 5; decision-11 (GPUI directly, pinned at 0.2.2; read its README and the examples text.rs, text_layout.rs, input.rs and painting.rs against the pinned source). The terminal's cell drawing is the app's own and uses the foundations' terminal themes and fonts through the theme; it is not one of the component crate's four levels unless the plan finds it should be (doc-17, out of scope). What this leaves for TASK-1.56: the latency budget as a pass or fail on a 120 Hz display, the 100 MB flood with a second pane, font settings, a size set by another client, and fitting the view into the pane layout. What it leaves for TASK-1.52: the app holding what the server reports through events, a second window, and reconnecting.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Running the app opens one window showing one terminal: it starts a server if none is running, makes a session, attaches to its pane, and draws the pane's rows, colours and cursor from the attachment's snapshot and updates, in the terminal theme the foundations give for the system's light or dark setting
- [ ] #2 Typing reaches the program and its output is drawn: a shell prompt accepts a command and shows the result; keys that are text, Enter, Backspace, Tab, the arrows and Control combinations arrive as the bytes a terminal sends; resizing the window declares the new size and the program is told
- [ ] #3 Closing the window leaves the session running on the server, and running the app again shows the same screen (the server owns the terminal: the app keeps no copy it treats as the truth)
- [ ] #4 The app's own instrumentation records key to paint and key to frame for each keystroke and prints the 50th and 95th percentiles for a run; the numbers from a real run on this machine are in the notes beside doc-9 R19's budget, as a measurement and not yet as a pass or fail
- [ ] #5 Printing a large amount of output does not freeze the window: what is drawn per frame is bounded, and the notes say what was measured and what is left for TASK-1.56
- [ ] #6 The pull request carries an image of the window with a shell in it and a short recording of typing a command; a recipe starts the app; nothing in just check or CI opens a window; what can be tested without a window (key to bytes, a snapshot and updates to the rows to draw, the per-frame bound) has tests with their controls
<!-- AC:END -->
