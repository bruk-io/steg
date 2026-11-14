---
id: TASK-1
title: A terminal like Rex, built in Rust and GPUI and extendable through Python
status: To Do
assignee: []
created_date: '2026-11-13T22:34:00Z'
updated_date: '2026-11-14T00:59:00Z'
labels: []
dependencies: []
documentation:
  - doc-1
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
