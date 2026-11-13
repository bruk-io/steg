---
id: DRAFT-1
title: A terminal like Rex, built in Rust and GPUI and extendable through Python
status: Draft
assignee: []
created_date: '2026-11-13T22:34:00Z'
updated_date: '2026-11-13T22:41:00Z'
labels: []
dependencies: []
documentation:
  - doc-1
type: intent
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
