---
id: decision-7
title: >-
  Program status: what each release shows, one ring per tab, and when a status
  ends
date: '2026-11-15T01:11:00Z'
status: accepted
---
## Context

The spec put program status in the second release and notifications in the third, while the app's design is built around showing status. The design also contradicted itself on when a finished or failed status goes away, and nothing said what a tab holding several panes shows. Decided across 2026-11-14 and 2026-11-15, and recorded at the time on the archived tasks TASK-1.29 and TASK-1.30.

## Decision

1. **Across releases.** The first release shows only what steg can know without a program's help: running, and finished or failed from the exit status. The Program Status Protocol, with progress, waiting for a person, the bell as a status, and promotion, stays in the second release.
2. **One ring per tab.** A tab shows one status, for the most pressing of its panes, in the order waiting, failed, bell, finished, running. A session shows none. The selected tab shows none either, because its panes are in view.
3. **A set, not a queue.** Within one pane a new report replaces that pane's last. A tab holds each pane's current status and shows the most pressing, then the next when one ends. The order is how pressing, then newest, never first in first out.
4. **Promotion.** A pane's status can be promoted to its tab from the CLI. A pane that is waiting or has failed still takes the ring. A pane may promote itself; promoting another pane is for the CLI only.
5. **When a status ends.** Running: when the program reports an end, or exits. Waiting: when the person answers, or the program withdraws it; never on a timer. Failed: on a key press or click in that pane; never on a timer. Finished: on a key press or click in that pane, or after 10 minutes. Bell: on a key press or click in that pane, or after 30 seconds. Opening a tab clears nothing.
6. **Notifications.** Only waiting and failed notify, only while steg is in the background, and not more than once per pane per state.

## Consequences

- In the first release a running status can only be of unknown progress, because without the protocol steg cannot know how far along a program is.
- Finished and failed for a command typed at a shell prompt depend on the shell reporting prompt marks, which the spec already requires in the first release.
- With no status on a session and no count in the bar, nothing in the window says that a pane in a closed session is waiting until the palette is opened. Bruk chose this knowingly.
- Still open: whether running needs a lifetime for a program that stops reporting without exiting, and the form of the promotion command.
