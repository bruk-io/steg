---
id: decision-8
title: Session colour in the first release
date: '2026-11-15T01:11:00Z'
status: accepted
---
## Context

The spec put a colour per session in the second release, but the app's design uses the session's colour for the tint on the open session, the cursor and the window's ground.

## Decision

Decided by Bruk on 2026-11-15: session colour follows the design artifact. In the first release each session has one of five colours (violet, pink, orange, blue, green); a new session takes the next one not in use, and the palette changes it. An arbitrary colour or a gradient set from the CLI or Python stays in the second release.

## Consequences

- The server stores a colour per session from the first release.
- The five colours are the design's: four steps of its ramp, and one blue that is off the ramp.
