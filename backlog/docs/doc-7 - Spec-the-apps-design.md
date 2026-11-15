---
id: doc-7
title: 'Spec: the app''s design'
type: specification
created_date: '2026-11-15T00:22:00Z'
updated_date: '2026-11-15T00:30:00Z'
---
Intent: TASK-1. Delivers the design section that TASK-1.27 asks for (its criterion 3), as a document of its own because it is too long for a section of doc-1.

Source: the Design artifact "steg app design", https://claude.ai/artifact/Ex9HHnDNY8C8fxV6BR4WVZ (private to Bruk). The artifact is the picture and the working mock; this document is the text an app task builds from and cites by number. Where the two disagree, say so on the task rather than choosing one.

Status of this document: written by @bruk.o from the design Bruk worked through on 2026-11-14. Bruk said the design is close enough to use as a spec; he has not yet approved this text. Every value marked (proposed) was chosen by the orchestrator to make a rule concrete and needs Bruk's yes or a different number.

Requirements are numbered D1 onward so they do not collide with doc-1's R numbers. "The bar" is the one strip that holds sessions and tabs. "The header" is the 30px strip at the top of a pane.

## 1. Named values

### 1.1 Neutral colours

| Name | Dark | Light | Used for |
|---|---|---|---|
| surface.ground | #0e0d10 | #f1efec | The window, behind everything |
| surface.pane | #141318 | #f8f6f4 | Panes that are not active; the tab track |
| surface.raised | #1c1b21 | #ffffff | The active pane, the selected tab, buttons |
| surface.overlay | #201f26 | #ffffff | Command palette |
| fill.hover | #19181d | #f7f5f3 | A tab or row under the pointer |
| fill.quiet | #28272e | #dfdcd7 | Key caps, a button under the pointer, a pressed tab or row |
| outline | #3d3b45 | #bfbbb5 | Outlines with Increase Contrast; a pressed button |
| status.track | #3d3b45 | #cfcbc5 | The grey track of a status ring or bar |
| text | #ecebee | #1b1a1f | Primary text |
| text.secondary | #b0aeb9 | #4f4d57 | Inactive tabs and captions |
| text.muted | #8f8d99 | #63616b | Labels, hints, a part that cannot be used |

### 1.2 The six colours, and blue

| Name | Dark | Light | Used for |
|---|---|---|---|
| ramp.1 violet | #a98cff | #7b5cf0 | Accent, focus ring, default session colour |
| ramp.2 pink | #f277b5 | #d6408c | Session colour; terminal magenta |
| ramp.3 red | #ff8a80 | #bf2c27 | Failed; terminal red |
| ramp.4 orange | #ff9950 | #e0701a | Session colour |
| ramp.5 amber | #ffc25e | #8f5300 | Waiting, and the bell; terminal yellow |
| ramp.6 green | #6fd3a0 | #14734a | Finished; session colour; terminal green |
| session.blue | #6fb1ff | #2463c9 | Session colour; terminal blue. The one colour not on the ramp |

- The ramp: a left-to-right linear gradient of steps 1, 2 and 4 at 0%, 50% and 100%.
- The extended ramp: a left-to-right linear gradient of steps 1 to 6, evenly spaced.

### 1.3 Type

| Name | Value |
|---|---|
| font.ui | The system font (San Francisco) |
| font.terminal | JetBrains Mono, then SF Mono, then Menlo |
| terminal | 13px on a 20px line, weight 400 |
| ui.body | 13px, weight 400; 600 when selected |
| ui.small | 12px: captions, key caps, hints |
| ui.label | 11px, weight 600, upper case, letter spacing 0.06em |
| palette.input | 15px |

### 1.4 Sizes

| Name | Value |
|---|---|
| space | Steps of 2, 4, 6, 8, 10, 12, 16px |
| radius | 14px on panes and the palette; 10px on the tab track and session chips; 8px on anything inside one of those; fully round on swatches of colour and on bars |
| bar.side | 232px wide; rows 30px high; tabs indented 28px |
| bar.top | 40px high; tabs 30px high in a track with a 2px inset and a 2px gap |
| tab.width | 120px usual, 240px at most, 72px at least |
| slot | 22px square: the fixed place at the right end of a tab or session row |
| pane.gap | 6px between panes and to the window edge; grabbed across 12px |
| pane.header | 30px |
| palette | 640px wide, rows 40px, 120px from the top of the window |
| status.ring | 16px across, a 2px stroke on a 6px radius |
| status.bar | 120px wide, 4px thick, in a 14px-high box; a 20px gap at its centre when it carries a mark |
| scrim | Black at 60% in dark; the text colour at 28% in light |

### 1.5 Terminal themes

Two themes, "steg dark" and "steg light", each with sixteen ANSI colours. They are kept apart from the values above so a theme can be swapped without touching the window.

| | black | red | green | yellow | blue | magenta | cyan | white |
|---|---|---|---|---|---|---|---|---|
| dark | #2c2a33 | #ff8a80 | #6fd3a0 | #ffc25e | #6fb1ff | #f277b5 | #6fd0e0 | #b0aeb9 |
| dark, bright | #6f6d7a | #ffb0a8 | #9be5bf | #ffd78f | #b6ceff | #f8a5cf | #a3e3ee | #ecebee |
| light | #1b1a1f | #bf2c27 | #14734a | #8f5300 | #2463c9 | #d6408c | #0f6f80 | #63616b |
| light, bright | #4f4d57 | #d9443e | #1f8f5e | #a86a10 | #3d6fe0 | #e0559c | #1b8799 | #8b8993 |

Dark: foreground #ecebee; background is the pane's own surface (#1c1b21 active, #141318 not); selection #38354d. Light: foreground #1b1a1f; background #ffffff active, #f8f6f4 not; selection #e2dcf7. In both, the cursor is the session colour.

## 2. Requirements

### 2.1 Surfaces and colour

- D1. Nothing is separated by a line. Surfaces are told apart by their fill and by the gap between them.
- D2. Surfaces step up in three fills: the window (surface.ground), a pane (surface.pane), the active pane (surface.raised). Whatever is selected takes the top step.
- D3. Corners use the three radius values in 1.4 and no others; swatches of colour and bars are fully round.
- D4. Nothing is underlined. Selection is a filled shape; a match in the palette is bold.
- D5. Every colour is a solid fill or a linear gradient. No blur, no materials, no stacked shadows.
- D6. Every accent, session and status colour is one of the six ramp steps or session.blue. No other hue is introduced.
- D7. The ramp is decoration: full strength in the app mark, and across the active pane's header at 50% in dark and 40% in light. It is never behind text at full strength.
- D8. The extended ramp is progress, and appears only in a running status ring and a running status bar.
- D9. A session colour is a fill and never colours text. It appears as the tint on the open session in the bar (the colour at 18%), as the cursor, and as the window ground when the session wash is on.
- D10. A session colour set from the CLI is used as given in dark, and darkened until it reaches 3:1 against the light ground in light.
- D11. The app follows the macOS light or dark setting, and both themes are complete: every requirement here holds in both.

### 2.2 Controls, in every state

- D12. Every control that can be used has five states: at rest, under the pointer, pressed, keyboard focus, and cannot be used.
- D13. Under the pointer is a change of fill one step up: fill.hover on a tab, session row or icon button; surface.raised on a session chip; fill.quiet on a button.
- D14. Pressed is one step further than under the pointer: fill.quiet on a tab, row, chip or icon button; outline on a button. (proposed)
- D15. Keyboard focus is a 2px ring in ramp.1 of the session's colour set, 2px clear of the control, shown only when focus arrives from the keyboard. Its corner is the control's corner plus 4px.
- D16. A control that cannot be used keeps its place and takes text.muted. It is never faded and never hidden.
- D17. A button has an icon and a label. Its label names the thing and the act ("New session").
- D18. An icon button is an icon alone, with no fill at rest: 30px in the bar, 24px in a pane header and inside a tab. Its name is its tooltip and its VoiceOver label.
- D19. A tooltip appears beside its control after the pointer has rested for half a second, or at once on keyboard focus, and never covers the control. It is the only place a status is written as a word.

### 2.3 Resizing

- D20. A component that can resize declares a compact and a regular form, and a full form where there is more to show. It shrinks by dropping whole parts in a stated order. It never squeezes, fades or overlaps them.
- D21. An icon and a status are never dropped and never cut off. Text is the first thing to shorten.
- D22. What is dropped stays reachable: a label becomes the tooltip; a status stays on the tab's ring.
- D23. Button: icon only below 72px of room (proposed); icon and label by default; icon, label and shortcut in the palette and in menus. Dropped first: the shortcut, then the label.
- D24. The compact button keeps the button's fill, height and corner. It is not the icon button.
- D25. Tab: 72px at least, 120px by default, up to 240px when its title needs it. All tabs in one track are the same width and shrink together. Dropped first: the end of the title. Never the slot at its right end.
- D26. Session chip: the name by default; two letters when the tabs would otherwise go below 120px. All chips change together, before any tab shrinks.
- D27. Pane header, by the pane's width (all four proposed): 360px or wider shows title, directory, status bar and four buttons; under 360px drops the directory; under 260px drops the status bar; under 200px drops split and fill, leaving title and close.

### 2.4 The bar: sessions and tabs

- D28. Sessions and tabs live in one bar, on the side or along the top, never both. A button in the bar moves it; the position is also a setting and a palette action.
- D29. On the side, the bar lists every session, with the open session's tabs under it, indented.
- D30. Along the top, the open session is one tinted track holding its name and its tabs, and every other session is a chip beside it.
- D31. Only one session is open in a window at a time. Opening another closes the first in that window; it keeps running.
- D32. A session shows no status of its own, open or closed. Nothing in the bar counts or marks what is waiting in a closed session.
- D33. A new session is a + in the bar: in its header beside the "Sessions" label on the side, and after the session chips along the top. It is always shown.
- D34. A new tab is a + on the open session: on its row on the side, shown at rest; at the end of its track along the top. It is not offered on a closed session.
- D35. On the side, a session row ends in two fixed 22px slots, reserved on every session row so names never shift: + for a new tab on the open session only, then x to end the session, shown only under the pointer or with keyboard focus.
- D36. Ending a session always asks first (D83), whether or not anything is running in it.
- D37. The right end of every tab is one fixed 22px slot. At rest it shows the tab's status ring, or nothing. Under the pointer or with keyboard focus it shows the close button in the same place. Nothing moves sideways.
- D38. The selected tab shows no status ring: its panes are in view and their headers carry the status. It has no permanent close button.
- D39. A tab is called what the person named it; otherwise the title of the program in its active pane.
- D40. Closing a session's last tab ends the session, and so asks as D36 does.

### 2.5 Panes

- D41. The active pane is marked three ways: surface.raised, the ramp across its header (D7), and a filled cursor in the session colour. Its header also carries its controls: split right, split down, fill, close.
- D42. Other panes keep their text at full strength, sit on surface.pane, show a hollow 1px cursor, and show no controls.
- D43. A pane's header shows, left to right: the program's title, the directory or the program's own note, the status bar if it has a status, then the controls if it is active.
- D44. The gap between two panes is the handle for resizing them: drawn 6px wide, grabbed across 12px. Under the pointer a 4px by 28px pill appears in it in the outline colour; while dragging the pill takes the session colour. A double click makes the two sides equal.

### 2.6 Pane layout

All of 2.6 is a proposal: none of it was in doc-1.

- D45. A split comes from the active pane. It gives up half its space; the new pane takes that half, becomes active, and starts in the same directory.
- D46. Splits nest: each split divides one pane and nothing else.
- D47. The smallest pane is 200px wide and 110px high (proposed): the header and four lines of text.
- D48. A drag stops at the smallest size. A split that would make either half smaller is refused, and the split button shows as cannot be used.
- D49. Closing a pane returns its space to the pane it was split from. Nothing else moves.
- D50. When the window is resized, every pane keeps its share. A pane that reaches the smallest size holds there and its neighbours give up the rest. The window cannot be made smaller than its panes at their smallest sizes laid out as they are.
- D51. Fill makes the active pane cover the tab. The others keep running and keep their shares for when fill ends.

### 2.7 Status: the ring and the bar

- D52. A status is drawn in one of two shapes that always agree: a ring, for a pane that is out of view; and a bar, in the header of a pane that is in view.
- D53. The ring appears only at the right end of a tab (D37) and at the start of a pane's row in the palette. A row with nothing to report shows no ring.
- D54. The ring is a grey track, a coloured arc for how far the task has got, starting at the top and running clockwise, and a mark in the middle when the state needs one.
- D55. Running, progress known: the ring has six equal sections in the six ramp colours in order, and the arc fills them in order, so a task near its end is reaching green.
- D56. Running, progress unknown: a short arc in ramp steps 1, 2 and 4 turns continuously.
- D57. Waiting for a person: the arc stops where it was and takes ramp.5, with two pause bars in the middle. With no progress reported, the ring is full.
- D58. Failed: the arc stops where it was and takes ramp.3, with an exclamation mark.
- D59. Finished: the ring is full, ramp.6, with a tick.
- D60. The bell is an event, not a stage: a full ring in ramp.5 with a small bell in the middle.
- D61. The mark means a stopped state never rests on colour alone.
- D62. The bar is the ring rolled out flat: the same track and the same fill. A running bar is unbroken and its fill runs through the extended ramp, fixed to the track so more of it is revealed as progress grows.
- D63. A bar for waiting, failed, finished or the bell carries the same mark as the ring, in its middle: the bar parts to leave a 20px gap for it. Progress skips the gap, so the two halves together are the whole of 0 to 100.
- D64. A failed bar ends in a broken edge where every other bar ends in a round cap: square, with its top row of pixels 2px shorter than its bottom row.
- D65. Running with progress unknown, in the bar: a segment 22% of the bar's width, in ramp steps 1, 2 and 4, travels along the track.
- D66. The header strip itself is never tinted by status.
- D67. A tab shows one ring however many panes it holds: the most pressing of them, in the order waiting, failed, bell, finished, running. If several are running it shows the one least far along, and unknown if any of them is unknown.
- D68. When a tab is open, each of its panes shows its own status in its own header.
- D69. Output arriving in a pane that is out of view is not a status and shows nothing.
- D70. How loud each status is, most pressing first: waiting (first in the palette; notifies in the background), failed (the ring shakes once; notifies in the background), bell (swings once), finished (no motion once it has closed), running (moves only as progress arrives).
- D71. Outside the window, only waiting and failed raise a notification, only while steg is in the background, and not more than once per pane per state.

- D109. Each status ends by its own rule. Running: when the program reports an end, or exits. Waiting: when the person answers, or the program withdraws it; never on a timer. Failed: on a key press or click in that pane; never on a timer. Finished: on a key press or click in that pane, or after 10 minutes. Bell: on a key press or click in that pane, or after 30 seconds.
- D110. Opening a tab does not clear anything. Only a key press or click in the pane itself counts as having dealt with it.

How a pane's status is promoted to its tab from the CLI, and how several statuses in one tab are held, are decided in TASK-1.30 and are not restated here.

### 2.8 Status: motion

- D72. A change of status fades in over 200ms on both the ring and the bar. The bar's gap opens over the same 200ms.
- D73. On finishing, the arc and the fill run to the end first, then turn green.
- D74. A failure does not fade: ring and bar turn red at once, and the ring shakes once, about 1.5px either way, dying out within a second.
- D75. On a failure, 2px flakes spill off the bar's leading edge for ten seconds. Each leaves with a little sideways speed, tumbles as it falls under gravity, bounces once if it lands hard, and settles flat on the bottom edge of the pane header. Flakes pile like sand: one landing on a column two or more taller than its neighbour slides to the neighbour. Each lies for its own time, between two and six seconds, then fades; when one underneath goes, those above drop. The bar stays readable throughout. (The counts and timings are proposed: about 40 flakes in a header, fewer where there is less room below the bar.)
- D76. When a bell rings, the bell swings once on the ring. On the bar, a wave runs outward from the bell on both sides: a sine wave drawn as a 750 Hz tone would look through a short window, at full height at once, ringing down to a straight line in about a second.
- D77. When a pane comes into view (its tab is opened, the session is changed, fill ends) and when a status begins in a pane already in view, its bar rolls out from the width of a ring to its full width over 250ms (proposed). When a status clears, the bar rolls back and goes.
- D78. With the macOS Reduce Motion setting on, nothing moves: the arc and the fill step to their new length; an unknown arc and segment sit still; a change of status is at once; there is no shake, no flakes, no swing, no wave, and no rolling out. Every state still shows its colour, its mark, and the broken edge of D64.

### 2.9 The command palette

- D79. One palette, and it is the only thing that ever opens over the window. There are no dialogs.
- D80. Its mode is chosen by the first character typed: nothing goes to a session, tab or pane; ">" lists actions; "?" lists the three modes. Backspace over the first character returns to go to.
- D81. Cmd+P opens it empty. Shift+Cmd+P opens it with ">" typed. Cmd+, opens it with ">settings" typed.
- D82. Go to, with nothing typed, lists panes that are waiting or failed first, under "Needs you", then recent ones. Each row leads with the pane's status ring, or a pane icon when it has none, then the program's title, the session and tab, and the directory. Return goes there; Cmd+Return opens it beside the current pane.
- D83. An action that needs an answer keeps the palette open for it, with the action shown as a chip in the input. Backspace on an empty input goes back to the actions.
- D84. Each action is named by what it acts on, then what it does ("Pane: Split right"). An action from a plugin says which plugin.
- D85. The selection follows the pointer and the arrow keys, and is filled in the session colour at 18%.
- D86. When nothing matches, the palette says so, says what to try next, and Return does nothing.
- D87. The palette is 640px wide and 120px from the top. In a window narrower than 720px it keeps 40px clear on each side; it grows with its contents to at most 60% of the window's height. (the last two proposed)
- D88. Escape, or a click outside it, closes it.

### 2.10 States and flows

All of 2.10 is a proposal: none of it was in doc-1.

- D89. With no sessions, the window keeps the bar with its + and says, in the middle: what a session is, a "New session" button with its shortcut, and that Cmd+P goes to anything.
- D90. When the open session ends and others remain, the window says which session ended and that nothing else is open. It does not open another session by itself.
- D91. A pane started with a command stays when the command ends, with everything it printed. Two muted lines are added under the output: that the program ended, with its exit code if not zero; and that Return runs it again and Cmd+W closes it. Those lines are steg's and are not part of what can be copied or searched.
- D92. In such a pane, Return runs the same command again in the same pane and directory. Exit code zero shows as finished; any other as failed.
- D93. A pane started as a plain shell closes when the shell exits.
- D94. Cmd+F searches the active pane. The field takes the place of the directory in that pane's header; the status bar and the split and fill buttons step aside while it is open. It is not an overlay and does not use the palette.
- D95. Every match gets a fill.quiet fill. The current match is filled in the session colour, with its text in surface.ground so it stays readable. The header shows which match of how many. Return goes to the next and scrolls it into view; Escape closes the field and leaves the view where it was.
- D96. A tab or session is renamed in place: double click its name, or run "Tab: Rename" or "Session: Rename". The name becomes a field of the same size, with the old name selected. Return saves; Escape puts the old name back; clicking elsewhere saves. An empty tab name puts back the automatic one (D39).
- D97. Closing a pane or tab that has a program running asks first, through the palette (D83), with "Keep it running" selected and "Close anyway" below it. It names the programs that are running. Escape keeps everything.
- D98. A pane at an idle shell prompt, or whose program has ended, closes at once.
- D99. Ending a session asks in the same way, with "Keep the session" selected, how many tabs it closes, and how many programs are still running.
- D100. Whether a program is running is the server's knowledge: a foreground process other than the shell.
- D101. There is no settings window. Each setting is a palette action named "Settings: ..." that shows its current value and, on Return, opens a follow-up step with the choices.
- D106. A setting changed in the palette applies at once and lasts for this run of the server. It is an override on top of the Python config (doc-1 section 5.9), which is the only file and which steg never writes to.
- D107. When a setting's value differs from the config, its row in the palette says so beside the value. An action on that setting opens the config file at the right place and shows the line to add.
- D108. Before the config file exists, in M1, every setting is an override the server remembers across restarts of the app.

### 2.11 Accessibility

- D102. Lines are drawn in three cases only: the keyboard focus ring (D15), the hollow cursor of a pane that is not active, and the macOS Increase Contrast setting, under which each surface gains a 1px outline and the active pane a 2px outline in the session colour.
- D103. Every icon button, status ring and status bar has a VoiceOver label that says what it is and, for a status, its state and progress.
- D104. Every action reachable with the pointer is reachable with the keyboard: the bar, the tabs, the slots at their ends, the pane controls and the palette all take keyboard focus.
- D105. No state is told by colour alone: selection is also weight and fill; a stopped status has its mark (D61).

## 3. Policies applied

- team:testing and the repository's own rules govern how app tasks prove these requirements; this document does not restate them.
- No licence or dependency policy is touched: the design asks for no asset beyond the fonts named in 1.3, of which JetBrains Mono is the only one steg would ship.

## 4. Areas of concern

For Bruk to decide. None is resolved here.

1. **Status across releases: decided 2026-11-15.** M1 shows only what steg can know without a program's help: running, and finished or failed from the exit code. So in M1, D56, D58, D59 and D64 apply, with their motion; D55, D57, D60 and D76 need the status protocol and arrive in M2. doc-1's R49, R50 and R52 and its release table are still to be reworded.
2. **Session colour: decided 2026-11-15.** It follows the artifact. In M1 each session has one of the five colours, a new session takes the next one not in use, and "Session: Set colour" in the palette changes it. An arbitrary colour or gradient from the CLI (D10) stays in M2. doc-1's R26 is still to be reworded.
3. **Settings: decided 2026-11-15.** The Python config is the only file and the palette never writes to it (D106 to D108). One point is left for doc-1 section 5.9: D108 says the server remembers overrides in M1, which is state 5.9 does not yet describe.
4. **When a status ends: decided 2026-11-15.** D109 and D110. Waiting and failed never end on a timer; finished ends after 10 minutes and the bell after 30 seconds, or sooner on a key press or click in the pane.
5. **R50 needs rewording.** It says a tab with an unfocused pane shows that pane's status with words. This design shows one ring per tab with no words (D37, D67), no ring on the selected tab (D38), and nothing on a session (D32).
6. **Whether GPUI can draw all of this cheaply** is not established: the six-section ring, the gradient fixed to the track, and above all the flakes of D75, which are a small physics simulation running for ten seconds. D75 is the first candidate to simplify if it costs frames against R19.
7. **Search puts the session colour behind text** (D95), which D9 otherwise forbids. It is the one exception, and it changes the text colour to keep contrast.
8. **With D32 and no count in the bar, nothing in the window tells a person that a pane in a closed session is waiting** until they open the palette. Bruk chose this knowingly; it is recorded so it is not rediscovered as a bug.

## 5. Open questions

- [ ] Do the values marked (proposed) stand: D14, D23, D27, D47, D75, D77, D87?
- [ ] Carried forward: more tabs than fit the top bar, many session chips, reordering tabs, and moving a pane to another tab are not designed. Bruk parked them on 2026-11-14 to come back to; they do not block the first window.
- [ ] Carried forward: whether a session can be ended from the top bar, where chips have no x. Today it can only be ended there from the palette.
- [ ] Carried forward to TASK-1.29: whether two windows on one session share one pane layout and one focused tab.
- [ ] Carried forward to TASK-1.30: promotion, and its CLI form. Lifetimes are settled in D109.

## 6. Out of scope

- The terminal's own behaviour: emulation, scrollback, selection, links (doc-1 section 5.3, doc-6).
- Key bindings beyond the few named here (doc-1 section 5.6).
- Anything shown beside the terminal (doc-1 section 5.14), agents (5.11), and remote hosts (5.12).
- The CLI and the wire protocol. Where this design needs something from them (focus, status, pane shares), TASK-1.29 and TASK-1.30 carry it.
- An icon for the app, the menu bar, and the Dock.
