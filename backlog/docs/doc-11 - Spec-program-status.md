---
id: doc-11
title: 'Spec: program status'
type: specification
created_date: '2026-11-15T01:13:00Z'
updated_date: '2026-11-15T01:15:00Z'
---
- Intent: TASK-1, "A terminal like Rex, built in Rust and GPUI and extendable through Python".
- One of steg's specs. doc-1 is the overview: it has the vocabulary, the release table and the list of specs. Requirement numbers are steg-wide and never reused, so R19 means the same thing in every document.
- Rests on: decision-7 (status across releases, one ring per tab, and when a status ends). The decision records are in pull request 5 until it merges.
- Status: draft. Reorganised from the single spec on 2026-11-15; every requirement here was moved without rewording, except that references to documents since retired now point at the decision records and at doc-10.

## Scope

What steg knows about the program in a pane and how long it keeps knowing it: running, finished, failed, and in M2 everything the Program Status Protocol reports. How a status is drawn is the app's (doc-10); how it travels is the wire protocol's (doc-14).

## Requirements

- R49 (M2). Steg implements the Program Status Protocol (OSC 7501) as Superlogical
  publishes it: grammar, keys, size limits, whole-record replacement, the `id` hierarchy,
  record lifetime, the feature detection reply, and the security rules.
- R82 (M1). Without the protocol, steg still shows what it can know by itself about a
  pane's foreground program: running while it runs, and finished or failed from its exit
  status. Progress, waiting for a person, and the bell as a status need R49.
- R50 (M1 for running, finished and failed; M2 for the rest). A pane's status is shown as
  a ring on its tab while the pane is out of view, and as a bar in the pane's own header
  while it is in view. A tab shows one ring, for the most pressing of its panes. The
  selected tab shows none, and a session never shows one. Shapes, colours, order and
  motion follow the design artifact (doc-10).
- R83 (M1). Each status ends by its own rule: waiting and failed never on a timer;
  finished after 10 minutes and the bell after 30 seconds, or sooner on a key press or
  click in that pane. Opening a tab clears nothing (the design artifact, doc-10).
- R84 (M2). A tab holds the current status of each of its panes as a set: a new report
  replaces only that pane's last one, and the tab shows the most pressing, then the next
  when one ends. A pane's status can be promoted to its tab from the CLI; a pane that is
  waiting or has failed still takes the ring. The command and its protocol form are
  decided in TASK-1.30.
- R51 (M3). A waiting or failed status can raise a macOS notification naming the session
  and pane, only while steg is in the background and not more than once per pane per
  state. Finished does not notify. Each is switchable on its own (the design artifact, doc-10).
- R52 (M2). Status changes are events, and the CLI can list a pane's status records.

## Open questions

- [ ] Whether running needs a lifetime, for a program that stops reporting without exiting.
- [ ] The CLI command for promoting a pane's status to its tab, and whether promotion is its own message in the status protocol or a field on the existing report.
- [ ] How the app tells the server a person has dealt with a pane by clicking in it. A key press is input the server already sees; a click is not (doc-14).

## Out of scope

- Output arriving in a pane that is out of view. It is not a status and shows nothing.
