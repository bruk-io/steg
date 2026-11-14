//! The spike's client, and the deterministic program the panes run.
//!
//! ```text
//! probe-client call     --socket S [--framing F] [--via CMD] [--protocol N] [--repeat N] METHOD [JSON]
//! probe-client attach   --socket S [--framing F] [--via CMD] --pane N --mode bytes|deltas|screen
//!                       [--cols C] [--rows R] [--duration-ms N] [--read-delay-us N]
//!                       [--input TEXT] [--ready-file P] [--dump P] [--refetch-scrollback]
//!                       [--quiet-ms N]
//! probe-client capture  --socket S [--framing F] --pane N [--out FILE] [--no-scrollback]
//! probe-client verify   --client FILE --server FILE --expect-last N [--gap-from K] [--gap-to R]
//! probe-client latency  --socket S [--framing F] --pane N --mode bytes|deltas [--samples N]
//! probe-client wait-quiet --socket S --pane N [--quiet-ms N] [--timeout-ms N]
//! probe-client bad-hello --socket S [--framing F] --what not-json|not-hello|huge|wrong-magic|wrong-framing
//! probe-client proxy    --socket S
//! probe-client emit     [--lines N] [--interval-us U] [--file PATH] [--wait]
//! probe-client fixture  --path P --mega-bytes N
//! ```
//!
//! `emit` is what the panes run: the same program every time, so a measurement is repeatable and
//! a gap in the output is visible by eye. `proxy` relays the protocol over a pipe pair, which is
//! how gate 2 is shown without an SSH connection.

mod capture;
mod grid;
mod replay;

use std::collections::HashMap;
use std::io::{BufRead as _, Read as _, Write as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use probe_wire::{
    AttachParams, AttachResult, Control, DataMode, Frame, FrameReader, FrameWriter, Framing, Kind,
    PROTOCOL_VERSION, Update, shake_hands,
};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio::net::UnixStream;

use crate::capture::percentiles;
use crate::grid::ClientGrid;
use crate::replay::Replay;

type Flags = HashMap<String, String>;

fn flags(args: &[String]) -> (Flags, Vec<String>) {
    let mut named = Flags::new();
    let mut positional = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if let Some(name) = arg.strip_prefix("--") {
            // A flag takes a value only when the next argument is not itself a flag. Anything
            // else and a switch such as `--refetch-scrollback` swallows the flag after it, which
            // is exactly what went wrong once already.
            let switch = args
                .get(index + 1)
                .is_none_or(|next| next.starts_with("--"));
            if switch {
                named.insert(name.to_owned(), "yes".to_owned());
                index += 1;
            } else {
                let value = args.get(index + 1).cloned().unwrap_or_default();
                named.insert(name.to_owned(), value);
                index += 2;
            }
        } else {
            positional.push(arg.clone());
            index += 1;
        }
    }
    (named, positional)
}

fn number<T: std::str::FromStr>(named: &Flags, name: &str, fallback: T) -> T {
    named
        .get(name)
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

struct Link {
    frames: FrameReader<Box<dyn AsyncRead + Unpin + Send>>,
    out: FrameWriter<Box<dyn AsyncWrite + Unpin + Send>>,
    child: Option<tokio::process::Child>,
    next_id: u64,
    /// Frames for a stream this client had not been told about yet. Counted and reported rather
    /// than quietly dropped: it must be zero, or the server broke the ordering rule that makes a
    /// reattach correct.
    unknown_stream_frames: u64,
}

async fn connect(named: &Flags) -> Result<Link, String> {
    let framing_name = named
        .get("framing")
        .cloned()
        .unwrap_or_else(|| "length".to_owned());
    let protocol = number(named, "protocol", PROTOCOL_VERSION);
    let read_framing = Framing::named(&framing_name)?;
    let write_framing = Framing::named(&framing_name)?;

    let (reader, writer, child): (
        Box<dyn AsyncRead + Unpin + Send>,
        Box<dyn AsyncWrite + Unpin + Send>,
        Option<tokio::process::Child>,
    ) = if let Some(command) = named.get("via") {
        // The protocol over a pipe pair through a relay it does not control: gate 2.
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|err| err.to_string())?;
        let stdin = child
            .stdin
            .take()
            .ok_or("the relay has no standard input")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("the relay has no standard output")?;
        (Box::new(stdout), Box::new(stdin), Some(child))
    } else {
        let path = named
            .get("socket")
            .ok_or("--socket PATH or --via COMMAND")?;
        let stream = UnixStream::connect(path)
            .await
            .map_err(|err| err.to_string())?;
        let (reader, writer) = stream.into_split();
        (Box::new(reader), Box::new(writer), None)
    };

    let mut frames = FrameReader::new(reader, read_framing);
    let mut out = FrameWriter::new(writer, write_framing);
    let hello = shake_hands(&mut frames, &mut out, "probe-client/0.0.0", protocol).await?;
    // The handshake, on stderr, so every recorded run shows which versions agreed (R39, AC #3).
    eprintln!(
        "handshake server={} protocol={} api={} framing={framing_name}",
        hello.server, hello.protocol, hello.api
    );
    Ok(Link {
        frames,
        out,
        child,
        next_id: 1,
        unknown_stream_frames: 0,
    })
}

impl Link {
    async fn request(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let request = Control::Request {
            id,
            method: method.to_owned(),
            params,
        };
        let payload = request.to_payload().map_err(|err| err.to_string())?;
        self.out
            .send(&Frame::control(probe_wire::CONTROL_STREAM, payload))
            .await
            .map_err(|err| err.to_string())?;
        loop {
            let frame = self
                .frames
                .next()
                .await
                .map_err(|err| err.to_string())?
                .ok_or("the server closed the connection")?;
            if frame.kind != Kind::Control || frame.stream != probe_wire::CONTROL_STREAM {
                self.unknown_stream_frames += 1;
                continue;
            }
            match Control::from_payload(&frame.payload) {
                Ok(Control::Response {
                    id: answered,
                    result,
                }) if answered == id => {
                    return Ok(result);
                }
                Ok(Control::Error {
                    id: answered,
                    code,
                    message,
                }) if answered == id => {
                    return Err(format!("{code}: {message}"));
                }
                _ => {}
            }
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().cloned() else {
        eprintln!("probe-client: a subcommand is needed; see the source's header");
        return ExitCode::from(2);
    };
    let (named, positional) = flags(&args[1..]);

    match command.as_str() {
        "emit" => emit(&named),
        "fixture" => fixture(&named),
        "verify" => verify(&named),
        _ => {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    eprintln!("probe-client: {err}");
                    return ExitCode::FAILURE;
                }
            };
            let outcome = runtime.block_on(async {
                match command.as_str() {
                    "call" => call(&named, &positional).await,
                    "capture" => capture_pane(&named).await,
                    "attach" => attach(&named).await,
                    "latency" => latency(&named).await,
                    "proxy" => proxy(&named).await,
                    "bad-hello" => bad_hello(&named).await,
                    "wait-quiet" => wait_quiet(&named).await,
                    other => Err(format!("unknown subcommand {other}")),
                }
            });
            match outcome {
                Ok(()) => ExitCode::SUCCESS,
                Err(why) => {
                    eprintln!("probe-client: {why}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}

async fn call(named: &Flags, positional: &[String]) -> Result<(), String> {
    let method = positional.first().ok_or("a method is needed")?;
    let params: serde_json::Value = match positional.get(1) {
        Some(text) => serde_json::from_str(text).map_err(|err| err.to_string())?,
        None => serde_json::json!({}),
    };
    let repeat: usize = number(named, "repeat", 1);
    let mut link = connect(named).await?;
    let mut samples = Vec::with_capacity(repeat);
    let mut last = serde_json::Value::Null;
    for _ in 0..repeat {
        let started = Instant::now();
        last = link.request(method, params.clone()).await?;
        samples.push(started.elapsed());
    }
    if repeat > 1 {
        println!("call {method} {}", percentiles(&mut samples));
    } else {
        println!("{last}");
    }
    if let Some(mut child) = link.child.take() {
        let _ = child.kill().await;
    }
    Ok(())
}

enum View {
    Bytes(Box<Replay>),
    Deltas(Box<ClientGrid>),
}

impl View {
    fn capture(&self, scrollback: bool) -> String {
        match self {
            Self::Bytes(replay) => replay.capture(scrollback),
            Self::Deltas(client) => client.capture(scrollback),
        }
    }

    fn screen_text(&self) -> String {
        match self {
            Self::Bytes(replay) => replay.screen_text(),
            Self::Deltas(client) => client.screen_text(),
        }
    }

    fn resyncs(&self) -> u64 {
        match self {
            Self::Bytes(replay) => replay.resyncs,
            Self::Deltas(client) => client.resyncs,
        }
    }

    fn seq(&self) -> u64 {
        match self {
            Self::Bytes(replay) => replay.seq,
            Self::Deltas(client) => client.seq,
        }
    }
}

async fn open_attachment(
    link: &mut Link,
    named: &Flags,
) -> Result<(AttachResult, DataMode, View), String> {
    let pane: u32 = number(named, "pane", 1);
    let mode = DataMode::named(named.get("mode").map_or("deltas", String::as_str))?;
    let params = AttachParams {
        pane,
        mode,
        cols: number(named, "cols", 80),
        rows: number(named, "rows", 24),
    };
    let value = serde_json::to_value(params).map_err(|err| err.to_string())?;
    let result = link.request("pane.attach", value).await?;
    let attached: AttachResult = serde_json::from_value(result).map_err(|err| err.to_string())?;
    let view = match mode {
        DataMode::Bytes => View::Bytes(Box::new(Replay::from_snapshot(&attached.snapshot))),
        DataMode::Deltas => View::Deltas(Box::new(ClientGrid::from_snapshot(
            &attached.snapshot,
            true,
        ))),
        DataMode::Screen => View::Deltas(Box::new(ClientGrid::from_snapshot(
            &attached.snapshot,
            false,
        ))),
    };
    Ok((attached, mode, view))
}

fn apply_frame(view: &mut View, frame: &Frame) {
    match (&mut *view, frame.kind) {
        (View::Bytes(replay), Kind::Data) => replay.feed(&frame.payload),
        (View::Bytes(replay), Kind::Control) => {
            if let Ok(update) = serde_json::from_slice::<Update>(&frame.payload) {
                replay.apply_update(&update);
            }
        }
        (View::Deltas(client), Kind::Control) => {
            if let Ok(update) = serde_json::from_slice::<Update>(&frame.payload) {
                client.apply_update(&update);
            }
        }
        (View::Deltas(_), Kind::Data) | (_, Kind::Hello) => {}
    }
}

async fn attach(named: &Flags) -> Result<(), String> {
    let mut link = connect(named).await?;
    let asked = Instant::now();
    let (attached, mode, mut view) = open_attachment(&mut link, named).await?;
    let attach_ms = asked.elapsed().as_secs_f64() * 1000.0;
    let snapshot_bytes = serde_json::to_vec(&attached.snapshot).map_or(0, |bytes| bytes.len());

    if let Some(text) = named.get("input") {
        let bytes = text.replace("\\n", "\n").into_bytes();
        link.out
            .send(&Frame::data(attached.stream, bytes))
            .await
            .map_err(|err| err.to_string())?;
    }

    println!(
        "ready pane={} attachment={} mode={} seq={} snapshot_bytes={} scrollback_rows={} attach_ms={attach_ms:.1}",
        number::<u32>(named, "pane", 1),
        attached.attachment,
        mode.name(),
        attached.snapshot.seq,
        snapshot_bytes,
        attached.snapshot.scrollback.len()
    );
    let _ = std::io::stdout().flush();
    if let Some(path) = named.get("ready-file") {
        std::fs::write(path, "ready\n").map_err(|err| err.to_string())?;
    }

    let duration = Duration::from_millis(number(named, "duration-ms", 2_000));
    let delay = Duration::from_micros(number(named, "read-delay-us", 0));
    // Stop once nothing has arrived for this long, so a run does not depend on guessing how long
    // the program in the pane will take. The duration stays as an upper bound.
    let quiet = Duration::from_millis(number(named, "quiet-ms", 0));
    let deadline = Instant::now() + duration;
    let mut last_frame = Instant::now();
    // A demonstration should not have to guess when the program in the pane has finished: it
    // waits for the server's own counters to settle and then asks this client to stop, which is
    // what SIGTERM means here. Everything after the loop still runs, so the dump is taken from a
    // client that was applying updates until the moment it was told to stop.
    let mut stop = Box::pin(async {
        if let Ok(mut term) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            term.recv().await;
        }
    });
    let mut frames = 0u64;
    let mut wire_bytes_at_start = link.frames.bytes_read();
    let mut why = "the duration ran out";
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        if !quiet.is_zero() && last_frame.elapsed() >= quiet {
            why = "the pane went quiet";
            break;
        }
        let wait = if quiet.is_zero() {
            left
        } else {
            left.min(quiet / 4)
        };
        let next = tokio::select! {
            () = &mut stop => {
                why = "it was told to stop";
                break;
            }
            answer = tokio::time::timeout(wait, link.frames.next()) => answer,
        };
        match next {
            Err(_) => continue,
            Ok(Ok(Some(frame))) => {
                last_frame = Instant::now();
                if frame.stream == attached.stream {
                    apply_frame(&mut view, &frame);
                    frames += 1;
                } else {
                    link.unknown_stream_frames += 1;
                }
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
            }
            Ok(Ok(None)) => {
                why = "the server closed the connection";
                break;
            }
            Ok(Err(err)) => return Err(err.to_string()),
        }
    }

    // In `screen` mode the stream carries no scrollback at all, so a client that wants it asks
    // for it in pages. This is the other half of that design, and the reattach demonstration
    // uses it to show that the scrollback comes back exactly.
    let mut fetched_rows = 0usize;
    if named.contains_key("refetch-scrollback")
        && let View::Deltas(client) = &mut view
    {
        let pane: u32 = number(named, "pane", 1);
        // Anchored on the client's own screen: `rows_ever` came with the last update it
        // applied, so the page it asks for is the scrollback that belongs to that screen.
        let ever = client.rows_ever;
        let want_from = ever.saturating_sub(capture::SCROLLBACK_LIMIT as u64);
        let mut rows: Vec<probe_wire::Row> = Vec::new();
        let page = 1_000u64;
        let mut from = want_from;
        let mut gap = 0u64;
        while from < ever {
            let answer = link
                .request(
                    "pane.scrollback",
                    serde_json::json!({"pane": pane, "from": from, "to": (from + page).min(ever)}),
                )
                .await?;
            let answer: probe_wire::ScrollbackResult =
                serde_json::from_value(answer).map_err(|err| err.to_string())?;
            if answer.first_row > from {
                // The server has already dropped those rows. Absolute ids are what make this
                // visible instead of silently stitching the wrong rows together.
                gap += answer.first_row - from;
                from = answer.first_row;
            }
            if answer.rows.is_empty() {
                break;
            }
            from += answer.rows.len() as u64;
            rows.extend(answer.rows);
        }
        fetched_rows = rows.len();
        if gap > 0 {
            println!("gap in the fetched scrollback: {gap} rows the server no longer holds");
        }
        client.scrollback = rows;
        client.track_scrollback = true;
    }

    if let Some(path) = named.get("dump") {
        std::fs::write(path, view.capture(true)).map_err(|err| err.to_string())?;
    }
    let read = link.frames.bytes_read()
        - std::mem::take(&mut wire_bytes_at_start).min(link.frames.bytes_read());
    println!(
        "done why={why:?} frames={frames} bytes_read={read} resyncs={} seq={} unknown_stream_frames={} fetched_scrollback_rows={fetched_rows}",
        view.resyncs(),
        view.seq(),
        link.unknown_stream_frames
    );
    if let Some(mut child) = link.child.take() {
        let _ = child.kill().await;
    }
    Ok(())
}

/// Byte into the pty, to that byte showing on this client's own copy of the screen. It stops at
/// the client's grid: the drawing half is TASK-1.1's measurement, and R19's 16 ms has to cover
/// both halves added together.
async fn latency(named: &Flags) -> Result<(), String> {
    let mut link = connect(named).await?;
    let (attached, mode, mut view) = open_attachment(&mut link, named).await?;
    let samples: usize = number(named, "samples", 200);
    let mut timings: Vec<Duration> = Vec::with_capacity(samples);
    let mut lost = 0u64;

    for index in 0..samples {
        let token = format!("z{index}z");
        let started = Instant::now();
        link.out
            .send(&Frame::data(
                attached.stream,
                format!("{token}\n").into_bytes(),
            ))
            .await
            .map_err(|err| err.to_string())?;
        let mut seen = false;
        while started.elapsed() < Duration::from_secs(2) {
            let left = Duration::from_secs(2) - started.elapsed();
            match tokio::time::timeout(left, link.frames.next()).await {
                Ok(Ok(Some(frame))) => {
                    if frame.stream == attached.stream {
                        apply_frame(&mut view, &frame);
                        if view.screen_text().contains(&token) {
                            timings.push(started.elapsed());
                            seen = true;
                            break;
                        }
                    }
                }
                Ok(Ok(None)) => break,
                Ok(Err(err)) => return Err(err.to_string()),
                Err(_) => break,
            }
        }
        if !seen {
            lost += 1;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    println!(
        "latency mode={} {} lost={lost}",
        mode.name(),
        percentiles(&mut timings)
    );
    if let Some(mut child) = link.child.take() {
        let _ = child.kill().await;
    }
    Ok(())
}

/// Relay the protocol between this process's standard streams and the server's socket, so a
/// client can speak it over two pipes and a process in the middle.
async fn proxy(named: &Flags) -> Result<(), String> {
    let path = named.get("socket").ok_or("--socket PATH")?;
    let stream = UnixStream::connect(path)
        .await
        .map_err(|err| err.to_string())?;
    let (mut from_server, mut to_server) = stream.into_split();
    let upstream = tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match stdin.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    if to_server.write_all(&buf[..read]).await.is_err() {
                        break;
                    }
                    if to_server.flush().await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    let downstream = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match from_server.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    if stdout.write_all(&buf[..read]).await.is_err() {
                        break;
                    }
                    if stdout.flush().await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    let _ = tokio::join!(upstream, downstream);
    Ok(())
}

/// The program the panes run. Deterministic: numbered lines at a fixed pace, so a gap or a repeat
/// in a reattached client's screen is visible, or a file printed as fast as the pty takes it.
fn emit(named: &Flags) -> ExitCode {
    if named.contains_key("wait") {
        let mut line = String::new();
        let _ = std::io::stdin().lock().read_line(&mut line);
    }
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    if let Some(path) = named.get("file") {
        let Ok(mut file) = std::fs::File::open(path) else {
            eprintln!("emit: cannot open {path}");
            return ExitCode::FAILURE;
        };
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match file.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    if out.write_all(&buf[..read]).is_err() {
                        break;
                    }
                }
            }
        }
        let _ = out.flush();
        return ExitCode::SUCCESS;
    }
    let lines: u32 = number(named, "lines", 1_000);
    let interval = Duration::from_micros(number(named, "interval-us", 0));
    for line in 1..=lines {
        if writeln!(out, "line {line:06}").is_err() {
            break;
        }
        if out.flush().is_err() {
            break;
        }
        if !interval.is_zero() {
            std::thread::sleep(interval);
        }
    }
    let _ = out.flush();
    ExitCode::SUCCESS
}

/// The flood fixture, written where a measurement can print it. Lines of 79 characters and a
/// newline, so a megabyte is a known number of lines.
fn fixture(named: &Flags) -> ExitCode {
    let Some(path) = named.get("path").map(PathBuf::from) else {
        eprintln!("fixture: --path PATH");
        return ExitCode::from(2);
    };
    let megabytes: usize = number(named, "mega-bytes", 100);
    let Ok(file) = std::fs::File::create(&path) else {
        eprintln!("fixture: cannot write {}", path.display());
        return ExitCode::FAILURE;
    };
    let mut out = std::io::BufWriter::with_capacity(1 << 20, file);
    let lines = megabytes * (1 << 20) / 80;
    for line in 0..lines {
        let filler = "fill";
        if writeln!(out, "{line:010} {filler} {:.<62}", "").is_err() {
            return ExitCode::FAILURE;
        }
    }
    if out.flush().is_err() {
        return ExitCode::FAILURE;
    }
    println!("{} lines={lines}", path.display());
    ExitCode::SUCCESS
}

/// The server's own view of a pane as text. Written to a file when asked, with the last numbered
/// line printed, which is what the reattach demonstration needs at each step.
async fn capture_pane(named: &Flags) -> Result<(), String> {
    let pane: u32 = number(named, "pane", 1);
    let scrollback = !named.contains_key("no-scrollback");
    let mut link = connect(named).await?;
    let result = link
        .request(
            "pane.capture",
            serde_json::json!({"pane": pane, "scrollback": scrollback}),
        )
        .await?;
    let text = result
        .get("text")
        .and_then(serde_json::Value::as_str)
        .ok_or("the capture had no text")?;
    if let Some(path) = named.get("out") {
        std::fs::write(path, text).map_err(|err| err.to_string())?;
    }
    let numbers = numbered_lines(text);
    println!(
        "capture pane={pane} bytes={} lines={} last={}",
        text.len(),
        text.lines().count(),
        numbers.last().map_or(0, |last| *last)
    );
    if let Some(mut child) = link.child.take() {
        let _ = child.kill().await;
    }
    Ok(())
}

/// Every `line NNNNNN` in a capture, in the order it appears.
fn numbered_lines(text: &str) -> Vec<u32> {
    text.lines()
        .filter_map(|line| line.strip_prefix("line "))
        .filter_map(|rest| rest.trim().parse::<u32>().ok())
        .collect()
}

/// The reattach oracle, as code rather than as a judgement: a reattached client's screen and
/// scrollback against the server's own, and the output printed while nothing was attached.
fn verify(named: &Flags) -> ExitCode {
    let Some(client_path) = named.get("client") else {
        eprintln!("verify: --client FILE");
        return ExitCode::from(2);
    };
    let Some(server_path) = named.get("server") else {
        eprintln!("verify: --server FILE");
        return ExitCode::from(2);
    };
    let (Ok(client), Ok(server)) = (
        std::fs::read_to_string(client_path),
        std::fs::read_to_string(server_path),
    ) else {
        eprintln!("verify: both files have to be readable");
        return ExitCode::FAILURE;
    };

    let mut findings: Vec<String> = Vec::new();

    if client == server {
        println!(
            "same  the reattached client and the server agree byte for byte ({} bytes, {} lines)",
            client.len(),
            client.lines().count()
        );
    } else {
        let mut first = None;
        for (index, (left, right)) in client.lines().zip(server.lines()).enumerate() {
            if left != right {
                first = Some((index + 1, left.to_owned(), right.to_owned()));
                break;
            }
        }
        match first {
            Some((line, left, right)) => findings.push(format!(
                "the client and the server differ first at line {line}: client {left:?} server {right:?}"
            )),
            None => findings.push(format!(
                "the client has {} lines and the server {} ({} against {} bytes)",
                client.lines().count(),
                server.lines().count(),
                client.len(),
                server.len()
            )),
        }
    }

    let numbers = numbered_lines(&client);
    let expect_numbered = named.contains_key("expect-last") || named.contains_key("gap-from");
    match numbers.first() {
        None if expect_numbered => {
            findings.push("the client's capture holds no numbered lines at all".to_owned());
        }
        None => {}
        Some(first) => {
            let mut broken = 0;
            for pair in numbers.windows(2) {
                if pair[1] != pair[0] + 1 {
                    if broken < 5 {
                        findings.push(format!(
                            "the numbered lines jump from {} to {}: output was lost or repeated",
                            pair[0], pair[1]
                        ));
                    }
                    broken += 1;
                }
            }
            if broken == 0 {
                println!(
                    "whole {} numbered lines run from {first} to {} with no gap and no repeat",
                    numbers.len(),
                    numbers.last().unwrap_or(first)
                );
            }
        }
    }

    let expect_last: u32 = number(named, "expect-last", 0);
    if expect_last > 0 {
        match numbers.last() {
            Some(last) if *last == expect_last => {
                println!("last  the last line is {last}, as the program printed");
            }
            Some(last) => findings.push(format!(
                "the last numbered line is {last}, not the expected {expect_last}"
            )),
            None => findings.push("no numbered lines, so no last line".to_owned()),
        }
    }

    let gap_from: u32 = number(named, "gap-from", 0);
    let gap_to: u32 = number(named, "gap-to", 0);
    if gap_from > 0 && gap_to >= gap_from {
        let present: Vec<u32> = (gap_from..=gap_to)
            .filter(|line| !numbers.contains(line))
            .collect();
        if present.is_empty() {
            println!(
                "gap   every line from {gap_from} to {gap_to}, printed while no client was attached, is there"
            );
        } else {
            findings.push(format!(
                "{} of the lines printed while no client was attached are missing, starting at {}",
                present.len(),
                present.first().copied().unwrap_or_default()
            ));
        }
    }

    if findings.is_empty() {
        println!("PASS");
        ExitCode::SUCCESS
    } else {
        for finding in &findings {
            println!("FAIL  {finding}");
        }
        ExitCode::FAILURE
    }
}

/// AC #3 at the wire level: a first frame the server cannot read must come back as an answer, not
/// as a hang, a panic, or a huge allocation. Written by hand rather than through `FrameWriter`,
/// because the point is to send what a correct client never would.
async fn bad_hello(named: &Flags) -> Result<(), String> {
    let path = named.get("socket").ok_or("--socket PATH")?;
    let what = named.get("what").map_or("not-json", String::as_str);
    let framing = named.get("framing").map_or("length", String::as_str);
    let mut stream = UnixStream::connect(path)
        .await
        .map_err(|err| err.to_string())?;

    let length_frame = |kind: u8, payload: &[u8]| -> Vec<u8> {
        let mut frame = Vec::with_capacity(payload.len() + 9);
        frame.extend_from_slice(&((payload.len() + 5) as u32).to_be_bytes());
        frame.extend_from_slice(&0u32.to_be_bytes());
        frame.push(kind);
        frame.extend_from_slice(payload);
        frame
    };
    let ndjson_line = |line: &str| -> Vec<u8> {
        let mut frame = line.as_bytes().to_vec();
        frame.push(b'\n');
        frame
    };

    let frame: Vec<u8> = match (framing, what) {
        ("length", "not-json") => length_frame(0, b"]["),
        ("length", "wrong-magic") => {
            length_frame(0, br#"{"steg":"ssh","protocol":1,"client":"x"}"#)
        }
        ("length", "not-hello") => length_frame(
            1,
            br#"{"type":"request","id":1,"method":"describe","params":{}}"#,
        ),
        ("length", "huge") => {
            // Four bytes asking for four gigabytes, and ten bytes to back it up.
            let mut frame = u32::MAX.to_be_bytes().to_vec();
            frame.extend_from_slice(b"not really");
            frame
        }
        ("length", "wrong-framing") => ndjson_line(
            r#"{"stream":0,"kind":"hello","json":{"steg":"steg","protocol":1,"client":"x"}}"#,
        ),
        ("ndjson", "not-json") => ndjson_line("]["),
        ("ndjson", "wrong-magic") => ndjson_line(
            r#"{"stream":0,"kind":"hello","json":{"steg":"ssh","protocol":1,"client":"x"}}"#,
        ),
        ("ndjson", "not-hello") => ndjson_line(
            r#"{"stream":0,"kind":"control","json":{"type":"request","id":1,"method":"describe","params":{}}}"#,
        ),
        ("ndjson", "huge") => {
            // A line with no newline in sight, past the frame cap: the delimiter framing has to
            // stop buffering rather than grow for ever.
            vec![b'x'; probe_wire::MAX_FRAME + 1024]
        }
        ("ndjson", "wrong-framing") => {
            length_frame(0, br#"{"steg":"steg","protocol":1,"client":"x"}"#)
        }
        (other, _) => return Err(format!("unknown framing {other}")),
    };

    stream
        .write_all(&frame)
        .await
        .map_err(|err| err.to_string())?;
    stream.flush().await.map_err(|err| err.to_string())?;

    let mut answer = vec![0u8; 4096];
    match tokio::time::timeout(Duration::from_secs(5), stream.read(&mut answer)).await {
        Err(_) => {
            // Not an error for the `wrong-framing` case, where it is the finding: a server that
            // waits for a delimiter cannot tell a client of another framing from a slow one.
            println!("{what}: the server neither answered nor closed within 5s");
            if what == "wrong-framing" {
                Ok(())
            } else {
                Err(format!(
                    "{what}: the server hung on a first frame it could not read"
                ))
            }
        }
        Ok(Err(err)) => Err(format!("{what}: {err}")),
        Ok(Ok(0)) => {
            println!("{what}: the server closed the connection without answering");
            Ok(())
        }
        Ok(Ok(read)) => {
            let text =
                String::from_utf8_lossy(&answer[..read]).replace(char::REPLACEMENT_CHARACTER, "");
            let message = text.find('{').map_or_else(
                || text.trim().to_owned(),
                |start| text[start..].trim().to_owned(),
            );
            println!("{what}: {message}");
            Ok(())
        }
    }
}

/// Wait until a pane stops producing output, and say how long that took. Used to time the flood
/// without guessing at a sleep: the server's own byte counter decides when it is over.
async fn wait_quiet(named: &Flags) -> Result<(), String> {
    let pane: u32 = number(named, "pane", 1);
    let quiet = Duration::from_millis(number(named, "quiet-ms", 500));
    let timeout = Duration::from_millis(number(named, "timeout-ms", 120_000));
    let mut link = connect(named).await?;
    let started = Instant::now();
    let mut last = 0u64;
    let mut unchanged_since = Instant::now();
    loop {
        let stats = link.request("server.stats", serde_json::json!({})).await?;
        let bytes = stats
            .get("panes")
            .and_then(serde_json::Value::as_array)
            .and_then(|panes| {
                panes.iter().find(|entry| {
                    entry.get("pane").and_then(serde_json::Value::as_u64) == Some(u64::from(pane))
                })
            })
            .and_then(|entry| entry.get("pty_bytes"))
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("no counters for pane {pane}"))?;
        if bytes == last {
            if bytes > 0 && unchanged_since.elapsed() >= quiet {
                println!(
                    "quiet pane={pane} pty_bytes={bytes} after={:.1}s mb_per_s={:.1}",
                    started.elapsed().as_secs_f64(),
                    (bytes as f64)
                        / (1024.0 * 1024.0)
                        / (started.elapsed().as_secs_f64() - quiet.as_secs_f64()).max(0.001)
                );
                break;
            }
        } else {
            last = bytes;
            unchanged_since = Instant::now();
        }
        if started.elapsed() > timeout {
            return Err(format!("pane {pane} was still printing after {timeout:?}"));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if let Some(mut child) = link.child.take() {
        let _ = child.kill().await;
    }
    Ok(())
}
