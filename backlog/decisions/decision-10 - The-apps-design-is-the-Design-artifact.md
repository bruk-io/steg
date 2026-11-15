---
id: decision-10
title: The app's design is the Design artifact
date: '2026-11-15T01:11:00Z'
status: accepted
---
## Context

The app's look and behaviour were designed in a Claude Design artifact across 2026-11-14 and 2026-11-15: a design system in four pages (foundations, atoms, molecules, organisms), storyboards, and a working mock of the window. The orchestrator then restated it as 110 numbered requirements in a board document, because the workers could not read the artifact. That turned out to be a limit of the workers' tool list, not of artifacts.

## Decision

Decided by Bruk on 2026-11-15: the design artifact is the source for how the app looks and behaves, and the spec for the app refers to it; it does not restate it. The artifact is "steg app design", https://claude.ai/artifact/Ex9HHnDNY8C8fxV6BR4WVZ, private to Bruk.

## Consequences

- The app's spec is short: what the artifact cannot say, such as which release each part belongs to, and a pointer for everything else.
- Workers and reviewers need to be able to read the artifact. An intent for that is open on the asdlc repository as pull request 56.
- The artifact was built and checked by script and never rendered by the orchestrator; Bruk is the one who has looked at it.
- Several values in the design were chosen by the orchestrator to make a rule concrete and have not been confirmed: the smallest pane, the pane header's breakpoints, the pressed fill, the roll-out time, and the counts and timings of the failure flakes.
