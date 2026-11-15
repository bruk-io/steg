---
id: doc-10
title: 'Spec: app'
type: specification
created_date: '2026-11-15T01:13:00Z'
updated_date: '2026-11-15T01:24:00Z'
---
- Intent: TASK-1, "A terminal like Rex, built in Rust and GPUI and extendable through Python".
- One of steg's specs. doc-1 is the overview: it has the vocabulary, the release table and the list of specs. Requirement numbers are steg-wide and never reused, so R19 means the same thing in every document.
- Rests on: decision-10 (the design is the artifact), decision-5 (what a window is), decision-7 (status), decision-8 (session colour), decision-9 (settings).
- Status: draft. Reorganised from the single spec on 2026-11-15; every requirement here was moved without rewording, except that references to documents since retired now point at the decision records and at doc-10.

## Scope

The macOS app: its window, how it looks, and its keyboard. Everything about appearance and behaviour that can be drawn is in the design artifact; this spec holds the requirements, says where to look, and says what the artifact cannot: which release each part belongs to, and what is not yet confirmed.

## Requirements

### Desktop app

- R20 (M1). A macOS app built with GPUI. A window shows a session switcher, the current
  session's tabs, and the current tab's panes. More than one window can be open.
- R21 (M1). A command palette goes to any session, tab or pane, and lists every action,
  searchable by title. Its modes, steps and keys follow the design artifact (doc-10).
- R22 (M1). The app is fully usable with no config file.
- R23 (M1). The terminal font, size, line height, ligatures, and fallback fonts are
  configurable.
- R79 (M1). What the window looks like and how its parts behave is the design artifact, as doc-10 describes. Where doc-10 lists a value as not yet confirmed, it is not settled
  until Bruk says so.
- R80 (M1). There is no settings window. A setting is changed from the palette, applies
  at once, and is an override the server remembers; steg never writes to the config file (the design artifact, doc-10).

### Appearance

- R24 (M1). The app follows the system's light and dark setting and can be pinned to
  either.
- R25 (M1). The default dark look is a near-black base with warm gradient accents
  (violet through pink to orange), in the spirit of the Superlogical site and early Arc.
  No Liquid Glass and no dependence on macOS 26 materials.
- R26 (M1). Each session carries one of five colours (violet, pink, orange, blue, green),
  so sessions are told apart at a glance. A new session takes the next colour not in use,
  and the palette changes it. Where the colour shows follows the design artifact (doc-10). A change shows in
  every attached client at once.
- R81 (M2). A session's colour can also be any colour or gradient, set from the CLI and
  Python like any other session property (the design artifact, doc-10).
- R27 (M2). Terminal colour themes are selectable separately for light and dark.

### Keyboard

- R28 (M1). Every action can be bound to a key. Sensible defaults ship with the app.
- R29 (M1). A key that is not bound reaches the terminal unchanged.
- R30 (M2). Bindings support key sequences, such as `g` then `g`.
- R31 (M2). Bindings support named key modes, such as a resize mode in which arrow keys
  resize panes. Escape leaves a mode.

## The design

How the app looks and behaves is the Design artifact "steg app design", https://claude.ai/artifact/Ex9HHnDNY8C8fxV6BR4WVZ (decision-10). This spec does not restate it. Where the artifact and a requirement above disagree, say so on the task rather than choosing one.

| Page of the artifact | Is the source for |
|---|---|
| DesignSystem.Foundations | Every named value (colours, the two ramps, type, sizes, corners), the rules grouped by area, layout primitives and the resizing rule, both terminal themes, the focus ring |
| DesignSystem.Atoms | The icons; the status ring and bar in every state, with their motion and their Reduce Motion forms; buttons, key caps, labels, swatches, the cursor, tooltips |
| DesignSystem.Molecules | The tab, session row, session chip, pane header, palette row, palette input, split handle and empty result, each in every state |
| DesignSystem.Organisms | The bar on the side and on top, the pane, pane layout rules, the palette, status in and out of view, the window; and the states and flows: empty window, a program that has ended, search in a pane, renaming, closing while running, palette help, settings |
| Storyboards | Twelve flows, frame by frame |
| Interactive app | A working mock of the window |

## What belongs to which release

The artifact shows the finished design. Not all of it is M1.

| In the design | Release | Why |
|---|---|---|
| The ring and bar for running with unknown progress, finished, and failed; their motion | M1 | R82: known from the exit status alone |
| The six-section ring and bar for known progress; waiting for a person; the bell as a status | M2 | Needs the status protocol (R49) |
| Five session colours, assigned in turn and changed from the palette | M1 | R26 |
| Any colour or gradient for a session, set from the CLI or Python | M2 | R81 |
| Settings as palette actions, remembered by the server | M1 | R80 |
| A setting's row saying it differs from the config, and opening the config | M2 | Needs the Python config (doc-12) |
| Showing a file in a pane | M2 | doc-12 |
| The failure flakes, the bell's wave, the roll-out | Polish | The last things to build in whichever release their status belongs to, and the first to drop if they cost frames against R19 |

## Values in the design that are not yet confirmed

The orchestrator chose these to make a rule concrete. Bruk has seen the design but has not gone through them one by one.

- The pressed state of a control: one step darker than under the pointer.
- A button drops to icon only below 72px of room.
- The pane header's breakpoints: 360px, 260px and 200px.
- The smallest pane: 200px by 110px.
- The roll-out of a status bar: 250ms.
- The failure flakes: about 40 in a header, over ten seconds, each lying two to six seconds.
- The palette keeping 40px clear in a window narrower than 720px, and growing to at most 60% of the window's height.

## Open questions

- [ ] Do the values listed above as not yet confirmed stand?
- [ ] More tabs than fit the bar along the top, many session chips, reordering tabs, and moving a pane to another tab are not designed. Bruk parked them on 2026-11-14.
- [ ] Whether a session can be ended from the bar along the top, where a session chip has no close button. Today only the palette can.
- [ ] Whether GPUI can draw the six-section ring, a gradient fixed to its track, and the failure flakes within R19's budget. Not established.
- [ ] Workers and reviewers cannot yet read the artifact. The intent to fix that is accepted on the asdlc repository (`intent/workers-read-artifacts.md`); the work is not done.

## Out of scope

- The terminal's own behaviour (doc-9).
- An icon for the app, the menu bar and the Dock.
- Anything shown beside the terminal, and agents (doc-12).
