---
id: doc-16
title: 'Spec: repository architecture'
type: specification
created_date: '2026-11-15T02:34:00Z'
updated_date: '2026-11-15T02:43:00Z'
---
- Intent: TASK-1, "A terminal like Rex, built in Rust and GPUI and extendable through Python".
- One of steg's specs. doc-1 is the overview: it has the vocabulary, the release table and the list of specs. Requirement numbers are steg-wide and never reused.
- Rests on: decision-1 (the API description), decision-2 (the wire protocol), decision-5 (servers, clients and windows).
- Status: draft, written 2026-11-15 from decisions Bruk made that day. R98 and R101 were moved here from the server spec (doc-8) and reworded to match those decisions. R102 to R108, R110 and R111 are new.

## Scope

How steg is divided: the things that run, the packages they are built from, which package may depend on which, and how the repository is laid out. It is written at the levels of the C4 model, and the LikeC4 model in the repository is the drawing of it (R108).

It does not cover what the server holds (doc-15), what the API offers (doc-8), what travels on the wire (doc-14), or how the app looks (doc-10).

## Level 1: system context

| Element | Kind | Relationship |
|---|---|---|
| The person at the Mac | Person | Uses steg through its window and its CLI |
| steg | The system | Runs programs in terminals, keeps them running without a window, and can be driven from scripts |
| Programs in panes | External | A shell, an editor, an agent. steg starts them, feeds them input and reads their output. Some drive steg back through the CLI |
| macOS | External | Windows, pseudo-terminals, the clipboard, the system's light or dark setting |
| A plugin (M2) | External | A Python program beside the server, using the public API |
| A remote host (M3) | External | Another machine running a steg server |

## Level 2: containers, the things that run

| Container | What it is | Talks to |
|---|---|---|
| The server | One process per person per machine. Owns every session, tab, pane and running program (R1) | Programs in panes, through pseudo-terminals |
| The CLI | A short-lived process for one command | The server, over its socket, through the public API |
| Steg.app | The window. Draws what the server reports | The server, over its socket, through the public API |

A plugin is not a container of steg's: anyone can write one, so it is an external system in the context above, and it reaches the server the same way the CLI and the app do. The server and the CLI are one binary, `steg` (R61, R98). The app is a separate program. Every arrow into the server is the same socket and the same API (R36): there is no other way in.

## Level 3: components, the packages

A package here is a crate. Packages are not things that run: each container above is built from one or more of them. A package built into several containers, as the sdk is, is a component of each of them; it belongs to none more than another.

| Package | What it is | Does I/O | Built into |
|---|---|---|---|
| sdk | The public interface: the API description, the shapes that cross the API, and the protocol's logic | No | The server, the CLI, the app |
| kernel | The server's logic: the tree, layout, focus, status, the terminal engine | No | The server |
| server | The kernel plus sockets, pseudo-terminals, processes and the clock | Yes | The server |
| `steg` | The CLI, and the entry point that starts the server | Yes | The server and the CLI |
| app | The GPUI window | Yes | Steg.app |

### Who may depend on whom

| Package | May depend on |
|---|---|
| sdk | nothing of steg's |
| kernel | sdk |
| server | kernel, sdk |
| `steg` | sdk, server |
| app | sdk |

Anything not in that table is refused (R104).

## Deployment

In M1, one Mac: one server process, any number of CLI processes, and one Steg.app whose windows each hold a client to that server (decision-5). In M3 a window may hold a client to a server on another machine as well; the packages do not change.

## Requirements

- R98 (M1). The server's code is in packages no client depends on: the app does not depend on the server or the kernel, and neither does anything a client is built from. The server and the CLI ship as one binary (R61), made by the `steg` package, which holds the CLI and the entry point that starts a server and no server logic of its own.
- R101 (M1). The sdk is steg's public interface. It holds the API description (decision-1), the shapes that cross the API, and the protocol's logic: encoding, decoding, the handshake and the state of each stream. The server, the CLI and the app are all consumers of it, and none of them holds protocol code of its own.
- R102 (M1). The sdk does no I/O of its own. A consumer supplies the transport through an interface the sdk defines, so the sdk works on any runtime and over any channel a consumer can provide.
- R103 (M1). The sdk includes a provider for the local socket, which finds a server, starts one if there is none and connects. It is an optional part, off unless a consumer asks for it, so that the CLI and the app do not each write it and the kernel never builds it.
- R104 (M1). The dependencies in this spec's table are the only ones allowed between steg's packages. The repository's check refuses any other, and refuses an external dependency a package is not listed as allowed, each with its reason.
- R105 (M1). The kernel does no I/O and may depend on the sdk without its optional parts. A shape that crosses the API, such as an identifier, a status or a layout, is defined once, in the sdk. The screen is the exception: what the engine holds and what travels are different shapes (decision-2).
- R106 (M1). The sdk stays usable from another language: no GPUI type and no particular async runtime appears in its public interface, and its required dependencies are for data and serialisation only.
- R107 (M1). The repository is a monorepo. Each package builds and tests by itself and refers to nothing outside its own folder except the packages it is allowed to depend on, so that any of them could move to a repository of its own.
- R108 (M1). This architecture is drawn as a LikeC4 model kept in the repository: context, containers, components and deployment, with dynamic views for attaching to a pane and for focusing one. The repository's check validates the model. Where the model, the check's configuration and this spec disagree, this spec is right and the other two are fixed.
- R110 (M1). A boundary a program can check is checked by the repository's check, not left to a rule. Beyond which package depends on which (R104), that means: code in the sdk and the kernel that reaches for a file, a socket, a process, the clock or a runtime fails the lint, and so does the kernel building the sdk's optional parts. Each refusal says which rule it enforces and why.
- R111 (M1). Each package has a `CLAUDE.md` of its own saying what the package is, what it may and may not depend on, and how to run its tests, and pointing at this spec. The repository's rules hold what is left to judgement about a boundary, scoped to the package's files. Nothing is stated in both a lint and a rule, or in two rules.

## Areas of concern

- The sdk is public from the first release, but its Rust interface makes no promise of stability until a release says so. Only the protocol is versioned (R39).
- A consumer that supplies its own transport also takes on starting a server. Only the local-socket provider does that for it.
- The kernel depending on the sdk means a change to a public shape reaches the kernel. That is accepted in exchange for one definition.

## Open questions

- [ ] Python in M2: bindings over the sdk, with Python supplying an asyncio transport, or a pure-Python sdk generated from the API description. R106 keeps the first possible. To be decided when M2 is broken down (doc-12).
- [ ] Where the vendored ghostty source and the design artifact's generator scripts live in the repository.

## Out of scope

- The fields and methods of the API (doc-8, decision-1).
- Packaging, signing and distribution of the app (doc-13).
