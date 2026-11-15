---
id: TASK-1
title: A terminal like Rex, built in Rust and GPUI and extendable through Python
status: To Do
assignee: []
created_date: '2026-11-13T22:34:00Z'
updated_date: '2026-11-15T01:28:00Z'
labels: []
dependencies: []
documentation:
  - doc-1
  - doc-2
  - doc-8
  - doc-9
  - doc-10
  - doc-11
  - doc-12
  - doc-13
  - doc-14
  - doc-15
type: intent
asdlc_accepted_from: DRAFT-1
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Requested by @bruk.

## Problem

"The purpose of this new project is to build a similar app to the Rex Terminal." Rex is extended through Lua, "but I would prefer Python". The app "will have a core kernel" and "should be extendable through Python". "Like Rex, it should be composed of a server, a CLI, and the UI."

It is "personal, but assume after a few months I may want to distribute."

## Constraints

- Rust, with GPUI for the UI.
- Extension is in Python, not Lua.
- A core kernel, with a server, a CLI and a UI around it.
- Rex's Program Status Protocol is matched as it is, not reinvented.
- No Liquid Glass. It was asked for at first and then dropped.
- macOS only, for the server and the app. "Linux support can be ignored."
- The product spec is based on Rex's documentation (https://www.superlogical.com/rex/docs) and takes inspiration from the text of https://www.superlogical.com/.
- Colours: Superlogical's palette is reminiscent of the Arc browser when it came out. Noted as a direction, not a hard rule.

## Open questions

- [ ] 1. What does Rex not do for you today, beyond extending in Python? The request names a product to resemble, not a pain to remove.
- [ ] 2. Does Python run inside the server process or in a process of its own beside it?
- [ ] 3. Which terminal emulation library does steg use?
- [ ] 4. Are sessions on other machines, reached over SSH, part of the first usable version or a later one?
- [ ] 5. Should steg match Rex's command and API names, so Rex scripts and agent skills port easily?
- [ ] 6. Should a session survive a server restart or a reboot?
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Programs in a session keep running when the app is closed, and the session comes back as it was when the app is reopened
- [ ] #2 The product is a server, a CLI and a desktop UI, and the CLI can do what the UI can do to sessions
- [ ] #3 Behaviour can be added and changed in Python without changing the Rust code
- [ ] #4 A program that reports its status through Rex's Program Status Protocol shows that status in steg with no change to the program
- [ ] #5 Nothing built for personal use has to be redone to distribute it to other people later
- [ ] #6 An agent working in a pane can show the user a file in another pane from the CLI, such as a Markdown document with proper Markdown rendering
- [ ] #7 Each session can have its own colour, which a running agent can change from the CLI
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decided 2026-11-15 (Bruk): session colour follows the design artifact. In M1 each session has one of the five colours (violet, pink, orange, blue, green): a new session takes the next one not in use, and 'Session: Set colour' in the palette changes it. An arbitrary colour or gradient set from the CLI stays in M2. doc-1's R26 and the release table are to be reworded for this.

Decided 2026-11-15 (Bruk): settings. There is no settings window and no separate settings file. The Python config (doc-1 section 5.9) is the only file; the palette's 'Settings: ...' actions read and change the same values it sets, and 'Settings: Open the config file' opens it. Not yet decided: how a change made in the palette is kept when the config is code a person wrote, and what the palette does in M1, before the Python config exists (5.9 is M2).

Decided 2026-11-15 (Bruk): settings follow the convention of code configs. A setting changed in the palette applies at once and lasts for this run of the server, as an override; steg never writes to the Python config. The palette shows when a value differs from the config, and an action opens the config at the right place with the line to add. In M1, before the config exists, every setting is an override the server remembers. Recorded in doc-7 as D106 to D108.

2026-11-15: Bruk read the day's spec work (doc-7 new; doc-1, doc-2 and doc-5 revised) and said it looks good. Recorded as he said it; the values doc-7 marks (proposed) were not gone through one by one, and doc-1's header still says draft.

Decided 2026-11-15 (Bruk): the terminal emulation library is ghostty's VT engine, which is doc-6's recommendation. doc-6 names what follows from it and none of these is decided yet: the licence policy has to allow CDLA-Permissive-2.0 or exclude build-only dependencies, because the repository's own licence check refuses the ghostty path today; R19's first sentence needs replacing, since neither candidate meets 16 ms once the wait for the display is counted; and the build needs a Zig toolchain and a vendored source tree. The dependency doc-6 names is ghostty-vt 0.8 over ghostty 1.4.0-main, not libghostty-vt 0.2.2, which could not be built.

2026-11-15: Bruk accepted the three consequences of choosing ghostty's VT engine. Written into doc-1: R19 replaced with 12 ms to paint and 20 ms to frame at the 95th percentile; R59 now says the licence check allows permissive licences including CDLA-Permissive-2.0 and refuses GPL and AGPL; R92 added for the library and its build. The licence wording follows the archived TASK-1.28's title; the repository's deny.toml has not been changed and needs a task.

2026-11-15: the specs were reorganised at Bruk's request. doc-1 is now the overview; doc-8 to doc-14 and doc-2 are one spec per area, all in the same shape, with requirement numbers unchanged. doc-7 (the app's design restated as text) is archived: doc-10 points at the design artifact. The decision documents doc-3 to doc-6 are being moved to decision records in pull request 5 and are archived once it merges.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-14T00:59:00Z

---

Accepted by @bruk. Draft ids are reused once promoted, so refer to these by their new ids (asdlc_accepted_from on each keeps the old one):

- DRAFT-1 -> TASK-1
- DRAFT-2 -> TASK-1.1
- DRAFT-3 -> TASK-1.2
- DRAFT-4 -> TASK-1.3
- DRAFT-5 -> TASK-1.4
- DRAFT-6 -> TASK-1.5
- DRAFT-7 -> TASK-1.6
- DRAFT-8 -> TASK-1.7
- DRAFT-9 -> TASK-1.8
- DRAFT-10 -> TASK-1.9
- DRAFT-11 -> TASK-1.10
- DRAFT-12 -> TASK-1.11
- DRAFT-13 -> TASK-1.12
- DRAFT-14 -> TASK-1.13
- DRAFT-15 -> TASK-1.14
- DRAFT-16 -> TASK-1.15
- DRAFT-17 -> TASK-1.16
- DRAFT-18 -> TASK-1.17
- DRAFT-19 -> TASK-1.18
- DRAFT-20 -> TASK-1.19
- DRAFT-21 -> TASK-1.20
- DRAFT-22 -> TASK-1.21
- DRAFT-23 -> TASK-1.22
- DRAFT-24 -> TASK-1.23
- DRAFT-25 -> TASK-1.24
- DRAFT-26 -> TASK-1.25
- DRAFT-27 -> TASK-1.26
- DRAFT-28 -> TASK-1.27

---
<!-- COMMENTS:END -->
