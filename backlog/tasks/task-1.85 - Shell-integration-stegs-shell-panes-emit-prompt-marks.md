---
id: TASK-1.85
title: 'Shell integration: steg''s shell panes emit prompt marks'
status: To Do
assignee: []
created_date: '2026-11-15T21:43:00Z'
updated_date: '2026-11-15T23:10:00Z'
labels:
  - size-3
milestone: m-4
dependencies:
  - TASK-1.79
  - TASK-1.76
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Piece five of the split of TASK-1.79 (2026-11-15). Whether steg sources shell integration automatically is the engineer's decision; the worker's plan on TASK-1.79 lays out what ghostty ships (zsh, bash, fish and elvish scripts; nushell's never emits OSC 133) and that its resources_dir lookup does file I/O. Without marks, the status for typed commands never shows, so this is what makes the feature work for a person who has configured nothing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A shell pane started by steg in zsh or fish loads steg's shell integration automatically, with no file of the person's edited: the shell emits OSC 133 prompt marks with the exit code and reports its working directory when it changes (OSC 7). bash does the same automatically if that can be done cleanly on the bash macOS ships and on a current one; otherwise bash is manual at first, and the plan says which and why
- [ ] #2 The integration is steg's own minimal scripts, one per shell, doing those two things and nothing else, derived from Ghostty's with the origin and licence recorded; they add no wrappers around sudo or ssh, change no cursor or title, and a person can read each in full
- [ ] #3 A command prints the script for a shell so a person can source it by hand, and an environment variable read when the server starts turns the automatic loading off; the off switch is not a setting, because settings do not outlast the server until the configuration file exists (decision-9)
- [ ] #4 A person's own shell configuration still loads exactly as it would without steg, including for someone who already sets ZDOTDIR or its equivalent; a shell steg does not know is started untouched; only shells steg itself starts are affected, not a shell inside ssh or one started by hand inside a pane
- [ ] #5 Where the scripts live and how they are found at run time from the app bundle and from a development build are stated; reading them is done by the server or the steg program, never the kernel
- [ ] #6 Shown with a real zsh and a real fish in a real pty: a command that succeeds and one that fails produce their marks with the right exit code, and changing directory is reported, so that a new pane split from that one starts in the same directory (TASK-1.76's rule) where before it fell back to home
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decisions by Bruk on 2026-11-15, taking the orchestrator's recommendations: automatic and on by default, with a command that prints the script; steg's own minimal scripts (prompt marks and the working-directory report only), derived from Ghostty's with attribution, not Ghostty's as they are; zsh, bash and fish, with bash allowed to be manual at first; the off switch is an environment variable read at server start; only shells steg starts. What the recommendations rested on is the orchestrator's memory of how Ghostty and other terminals load integration (ZDOTDIR for zsh, a search-path directory for fish) and of what Ghostty's scripts do beyond the marks; the plan must verify both from the vendored scripts in third_party/ghostty before relying on them. Two features depend on this task: status for typed commands (TASK-1.83, 1.84), and TASK-1.76's rule that a new pane starts in the directory of the pane it was split from, which needs the shell to report its directory; stock zsh on macOS reports it only to Apple's Terminal.
<!-- SECTION:NOTES:END -->
