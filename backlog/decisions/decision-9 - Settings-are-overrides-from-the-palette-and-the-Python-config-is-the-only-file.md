---
id: decision-9
title: >-
  Settings are overrides from the palette, and the Python config is the only
  file
date: '2026-11-15T01:11:00Z'
status: accepted
---
## Context

The app's design has no settings window: each setting is an action in the palette. The design had also drawn a plain settings file, while the spec makes configuration Python. Bruk asked how tools with a Lua config handle this; they do not write to the file at all.

## Decision

Decided by Bruk on 2026-11-15:

1. The Python config is the only file. There is no separate settings file.
2. A setting changed in the palette applies at once and lasts for this run of the server. It is an override on top of the config, and steg never writes to the config file.
3. When a value differs from the config, the palette says so, and an action opens the config at the right place with the line to add.
4. In the first release, before the config exists, every setting is an override the server remembers.

## Consequences

- steg never rewrites code a person wrote, which is the convention of tools with a code config.
- Keeping a change is an explicit step by the person.
- The server holds override state in the first release that the Python config section of the spec does not yet describe.
