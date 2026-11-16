---
id: doc-17
title: 'Spec: UI components'
type: specification
created_date: '2026-11-16T00:50:00Z'
updated_date: '2026-11-16T00:50:00Z'
---
- Intent: TASK-1, "A terminal like Rex, built in Rust and GPUI and extendable through Python".
- One of steg's specs. doc-1 is the overview. Requirement numbers are steg-wide and never reused.
- Rests on: decision-10 (the design is the artifact), decision-11 (the components are built on GPUI directly), doc-10 (the app), doc-16 (the packages).
- Status: draft, written 2026-11-15 from Bruk's decisions that day: a component crate of its own, pure UI with no domain logic, organised as foundations, atoms, molecules and organisms with one rule for each, built on GPUI without gpui-kit, and the design artifact as the source of every named value.

## Scope

The package the app's interface is built from: where it sits, what may be in it, how it is organised, and how the design's named values reach the code. What each component looks like and how it behaves is in the design artifact (decision-10) and is not restated here. What the app does with the components, and everything about the server, is doc-10's and doc-16's.

## The package

`crates/ui`, package `steg-ui`. It is a component of Steg.app only.

| Package | What it is | Does I/O | May depend on |
|---|---|---|---|
| ui | The app's components and the design's named values. Pure interface: it knows nothing of sessions, tabs, panes or the server | No | nothing of steg's; `gpui` |
| app | The GPUI window. Maps what the server reports onto components and what the user does onto requests | Yes | sdk, ui; `gpui` |

```
crates/ui/
  design/foundations.json   exported from the design artifact; never edited by hand
  src/
    foundations/            named values and the functions that read them
      generated.rs          written by the tool from design/foundations.json
    atoms/
    molecules/
    organisms/
  gallery/                  a program that shows every component, light and dark
```

## The four levels

Each level has one test that decides what belongs in it. The levels and their names are the design's own four pages.

| Level | The test | From the design |
|---|---|---|
| Foundations | It draws nothing. It is a value, or a function from values to a value | Colours, the two ramps, type, sizes, corners, the terminal themes, the focus ring's measurements |
| Atom | It draws one thing, is built from no other component of this crate, and has no say in where it is placed | Icon, label, key cap, swatch, the status ring, a button |
| Molecule | It is built from atoms into one unit that does one job, and holds no state beyond the passing kind: hovered, pressed, open, being edited | A tab in the bar, a pane's header, a row in the palette |
| Organism | It arranges several independent units into a region of the window, and owns their layout and what happens between them | The bar, the pane grid, the palette |

Where a component could be a molecule or an organism, the question is whether it is one unit or an arrangement of several that could each stand alone. A component used by exactly one other and never alone is part of that other, not a level of its own.

## Requirements

- R117 (M1). The app's components and named values are in a package of their own, `steg-ui`, which depends on nothing of steg's: not the sdk, the kernel or the server. The repository's check refuses any other dependency (R104). The app depends on it and is where the server's records become what a component is given.
- R118 (M1). A component is given plain values and reports what the user did; it decides nothing about what that means. No type in the package's public interface names a session, tab, pane, client, status of a program or any other thing of steg's domain. A component does no I/O and reads no clock, beyond what GPUI does to animate it.
- R119 (M1). The package is organised as foundations, atoms, molecules and organisms, by the tests above. A level uses only the levels before it: organisms use molecules, atoms and foundations; molecules use atoms and foundations; atoms use foundations; foundations use nothing of the crate. The repository's check refuses an import that goes the other way.
- R120 (M1). Every colour, size, radius, duration and font a component uses is a named value from foundations. No component writes one as a literal. The repository's check refuses a colour literal outside foundations.
- R121 (M1). The design artifact is the source of every named value (decision-10). The values are exported from it into one data file in the package, under the names the design gives them, and the code that holds them is written from that file by a tool in `tools/`. Neither the data file nor the generated code is edited by hand, and `just check` fails when the generated code does not match the data file. A value changes by changing the design and exporting again.
- R122 (M1). The export is a deliberate step, since only an agent or a person can read the artifact: the data file records the artifact's version it was exported from, and a change to it arrives as a pull request whose diff shows each value that changed.
- R123 (M1). Each named value has its light and its dark form where the design gives both, and a component takes its values through the theme in force, never from one form directly.
- R124 (M1). A program in the package, the gallery, shows every component in every state the design draws, in light and in dark. It is where a component is looked at while it is built and the source of the image a pull request that changes the interface must show.
- R125 (M1). Every component a person can act on is reachable and usable from the keyboard, shows the design's focus ring when focused from the keyboard, and gives assistive technology its role, name and state. A component that moves has the design's Reduce Motion form.
- R126 (M1). Each level has one rule in the repository's plugin, delivered when a file of that level is touched, that states the level's test, what it may use, and one file that shows the pattern. What a program can check (R117, R119, R120, R121) is a check and is not restated as a rule.

## The design, and where its values are today

The artifact is "steg app design", https://claude.ai/artifact/Ex9HHnDNY8C8fxV6BR4WVZ. Its foundations are the pages `Tokens`, `Layout` and `Details`.

Checked on 2026-11-15 against the artifact's version 1794702644-d50e:

- The artifact holds the foundations as drawn pages only. The data they were drawn from, and the scripts that drew them, were never published to it: they were working files of the session that built the design. A copy was kept that day outside the session's temporary directory. The first task under this spec gives them a durable home.
- That data has 17 colours, each with a name, a dark value, a light value and its use, and all 17 appear in the artifact's page under the same names and values. The colours and the 16 ANSI colours of each terminal theme can be exported as they are.
- Type (7 entries), sizes (10 entries), the ramps and the terminal themes' base colours are written as sentences, for example "Steps of 2, 4, 6, 8, 10, 12, 16". They have to be given a structured form at the source before they can be exported.

## Built on GPUI

decision-11: the components are built on GPUI directly, with no component library between. The examples in GPUI's own repository are the reference: https://github.com/zed-industries/zed/tree/main/crates/gpui/examples. Among them: `input.rs` for a text field with a cursor, selection and input methods; `uniform_list.rs` for a long list; `painting.rs` and `gradient.rs` for drawing the status ring; `animation.rs`; `a11y.rs`, `focus_visible.rs` and `tab_stop.rs` for accessibility and keyboard order; `testing.rs`. They follow GPUI's main branch and steg pins a published version, so an example may use something the pinned version lacks.

## Areas of concern

- The export crosses from the artifact to the repository by hand (R122). A change to the design that is not followed by an export leaves the two apart, and nothing in `just check` can see it. Reviewing a design change includes asking for the export.
- Whether GPUI can draw the status ring within doc-10's budget is still not established (doc-10's open question). The ring is an atom, so the component crate is where it is answered, early.
- R125 depends on what the pinned GPUI version offers assistive technology. The first interactive atom establishes it, and what cannot be done yet is recorded then, not assumed now.

## Open questions

- [ ] Whether the working data and scripts that drew the design's pages are published into the artifact beside the pages, so that the artifact holds its own values as data, or are kept in the repository as the tool that writes the artifact's pages. For Bruk.
- [ ] The form of the generated code for a value that has no light and dark pair, such as a size.

## Out of scope

- What any component looks like or does: the design artifact.
- The terminal view's drawing of cells: doc-9 and the app's own task. It uses foundations for its themes and is not a component of this package's four levels unless its plan finds it should be.
- Loading themes or values at run time, and anything a plugin could change: M2.
