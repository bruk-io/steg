---
id: decision-11
title: The app's components are built on GPUI directly, in a package of their own
date: '2026-11-16T00:50:00Z'
status: accepted
---
## Context

The app is about to be built, and its design is a design system in four pages: foundations, atoms, molecules and organisms (decision-10). Two questions came up on 2026-11-15 before any of it was written: whether the components belong in a package of their own, and whether to build them on gpui-kit, a component library for GPUI, or on GPUI alone.

gpui-kit was read on that day, at version 0.9.1 (published 2026-11-09 by Longbridge, https://github.com/longbridge/gpui-kit):

- It is not a library added beside GPUI. It pins its own GPUI crates (`gpui-pre =0.3.12`) and re-exports them, so that "an application never lists GPUI itself". steg pins the published `gpui` crate at 0.2.2. Adopting gpui-kit would change which GPUI steg is built on, and from then on gpui-kit's releases would decide when steg could take a change to GPUI.
- It has two layers: gpui-base, behaviour without appearance (button, checkbox, switch, text input, editor, toast, scrollbar, long lists, focus trapping, motion), and gpui-component, more than sixty finished components. gpui-base is Apache-2.0; gpui-kit's own licence was not confirmed from its documentation.
- About half of gpui-base's public interface was documented.

What steg's interface needs is small and mostly its own: the bar and its tabs, pane headers, the status ring, the palette, and the terminal view. The ring and the terminal view are drawn by hand whatever library is used. There is no settings window (decision-9), which is where most of a component library's parts would go. The one part that is hard to write is a text field with a cursor, selection and input methods, for the palette and for find; GPUI's own repository has a working example of one, and of long lists, custom drawing, animation and accessibility (https://github.com/zed-industries/zed/tree/main/crates/gpui/examples).

## Decision

Decided by Bruk on 2026-11-15:

1. The app's components are built on GPUI directly. gpui-kit is not adopted.
2. The components and the design's named values are a package of their own inside this repository, `steg-ui`, organised as the design is: foundations, atoms, molecules, organisms.
3. That package is pure interface. It holds no domain logic and depends on nothing of steg's; the app is where what the server reports becomes what a component is given.
4. The design artifact stays the source of every named value. The values are exported from it into the repository and the code is written from the export; they are not edited in the repository.

doc-17 holds the requirements that follow from this.

## Consequences

- steg writes its own text field, long list and focus handling, from GPUI's examples. That is the cost of this decision, accepted for one fewer foundation under the part of the app where typing latency is decided.
- Because the app sees only `steg-ui`'s components, a later change of mind is contained in that one package.
- doc-16 gains a package, and the repository's check gains its allowed dependencies.
- What would reopen this: a component that proves expensive to write and that gpui-base provides, such as a full text editor or a table; or finding that the published `gpui` crate is not kept up, which is a question about which GPUI to build on and is separate from components. Whether the published crate is kept up was not checked when this was decided.
