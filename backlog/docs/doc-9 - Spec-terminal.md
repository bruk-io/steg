---
id: doc-9
title: 'Spec: terminal'
type: specification
created_date: '2026-11-15T01:13:00Z'
updated_date: '2026-11-15T14:15:00Z'
---
- Intent: TASK-1, "A terminal like Rex, built in Rust and GPUI and extendable through Python".
- One of steg's specs. doc-1 is the overview: it has the vocabulary, the release table and the list of specs. Requirement numbers are steg-wide and never reused, so R19 means the same thing in every document.
- Rests on: decision-3 (ghostty's VT engine), decision-2 (the wire protocol, for how a screen reaches a client).
- Status: draft. Reorganised from the single spec on 2026-11-15; every requirement here was moved without rewording, except that references to documents since retired now point at the decision records and at doc-10.

## Scope

What a pane's terminal does: emulation, scrollback, speed, and the features full-screen programs need. It does not cover how the app draws a terminal or selects text in it (doc-10).

## Requirements

- R13 (M1). A pane runs the user's login shell, or a given command, with a given working
  directory and environment.
- R14 (M1). Full-screen programs render correctly: vim, htop, tmux, and Claude Code are
  the acceptance set. Supported features include truecolour, mouse reporting, bracketed
  paste, the alternate screen, the kitty keyboard protocol, OSC 8 links, OSC 52
  clipboard, OSC 7 working directory, and OSC 133 prompt marks.
- R15 (M1). The server keeps scrollback per pane up to a configurable limit, 10,000 lines
  by default.
- R16 (M1). The server knows each terminal's working directory, title, and its program's
  exit code, and reports changes as events.
- R116 (M1). A cell says whether it is part of a link, which is enough to draw one. A link's address is not carried with the screen: it is asked for, by cell, when a person acts on the link. Two different links side by side are not told apart until one is asked about.
- R17 (M1). The app supports text selection, copy, paste, find in scrollback, and
  clickable links.
- R18 (M2). The app can jump between shell prompts in scrollback.
- R19 (M1). Typing in a local pane is drawn into the next frame the app builds within
  12 ms at the 95th percentile, measured from the key event to the end of the paint.
  Including the wait for the display's refresh, the time from key to frame is within 20 ms
  at the 95th percentile on a 120 Hz display. Both are measured with the app's own
  instrumentation. Printing a 100 MB file does not freeze the app or delay input to other
  panes; to hold that, the app caps what it feeds the emulator in one frame and carries the
  rest over. These replace the first proposal of 16 ms, which did not say where the clock
  stops; decision-3 section 5 has the measurements.
- R92 (M1). Terminal emulation is ghostty's VT engine, through the `ghostty-vt` bindings
  (decision-3). The build needs a Zig toolchain and uses a vendored copy of ghostty's source, so
  that no build reaches the network.

## Open questions

- [ ] The server's tick, which sets how soon a change reaches a client's grid. 8 ms leaves too little of R19's budget; 1 ms costs resynchronisations under a flood. It wants a real window in front of it (decision-2).
- [ ] The cell model a snapshot carries is being built by TASK-1.39: wide characters and their spacers, underline style and colour, and the cursor's shape. Link identity is settled by R116: the engine gives presence on the path used for drawing and the address only by asking about one cell (Bruk, 2026-11-15).

## Out of scope

- Pane kinds other than a terminal. R12 keeps the door open; v1 ships only the terminal.
