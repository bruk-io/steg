---
id: doc-5
title: 'Decision: steg''s wire protocol'
type: specification
created_date: '2026-11-14T05:13:00Z'
updated_date: '2026-11-15T00:51:00Z'
---
- Intent: TASK-1, "A terminal like Rex, built in Rust and GPUI and extendable through Python"
- Spec: doc-1. This is design decision 2 in its section 7, and it serves R1, R2, R36 and R39.
- Task: TASK-1.2. The throwaway code and the three scripts that produce every number here are in
  `spikes/task-1.2-wire-protocol/`; TASK-1.12 deletes that directory when the real attach path
  lands.
- Status: the spike's answer. One recommendation is Bruk's to confirm (section 10).
- Extended 2026-11-15 with section 13, which is not the spike's: it is what the protocol has to carry for the decisions Bruk made that day on focus, clients, terminal size, status and pane layout (doc-1 R83 to R91). Sections 1 to 12 are unchanged, and nothing in section 13 was measured.

## 1. The decision, in one page

**Framing.** A length-prefixed frame: a 4-byte big-endian length, then a 5-byte header (a
`u32` stream id and a `u8` kind), then the payload. The cap on a frame body is 8 MiB. Newline
-delimited JSON, which TASK-1.3 used as a declared stand-in, is replaced.

**Topology.** One connection, multiplexed. Stream 0 carries the handshake, control requests and
responses, and events; each attachment gets a stream of its own. The writer drains control
before any terminal data and then takes one frame from each attachment in turn. Rex's shape, a
ticket for a second data connection per block, is not needed: measured, a 100 MB flood in one
pane leaves a control call at 0.060 ms and a keystroke in another pane untouched.

**How terminal output reaches a client.** A snapshot when the client attaches, and then **screen
updates the server computes**: the rows that changed, the cursor, and a count of how many rows
have ever scrolled off. Scrollback is **not** streamed; a client that wants it asks for a page
(0.04 ms for 100 rows). Raw pty bytes replayed by an emulator in the client were built and
measured too, and are the runner-up (section 4).

**Version.** The first frame of a connection is a hello whose shape is frozen for every future
version (section 7), so a mismatch comes back as a message naming both versions rather than as a
parse error.

The reasons, in the order they carried weight:

1. **One emulator in the system.** The server has to hold the screen whatever the wire carries
   (section 2), so raw bytes mean a *second* emulator in every client, and between resyncs that
   client's emulator is a copy it treats as the truth, which `.claude/rules/architecture.md:11`
   forbids. With screen updates, a client holds plain rows: the CLI needs no emulator for
   `pane capture`, and neither does a Python plugin (R42, R75) - proved by a dependency-free
   Python client that ends up with the server's screen byte for byte.
2. **It is the only representation that stays cheap when a program floods.** 100 MB printed in
   one pane: 3.1 MB on the wire against 63.1 MB for raw bytes, with no dropped output and no
   resynchronisation at all, because one update per tick is coalescing by construction.
3. **Back-pressure has a cheap answer.** A client that falls behind is given one fresh
   screen-sized snapshot, not a 1.3 MB one (section 6).

The cost, stated plainly: latency to the client's grid is one tick (10.1 ms at an 8 ms tick,
1.34 ms at a 1 ms tick) against 0.057 ms for raw bytes, and the cell model has to carry
everything the app draws. Both are section 9.

## 2. What was already settled before any candidate existed

The server holds the emulator in every candidate. This is not this spike's choice:

- R15 keeps 10,000 lines of scrollback per pane **on the server**.
- `pane capture` (doc-2 section 8) prints a pane's screen and scrollback as text with no client
  attached, and a Python plugin on the public API (R42, R75) cannot replay VT bytes.
- `.claude/rules/architecture.md:11`: a client "never keeps a copy it treats as the truth".
- Acceptance criterion 2 includes output printed while **no client was attached at all**, which
  has no byte stream left to replay.

So the question was never who emulates. It was what the server puts on the wire, and a snapshot
on attach is forced in every candidate. Acceptance criterion 1's two options ("raw bytes that a
client-side emulator replays, or screen updates the server computes") are therefore two choices
of what *follows* the snapshot. Both were built.

## 3. The wire format, in enough detail to build from

```
frame   = length:u32be body
body    = stream:u32be kind:u8 payload
kind    = 0 hello | 1 control (UTF-8 JSON) | 2 data (opaque bytes)
length  = len(body), at most 8 MiB
```

- **Stream 0** carries the handshake, control requests, responses and events. Every other stream
  id belongs to one attachment, and the server hands it out in the attach result.
- **Control** payloads are JSON: `{"type":"request","id":N,"method":"...","params":{...}}`, and
  `response`, `error`, `event` in reply. An event carries no id, so a long-lived subscription
  (`event watch`) never blocks a request on the same connection.
- **Data** frames on an attachment's stream are opaque bytes: terminal output in raw-byte mode,
  and input on its way to the program in either mode. A client sends input on the same stream it
  attached with, so no second connection and no second call is needed to type.
- **Updates** for a screen-mode attachment are control frames *on the attachment's stream*, not
  on stream 0: they are JSON, so they belong to the control kind, while the stream id keeps them
  out of the request and response path.

Snapshot, as plain data with no emulator needed to read it: sequence number, size, the screen as
rows, the scrollback as rows, the cursor, whether the alternate screen is on, and `rows_ever`.
A row is attribute runs (`{"text","fg","bg","flags"}`), not a cell array; the server resolves
the palette so a client never needs one.

An update is: the rows that changed with their index, the cursor, `rows_ever`, and - only for a
client that asked for streamed scrollback - the rows that scrolled off and the count dropped.
The other update is `resync`, which carries a whole snapshot and means "throw away what you
have and start from this".

### Two rules that are load-bearing, and both were learned the hard way

- **The snapshot and the stream that follows it must be ordered on the wire, not just produced
  atomically.** The server takes the snapshot on the thread that owns the terminal, in the same
  step as the subscription, so the stream resumes at `snapshot.seq + 1`. That is not enough: the
  attach *result* travels on stream 0 and the output travels on the attachment's stream, and
  with the attachment's queue registered with the writer before the result was queued, a client
  was handed output for a stream it had not been told about yet and silently dropped 12 lines.
  The rule: queue the attach result first, register the attachment's queue second. The spike's
  client counts frames for streams it does not know and reports the count, so the rule stays
  tested: every recorded run shows `unknown_stream_frames=0`.
- **Scrollback rows need absolute identity, not a window position.** A page fetched by window
  index stitches two different moments together as soon as the window slides: the first attempt
  produced a client whose scrollback and screen disagreed across their seam. `rows_ever` gives
  every row an absolute id, so a page request says which rows it wants and the answer says which
  rows it got (`first_row`), and a client can tell "here is your page" from "those rows are
  gone".

## 4. The measurements

Every number below is the recorded output of `spikes/task-1.2-wire-protocol/measure`, run through
`asdlc record-check` on this task. macOS 27.0.0 arm64, 28 cpus, rustc 1.98.1, release profile
(opt-level 3, lto off), `alacritty_terminal` 0.26.0 as the emulator, 80x24 panes, 10,000-line
scrollback, a 64-frame queue per attachment, and a deliberately slow client that waits 2 ms after
every frame it applies.

**Latency** is a byte into the pty, to that byte appearing on the client's own copy of the screen.
It stops at the client's grid: the drawing half is TASK-1.1's measurement, and R19's 16 ms has to
cover both halves added together. It is not photon-to-glass.

### 4.1 A 100 MB flood in one pane, with a slow client attached to it

| data path | tick | quiet latency p50 / p95 | wire | dropped and resynced | flood | keystroke in another pane p95 | control call p95 |
|---|---|---|---|---|---|---|---|
| raw bytes | 8 ms | 0.057 / 0.089 ms | 63.1 MB | 46 | 2.5 s, 66.6 MB/s | 0.144 ms | 0.067 ms |
| screen deltas, scrollback streamed | 8 ms | 10.04 / 10.56 ms | 110.1 MB | 24 | 2.5 s, 68.4 MB/s | 10.18 ms | 0.072 ms |
| **screen deltas, scrollback on request** | 8 ms | 10.13 / 10.63 ms | **3.1 MB** | **0** | 2.3 s, 79.4 MB/s | 10.47 ms | 0.060 ms |
| screen deltas, scrollback on request | 1 ms | **1.34 / 1.43 ms** | 4.8 MB | 255 | 2.8 s, 54.4 MB/s | 1.44 ms | 0.062 ms |

What this says, in order:

- **Streaming scrollback is what makes a delta stream expensive**, not the deltas. With every
  scrolled row carried, the stream costs more than the bytes it describes (110 MB for 100 MB of
  output). With scrollback left to a request, the same workload costs 3.1 MB, because the stream
  then costs the screen times the tick rate and nothing else.
- **No head-of-line blocking at all**, in any mode: a control call during a 100 MB flood stays at
  0.060-0.072 ms at the 95th percentile, and a keystroke in another pane is the same during the
  flood as on a quiet machine. One multiplexed connection with control-first and a round-robin
  writer is enough; the ticket-and-second-connection shape is not needed.
- **The tick is the latency knob, and it is a real trade.** At 1 ms the delta stream is 1.34 ms
  to the client's grid, which leaves about 14 ms of R19's budget for drawing. It costs 255
  resynchronisations under the flood (a slow client cannot keep up with 1,000 updates a second)
  and 1.7 MB more on the wire. A tick of 1-2 ms, or publishing on idle with a ceiling, is the
  recommendation for TASK-1.12 to settle with a GPUI client in front of it.

### 4.2 Attaching, and fetching scrollback

| | |
|---|---|
| snapshot with 10,000 scrollback rows and a 24-row screen | 1,343,355 bytes |
| the whole attach call, that snapshot included | 34-46 ms |
| a page of 100 scrollback rows | p50 0.040 ms, p95 0.089 ms |
| a control call on a quiet server (`pane.list`) | p50 0.022 ms, p95 0.040 ms |

A full-scrollback snapshot is 1.3 MB of JSON and about 40 ms. That is acceptable for an attach
and far too expensive for a resynchronisation, which is why section 6's policy exists. If 40 ms
proves visible when the app reopens with many panes, the fix is in the format rather than the
protocol: the snapshot's rows could be fetched in pages like the scrollback, since `rows_ever`
already makes them addressable. Not needed for M1 on this evidence.

### 4.3 The framing

| | wire, same workload | note |
|---|---|---|
| length-prefixed, screen deltas | 3.13 MB | |
| newline-delimited JSON, screen deltas | 3.16 MB | +1%: both carry JSON, so only the newline differs |
| length-prefixed, raw bytes | 63.1 MB | |
| newline-delimited JSON, raw bytes | 47.2 MB | lower only because more was dropped (34 drops against 46); not a saving |

The byte counts do not decide this axis, and the raw-byte comparison is confounded: under a flood
with dropping, the wire total measures what got through. What decides it:

1. **NDJSON cannot carry terminal bytes without base64**, which is +33% by construction. In the
   run above that cost showed up as other work rather than as bytes: a keystroke in another pane
   went from 0.144 ms to 3.611 ms at the 95th percentile while the server base64-encoded 100 MB.
2. **NDJSON cannot preserve a control payload's bytes.** It has to parse the payload into JSON to
   embed it in its envelope and serialise it again, and object key order changes on the way
   through. The JSON *object* survives - which is what TASK-1.3's contract needs, and
   `probe-wire/tests/framing.rs` asserts exactly that - but byte-identical payloads do not, so
   nothing downstream can ever sign or hash a payload. The length framing never looks inside.
3. **A delimiter cannot bound a first frame.** Recorded in `gates`: a client speaking the other
   framing gets closed at once by the length framing, and hangs for ever against NDJSON, which
   is still waiting for a newline. Worse, under NDJSON a first frame that is not valid JSON
   fails in the framing layer *before* the handshake can answer, so the server closes the
   connection with no message; under the length framing the same client gets
   `malformed-hello` naming the server's version. Framing errors and payload errors stay
   separate only when the framing does not parse the payload.

## 5. Acceptance criterion 2: a client killed mid-output, and a new one restoring the screen

`spikes/task-1.2-wire-protocol/reattach-demo`, recorded on this task for each of the three data
paths. The oracle was written before the run and is code, not judgement
(`probe-client verify`):

1. A pane prints 12,000 numbered lines at about one a millisecond, past the 10,000-line
   scrollback limit.
2. Client A attaches, is seen live, and is **SIGKILLed** mid-output. Not a clean detach, so it
   also proves the server survives a broken pipe mid-frame without dying or stalling the program.
3. Nothing is attached at all for about 1.5 s while the program keeps printing.
4. Client B attaches and reconstructs. The server's own byte counter says when the program has
   really finished, and client B is then told to stop and dump what it holds.
5. The checks: client B's rendered screen and scrollback equal the server's own capture **byte for
   byte**; the numbered lines run with no gap and no repeat; the last line is the last the program
   printed; and every line printed while nothing was attached is there.

The recorded result, in the screen-delta mode this document recommends:

```
   client A killed with SIGKILL while the pane was at line 3361
   the pane reached line 6696 with no client attached
   client B ready: seq=6727 snapshot_bytes=901957 scrollback_rows=6706 attach_ms=20.6
   client B: frames=292 bytes_read=2416687 resyncs=0 seq=11998
             unknown_stream_frames=0 fetched_scrollback_rows=10000
same  the reattached client and the server agree byte for byte (120277 bytes, 10024 lines)
whole 10023 numbered lines run from 1978 to 12000 with no gap and no repeat
last  the last line is 12000, as the program printed
gap   every line from 3361 to 6696, printed while no client was attached, is there
PASS
```

All three data paths pass it. In screen-delta mode the scrollback comes back in two parts, which
is the design: the 10,000 rows in the attach snapshot, and then pages on request, anchored on
`rows_ever` (section 3).

The same script also attaches two clients to one pane at once, each declaring its own size, and
kills one: the survivor keeps receiving output and still matches the server. The sizes are
carried per attachment, so R7 is left open; which size wins when they differ is design decision
6 and M3's question, untouched here.

## 6. Back-pressure, which turned out to be the decisive question

A client killed mid-output is the limiting case of a client that reads slowly, and R19's second
sentence ("does not freeze the app or delay input to other panes") forces a policy. Three were
possible and two were measured.

**Block the program until the client catches up.** Measured, with raw bytes and a slow client:
the 100 MB flood took **32.0 s instead of 2.5 s** (3.2 MB/s against 66.6 MB/s) and the thread
that owns the terminal was blocked for **29.3 s**. In this spike one thread owns one pane, so the
harm stayed inside the flooded pane's own program; a server that served several panes from one
task would have spread it to all of them. Either way this is the policy R19 forbids, and it is
out.

**Grow the queue.** Not measured, and not a policy: a client that never catches up takes the
server's memory with it.

**Drop and resynchronise.** Each attachment has a bounded queue (64 frames here). When a frame
cannot be queued, nothing more is queued for that attachment until one fresh snapshot is, and
the attachment is then up to date again. This is where the data paths differ most:

- Screen deltas coalesce for free: at most one update per tick whatever the output rate, so at an
  8 ms tick the flood never filled the queue at all (zero drops, zero resynchronisations).
- Raw bytes cannot be coalesced, so the raw-byte path needs the drop-and-resynchronise path
  anyway - 46 times in the flood - which means it needs the whole snapshot path *and* an
  emulator in the client. It cannot be the simpler option it looks like.

One number inside that policy matters enough to be a rule: **a resynchronisation must not carry
the scrollback.** At a 1 ms tick the slow client was resynchronised 255 times, and with a full
1.3 MB snapshot each time the flood cost **323 MB** on the wire. Sending the screen alone to a
client that does not track scrollback brought the same run to **4.8 MB**, a 67-fold drop, with
the latency unchanged. A client that was behind asks for the scrollback pages it wants.

## 7. Acceptance criterion 3: how the protocol carries a version

The hello is the first frame of a connection, in either direction. **What is frozen for every
future version:** the 4-byte big-endian length and its 8 MiB cap, that the first frame's payload
is UTF-8 JSON, the two keys `steg` (a magic, so a connection to something else fails with a
reason) and `protocol` (an integer), and that unknown keys are ignored. That is what lets a
mismatch be answered rather than parsed.

```
client -> {"steg":"steg","protocol":1,"client":"steg-cli/0.1.0"}
server -> {"steg":"steg","protocol":1,"server":"steg/0.1.0","api":"0.1.0"}
or     -> {"steg":"steg","error":"protocol-mismatch","client_protocol":2,"server_protocol":1,
           "message":"steg protocol mismatch: the client speaks version 2 and this server
                      speaks version 1"}
```

**They work together when the integer matches exactly**, and otherwise not. One number covers
the framing, the control envelope and the terminal data representation, because they ship in one
binary (R61) and a mismatch between them is not partly recoverable. What lets a client be newer
without a framing mismatch is the API's own version, `api`, which is the self-description's
(TASK-1.3): a client may know of methods this server does not have, and finds out by asking.

Recorded, from `gates` and from `probe-wire/tests/version.rs`:

- a client at protocol 2 and a client at protocol 0 each get a message naming both versions;
- a first frame that is not JSON, that has the wrong magic, that is a control frame instead of a
  hello, or that is missing `protocol`, gets `malformed-hello` naming the server's version and
  saying the client's is unknown;
- a first frame declaring 4 GiB is refused against the cap with nothing allocated for it, and the
  server is still serving afterwards;
- an unknown field in an otherwise valid hello is ignored, which is the forward-compatibility
  promise in action.

**One thing the server must add, and this spike does not have: a timeout on the handshake.** A
client that opens the connection and sends half a hello holds a connection for ever. TASK-1.9
owns it.

## 8. The gates, each proved by a run rather than an assertion

The decision rule was fixed in the plan before any of this existed. Every gate passed, for both
framings.

- **Usable from outside Rust (R42, R75).** `client.py` is 200 lines of Python with an empty
  dependency list and no emulator. It does the handshake, reads the self-description, opens a
  pane, attaches, applies deltas, sends input, and ends up holding the server's screen **byte for
  byte** (36,001 bytes, 3,001 lines). That a plugin needs no package and no generated code is
  part of the answer.
- **Survives a byte stream it does not control (R5 now, R55 later).** The same client, through a
  relay process over two pipes, does a control call and a whole attachment. The framing is
  self-delimiting, there is no file-descriptor passing, no dependence on datagram boundaries, and
  no peer-credential check inside the protocol, so SSH later needs nothing new.
  `probe-wire/tests/framing.rs` decodes a captured stream fed one byte at a time, in random chunk
  sizes, and in one piece, to the same frames. **Not tested: a real SSH channel.** `ssh` to this
  machine is refused (no listener), and `gates` says so in its output rather than implying
  otherwise.
- **Leaves R7 open.** Two attachments on one pane, each with its own declared size, one killed.
- **Licence (R59).** Section 9.
- **Clear of the workspace.** Nothing outside `spikes/task-1.2-wire-protocol/`, no root
  `Cargo.toml`, its own `[workspace]` table, and `just check` skips every Rust step exactly as it
  did before this branch.

One finding from the gate runs that belongs to the server rather than the protocol: the mode a
Unix socket is left with depends on the process's umask, so it has to be set rather than
assumed. The spike sets 0600 after binding, and TASK-1.9 should too (R5).

## 9. What this leaves to be built, and the risks in it

**The cell model is the real work this choice leaves.** The snapshot's row format here carries a
character, a resolved foreground and background, and five flags. That is a demonstration, not the
final model. Before the app can draw everything R14 names, the format needs at least: wide
characters and their spacers, hyperlink identity for OSC 8, underline style and colour, the
cursor's shape, and the alternate screen's own state. Each addition is a protocol change, so this
list belongs in TASK-1.12's plan rather than being discovered later. Raw bytes would have pushed
that work into every client instead, which is worse, but it is not free either way.

**The tick is unsettled.** 8 ms gives 10 ms to the client's grid, which leaves too little of
R19's 16 ms for drawing; 1 ms gives 1.34 ms and costs resynchronisations under a flood. The
answer wants a GPUI client in front of it, which is TASK-1.16, and the knob is a server setting
either way.

**Dependence on TASK-1.1.** The protocol does not depend on which emulation library wins: the
server reads a snapshot out of the emulator on the thread that owns it and never shares the
handle, which is the shape `libghostty-vt`'s `!Send + !Sync` handles force and the shape
`.claude/rules/rust.md:29` prescribes anyway. What does depend on the choice is how faithfully a
snapshot can be read out of the emulator, and how much the cell model above costs to fill.
`alacritty_terminal` was the instrument here, declared as one, for being pure Rust with no Zig
and no network at build time.

**Where the evidence is thin, on purpose.** The separate-data-connection topology (T2) was
weighed on paper, not built: T1 cleared the threshold the plan set for it, so the comparison
would have cost a second code path for no decision. What would overturn it: a measurement
showing one multiplexed connection cannot carry several flooding panes at once (this spike
flooded one), or an SSH channel behaving differently from a pipe pair. The SSH run itself is
untested here, for want of a listener.

## 10. For Bruk to confirm

**Acceptance criterion 1 is worded as a binary choice, and the answer is a hybrid.** Output
printed while no client was attached has no byte stream left to replay (section 2), so a snapshot
on attach is forced by the criteria themselves, and the two options are two choices of what
follows it. The recommendation is: snapshot on attach, then screen updates, with scrollback on
request. The plan said this would be asked rather than decided by the worker.

Nothing else here needs a decision before TASK-1.9 and TASK-1.12 start.

## 11. What each sibling task inherits

- **TASK-1.3**: the framing that replaces its newline-delimited JSON stand-in. Its JSON
  self-description payload survives unchanged: control payloads are JSON bytes the framing does
  not touch, and the Python client reads the description with no Rust and no generated code. One
  caveat if it ever matters: under the stand-in the bytes of a payload were not preserved
  (section 4.3), and under the chosen framing they are.
- **TASK-1.9**: the handshake's wire shape and what is frozen in it, the mismatch message its
  acceptance criterion 4 has to produce, the socket's mode (0600, set rather than assumed), and
  the handshake timeout this spike does not have.
- **TASK-1.12**: the attach sequence and its two ordering rules (section 3), the snapshot's
  contents, `rows_ever` and absolute scrollback addressing, the back-pressure policy with a
  screen-sized resynchronisation (section 6), the tick as a setting, and the cell model's
  remaining fields (section 9).
- **TASK-1.5**: the licences its `deny.toml` must allow for the protocol itself: MIT, Apache-2.0,
  `MIT OR Apache-2.0`, `Apache-2.0 OR MIT`, `Unlicense OR MIT`, `(MIT OR Apache-2.0) AND
  Unicode-3.0`. The protocol needs `serde`, `serde_json`, `bytes`, `tokio` and `tokio-util`, and
  `base64` only while the stand-in framing exists.
- **TASK-1.1**: nothing it must decide differently, and one thing to weigh - the chosen path asks
  the emulator for a faithful cell snapshot on demand, so how cheaply and completely each library
  gives one is worth a line in its own answer.

## 12. Licences (R59)

Nothing in the dependency graph is GPL or AGPL. The protocol crate's own graph is
`MIT`, `MIT OR Apache-2.0`, `Apache-2.0 OR MIT`, `Unlicense OR MIT` and
`(MIT OR Apache-2.0) AND Unicode-3.0`: `base64` 0.23.1, `bytes` 1.12.1, `errno`, `futures-core`,
`futures-sink`, `itoa`, `libc`, `memchr`, `mio`, `pin-project-lite`, `proc-macro2`, `quote`,
`serde` 1.0.229, `serde_core`, `serde_derive`, `serde_json` 1.0.154, `signal-hook-registry`,
`socket2`, `syn`, `tokio` 1.54.1, `tokio-macros`, `tokio-util` 0.7.20, `unicode-ident`, `zmij`.

The only Apache-2.0-only crate anywhere in the spike is `alacritty_terminal` 0.26.0, the declared
instrument, which is TASK-1.1's decision and not this one's. `portable-pty` 0.9.0 (MIT) pulls in
`anyhow`, `nix`, `rustix`, `serial2`, `signal-hook`, `filedescriptor`, `shell-words`,
`downcast-rs` and `log`, all permissive.

`cargo-deny` is not installed on this machine and there is no root workspace yet, so `just deny`
cannot be the proof here. The inventory above is `cargo tree -e normal --format "{p} {l}"` read by
hand, as TASK-1.1 did, and TASK-1.5 owns the mechanical check.

## 13. What the protocol carries beyond terminals

Written by @bruk.o on 2026-11-15 from decisions Bruk made that day. It is a proposal for
TASK-1.10 (methods), TASK-1.11 (events) and TASK-1.12 (attach) to build from and correct, not a
result. It adds no frame kind and changes nothing in sections 1 to 7: everything here is control
JSON on stream 0, or a field on a message that already exists. Method and event names below are
placeholders; the real ones come from the API description (doc-4, R38).

### 13.1 Two ways a client relates to a session

doc-1 R86 says a client knows about every session on its server and views zero or more of them.
On the wire those are two different things, and section 1's attachments are the second.

- **Knowing about sessions** is a subscription on stream 0. A client asks for the tree and then
  receives events as it changes. This is what the app's bar and palette are drawn from, and it
  costs no terminal traffic.
- **Viewing** is holding attachments (section 3) to panes. A client views a session when it is
  attached to at least one of that session's panes. In M1 the app attaches to the panes of the tab
  it is showing and to nothing else, so a tab that is not on screen streams nothing.

So "which sessions does this client view" is never sent by the client; the server reads it off
the attachments.

### 13.2 The tree, and the focus path in it

The tree snapshot, returned by one request and kept current by events:

```
session  id, label, position, colour, current_tab
tab      id, label, title, position, current_pane, layout, shown_status
pane     id, label, title, cwd, program, exit_code, status
```

- **Focus is two fields:** a session's `current_tab` and a tab's `current_pane` (R85). Nothing
  else on the wire says "focused".
- A request to focus a pane sets the pane's tab's `current_pane` and, if that tab was not
  current, its session's `current_tab`, in one step on the server. One event reports the whole
  path, `{session, tab, pane}`, so no client ever sees the two halves apart.
- Focusing never touches attachments (R88).

### 13.3 Clients

- The hello (section 7) gains two keys, which is allowed because unknown keys are ignored: `kind`,
  one of `app`, `cli` or `plugin`, and an optional `label`. The server's reply gains `client`, the
  identifier it has given this connection.
- `client.list` returns each client's identifier, kind, label, the sessions it views (from its
  attachments), and when it was last active. Events report a client connecting, leaving, and
  starting or stopping viewing a session.
- **Asking a client to show a session** (doc-2, `session focus`) is the one message addressed to a
  single client. It is an event on stream 0 delivered only to that client, carrying the session.
  The server does not wait for it to be obeyed: which session a window shows is the app's (R87).
  The caller's request succeeds once the event is queued.

### 13.4 The size of a terminal

Section 5 left this open: sizes are carried per attachment, and which wins was undecided. R91
decides it.

- Each attachment still declares its own size, at attach and whenever the client's pane changes
  size. That is the size the terminal would have in that client.
- The server remembers, per session, the **latest client**: the last one to send input on an
  attachment in that session, or to declare a new size on one.
- A terminal takes the size the latest client has declared for it. If the latest client has no
  attachment to that pane, its size does not change.
- When a terminal's size changes, every attachment to it is told. The snapshot already carries a
  size (section 3); an update gains an optional `size` for the same purpose. A client draws at
  that size, top left, padded or cropped (R91).

### 13.5 Status

- A pane's `status` is one record or nothing: `kind`, `progress` where known, `since`, and
  `ends_at` where the kind has a lifetime (R83). In M1 the kinds are running, finished and failed,
  set by the server from the program's exit status (R82). M2 adds what the status protocol
  reports.
- **The server owns the lifetimes.** It clears a finished status after 10 minutes and a bell after
  30 seconds, and it clears finished, failed and the bell when input arrives for that pane. A
  click is not input, so the app sends one small request saying the person has dealt with that
  pane. Opening a tab sends nothing and clears nothing.
- **The server chooses what a tab shows.** A tab's `shown_status` names the pane whose status the
  tab's ring stands for, by the order in doc-7 D67. It is computed once, on the server, so the
  app, the CLI and a plugin cannot disagree. Promotion (R84, M2) is the server choosing
  differently; no client needs to know why.
- Status changes are events, each carrying the pane and its new record or nothing.

### 13.6 Pane layout

- A tab's `layout` is a tree: a node is either a pane, or a split with a direction, a ratio
  between 0 and 1, and two children. A tab also has `zoomed`, a pane or nothing, for fill.
- Ratios, not sizes, are what the server holds, which is why every client shares the proportions
  (R89) while each has its own pixels.
- Splitting, closing, resizing, swapping and filling are requests that change this tree, and each
  is reported as one event carrying the tab's new layout whole. A layout is small, and sending it
  whole means a client never has to apply a change to a tree it might have wrong.

### 13.7 What this leaves open

1. **The smallest pane.** doc-7 D47 gives it in pixels, which only the app knows. The server needs
   a floor of its own in rows and columns so that the CLI cannot split a pane to nothing. Not
   chosen here.
2. **Whether a request to deal with a pane is its own method or a flag on focusing it.** 13.5
   assumes its own.
3. **Section 10 is still unanswered:** that the answer to the spike's first criterion is a
   snapshot followed by screen updates. Everything in this section assumes it stands.
4. **Events addressed to one client** (13.3) are new. Every other event in this document goes to
   every subscriber.

### 13.8 Who inherits this

- **TASK-1.6**: the layout tree of 13.6, and the two focus fields of 13.2.
- **TASK-1.10**: the tree request, the focus requests, `client.list`, and the request of 13.5.
- **TASK-1.11**: the events of 13.2 to 13.6, including the one addressed to a single client.
- **TASK-1.12**: the latest client and the size rule of 13.4, and `size` on an update.
- **TASK-1.9**: the two new keys in the hello and the identifier in its reply.
