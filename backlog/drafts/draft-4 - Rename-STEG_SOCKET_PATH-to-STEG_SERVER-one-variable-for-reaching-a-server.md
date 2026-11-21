---
id: DRAFT-4
title: 'Rename STEG_SOCKET_PATH to STEG_SERVER: one variable for reaching a server'
status: Draft
assignee: []
created_date: '2026-11-21T20:23:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Decided by the engineer on 2026-11-21: one variable, not two. doc-2 section 4 defines STEG_SERVER as how to reach the server a terminal belongs to; STEG_SOCKET_PATH is in no document and grew up in the code (crates/sdk/src/local_socket.rs and paths.rs, crates/server/src/config.rs and listen.rs, and about fourteen test and fixture files in crates/server and crates/steg) as the way to point a client or a server at one socket. Rename it to STEG_SERVER everywhere, with a deterministic tool rather than by hand, including doc comments, and change no behaviour: the value is still a socket path, read in the same places, and a server reads it to decide where to listen. Nothing reads both names and there is no fallback to the old one (nothing released depends on it). TASK-1.146 puts the same variable in a pane's environment and TASK-1.51 has the CLI follow it; with one name there is no precedence between two. Touches many test files by one line each, so merge origin/main just before finishing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 No occurrence of STEG_SOCKET_PATH remains in the repository's code, tests, fixtures, justfile or tools; every reader and writer uses STEG_SERVER
- [ ] #2 A client and a server given STEG_SERVER use that socket, proven by the existing tests that set it, which pass unchanged apart from the name
- [ ] #3 just check passes; the notes list the files changed and say the rename was done with a deterministic tool
<!-- AC:END -->
