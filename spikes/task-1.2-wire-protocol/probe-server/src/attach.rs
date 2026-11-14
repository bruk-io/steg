//! One connection: the handshake, the control calls, and the writer that keeps a flood in one
//! pane from delaying a control call or another pane (topology T1, R19's second sentence).
//!
//! The writer is where T1 earns its place. Control frames are drained before any terminal data,
//! and terminal data is taken one frame at a time from each attachment in turn, so no attachment
//! can hold the connection. T2 (a separate data connection per attachment, opened with a ticket)
//! would get the same property from the operating system instead; the document weighs it against
//! what this measures.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Sender as ThreadSender;
use std::time::Duration;

use probe_wire::{
    AttachParams, AttachResult, CaptureParams, CaptureResult, Control, Frame, FrameReader,
    FrameWriter, Framing, Handshake, Kind, OpenParams, OpenResult, PROTOCOL_VERSION, ResizeParams,
    ScrollbackParams, ServerHello, Snapshot, Stats, check_hello,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{Notify, mpsc, oneshot};

use crate::pty;
use crate::term::{Counters, Overflow, TermMsg, TerminalThread};

pub struct Pane {
    pub tx: ThreadSender<TermMsg>,
    pub program: String,
    pub cols: u16,
    pub rows: u16,
}

pub struct Server {
    pub framing: String,
    pub overflow: Overflow,
    pub tick: Duration,
    pub queue_depth: usize,
    pub counters: Arc<Counters>,
    pub shutdown: Arc<Notify>,
    /// A plain mutex, held for a lookup and never across an `await`. The alternative, a task
    /// owning the pane table, would add a hop to every control call for no gain in a spike whose
    /// subject is the wire and not the kernel (`.claude/rules/rust.md`: the reason beside it).
    panes: std::sync::Mutex<HashMap<u32, Pane>>,
    next_pane: AtomicU32,
    next_attachment: AtomicU32,
}

impl Server {
    pub fn new(
        framing: String,
        overflow: Overflow,
        tick: Duration,
        queue_depth: usize,
    ) -> Arc<Self> {
        Arc::new(Self {
            framing,
            overflow,
            tick,
            queue_depth,
            counters: Arc::new(Counters::default()),
            shutdown: Arc::new(Notify::new()),
            panes: std::sync::Mutex::new(HashMap::new()),
            next_pane: AtomicU32::new(1),
            next_attachment: AtomicU32::new(1),
        })
    }

    /// Start a program in a new pane, each on its own thread with its own terminal.
    ///
    /// # Errors
    /// When the pty cannot be opened or the program cannot be run.
    pub fn open_pane(&self, params: &OpenParams) -> std::io::Result<u32> {
        let id = self.next_pane.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = std::sync::mpsc::channel();
        let pty = pty::open(
            &params.program,
            &params.args,
            params.cols,
            params.rows,
            tx.clone(),
        )?;
        let thread =
            TerminalThread::new(id, pty, params.cols, params.rows, self.tick, self.overflow);
        std::thread::Builder::new()
            .name(format!("terminal-{id}"))
            .spawn(move || thread.run(rx))?;
        let pane = Pane {
            tx,
            program: params.program.clone(),
            cols: params.cols,
            rows: params.rows,
        };
        if let Ok(mut panes) = self.panes.lock() {
            panes.insert(id, pane);
        }
        Ok(id)
    }

    fn pane_tx(&self, id: u32) -> Option<ThreadSender<TermMsg>> {
        self.panes.lock().ok()?.get(&id).map(|pane| pane.tx.clone())
    }

    fn pane_list(&self) -> Vec<serde_json::Value> {
        let Ok(panes) = self.panes.lock() else {
            return Vec::new();
        };
        let mut listed: Vec<(u32, serde_json::Value)> = panes
            .iter()
            .map(|(id, pane)| {
                (
                    *id,
                    serde_json::json!({
                        "pane": id,
                        "program": pane.program,
                        "cols": pane.cols,
                        "rows": pane.rows,
                    }),
                )
            })
            .collect();
        listed.sort_by_key(|(id, _)| *id);
        listed.into_iter().map(|(_, value)| value).collect()
    }

    fn pane_ids(&self) -> Vec<u32> {
        let Ok(panes) = self.panes.lock() else {
            return Vec::new();
        };
        let mut ids: Vec<u32> = panes.keys().copied().collect();
        ids.sort_unstable();
        ids
    }
}

/// The self-description. Minimal on purpose: its format is TASK-1.3's to decide, and this spike
/// only has to show that the framing carries it and that a Python client can read it (R42, R75).
fn description() -> serde_json::Value {
    serde_json::json!({
        "protocol": PROTOCOL_VERSION,
        "api": "0.1.0",
        "note": "shape owned by TASK-1.3; TASK-1.2 only carries it",
        "methods": [
            {"name": "describe", "summary": "Every method, its parameters and its documentation", "params": []},
            {"name": "pane.open", "summary": "Run a program in a new pane", "params": ["program", "args", "cols", "rows"]},
            {"name": "pane.list", "summary": "Every pane on this server", "params": []},
            {"name": "pane.capture", "summary": "A pane's screen, and its scrollback when asked", "params": ["pane", "scrollback"]},
            {"name": "pane.scrollback", "summary": "A page of a pane's scrollback rows", "params": ["pane", "from", "to"]},
            {"name": "pane.attach", "summary": "Subscribe to a pane: a snapshot now, then its stream", "params": ["pane", "mode", "cols", "rows"]},
            {"name": "pane.detach", "summary": "Stop an attachment", "params": ["attachment"]},
            {"name": "pane.input", "summary": "Send bytes to the program in a pane", "params": ["pane", "text"]},
            {"name": "pane.resize", "summary": "Resize a pane's terminal", "params": ["pane", "cols", "rows"]},
            {"name": "pane.attachments", "summary": "Every attachment on a pane and the size it asked for", "params": ["pane"]},
            {"name": "server.stats", "summary": "The server's own counters", "params": []},
            {"name": "server.shutdown", "summary": "Stop the server", "params": []}
        ]
    })
}

struct Outbound {
    control: mpsc::UnboundedSender<Frame>,
    notify: Arc<Notify>,
}

impl Outbound {
    fn send(&self, frame: Frame) {
        if self.control.send(frame).is_ok() {
            self.notify.notify_one();
        }
    }

    fn respond(&self, id: u64, result: serde_json::Value) {
        let control = Control::Response { id, result };
        if let Ok(payload) = control.to_payload() {
            self.send(Frame::control(probe_wire::CONTROL_STREAM, payload));
        }
    }

    fn fail(&self, id: u64, code: &str, message: String) {
        let control = Control::Error {
            id,
            code: code.to_owned(),
            message,
        };
        if let Ok(payload) = control.to_payload() {
            self.send(Frame::control(probe_wire::CONTROL_STREAM, payload));
        }
    }
}

/// Serve one connection to the end. The same function serves a Unix socket and a pipe pair, which
/// is gate 2: the protocol must survive a byte stream it does not control (R55, over SSH, later).
pub async fn serve<R, W>(server: Arc<Server>, reader: R, writer: W)
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let framing = match Framing::named(&server.framing) {
        Ok(framing) => framing,
        Err(_) => return,
    };
    let write_framing = match Framing::named(&server.framing) {
        Ok(framing) => framing,
        Err(_) => return,
    };
    let mut frames = FrameReader::new(reader, framing);
    let mut out = FrameWriter::new(writer, write_framing);

    // The handshake, before anything else is read, so a version mismatch is answered with a
    // message naming both versions rather than a parse error (R39).
    let first = match frames.next().await {
        Ok(Some(frame)) => frame,
        Ok(None) | Err(_) => return,
    };
    if first.kind != Kind::Hello {
        let refusal = probe_wire::Refusal::Malformed {
            reason: format!("the first frame is a {:?} frame, not a hello", first.kind),
            server: PROTOCOL_VERSION,
        };
        if let Ok(payload) = serde_json::to_vec(&refusal.to_wire()) {
            let _ = out.send(&Frame::hello(payload)).await;
        }
        return;
    }
    match check_hello(&first.payload, PROTOCOL_VERSION) {
        Handshake::Refuse(refusal) => {
            if let Ok(payload) = serde_json::to_vec(&refusal.to_wire()) {
                let _ = out.send(&Frame::hello(payload)).await;
            }
            return;
        }
        Handshake::Accept { .. } => {
            let hello = ServerHello {
                steg: probe_wire::STEG_MAGIC.to_owned(),
                protocol: PROTOCOL_VERSION,
                server: "probe-server/0.0.0".to_owned(),
                api: "0.1.0".to_owned(),
            };
            let Ok(payload) = serde_json::to_vec(&hello) else {
                return;
            };
            if out.send(&Frame::hello(payload)).await.is_err() {
                return;
            }
        }
    }

    let (control_tx, control_rx) = mpsc::unbounded_channel();
    let (newcomer_tx, newcomer_rx) = mpsc::unbounded_channel();
    let notify = Arc::new(Notify::new());
    let outbound = Outbound {
        control: control_tx,
        notify: Arc::clone(&notify),
    };
    let writer_task = tokio::spawn(write_loop(
        out,
        control_rx,
        newcomer_rx,
        Arc::clone(&notify),
        Arc::clone(&server.counters),
    ));

    // stream id -> (pane, attachment)
    let mut attachments: HashMap<u32, (u32, u32)> = HashMap::new();

    loop {
        let frame = match frames.next().await {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(_) => break,
        };
        match frame.kind {
            Kind::Hello => {
                outbound.fail(
                    0,
                    "unexpected-hello",
                    "the handshake already happened".to_owned(),
                );
            }
            Kind::Data => {
                // Input on an attachment's stream, on its way to the program.
                if let Some((pane, _)) = attachments.get(&frame.stream)
                    && let Some(tx) = server.pane_tx(*pane)
                {
                    let _ = tx.send(TermMsg::Input(frame.payload));
                }
            }
            Kind::Control => {
                let Ok(control) = Control::from_payload(&frame.payload) else {
                    continue;
                };
                let Control::Request { id, method, params } = control else {
                    continue;
                };
                handle_request(
                    &server,
                    &outbound,
                    &newcomer_tx,
                    &mut attachments,
                    id,
                    &method,
                    params,
                )
                .await;
            }
        }
    }

    // The client is gone, cleanly or killed. Every attachment it held goes with it, and the
    // terminals carry on: that is R2, and it is what the reattach demonstration relies on.
    for (pane, attachment) in attachments.values() {
        if let Some(tx) = server.pane_tx(*pane) {
            let _ = tx.send(TermMsg::Detach {
                attachment: *attachment,
            });
        }
    }
    writer_task.abort();
}

async fn handle_request(
    server: &Arc<Server>,
    outbound: &Outbound,
    newcomers: &mpsc::UnboundedSender<(u32, mpsc::Receiver<Frame>)>,
    attachments: &mut HashMap<u32, (u32, u32)>,
    id: u64,
    method: &str,
    params: serde_json::Value,
) {
    match method {
        "describe" => outbound.respond(id, description()),
        "pane.open" => match serde_json::from_value::<OpenParams>(params) {
            Ok(open) => match server.open_pane(&open) {
                Ok(pane) => {
                    let result = serde_json::to_value(OpenResult { pane }).unwrap_or_default();
                    outbound.respond(id, result);
                }
                Err(err) => outbound.fail(id, "pane-open-failed", err.to_string()),
            },
            Err(err) => outbound.fail(id, "bad-params", err.to_string()),
        },
        "pane.list" => outbound.respond(id, serde_json::json!({"panes": server.pane_list()})),
        "pane.capture" => match serde_json::from_value::<CaptureParams>(params) {
            Ok(capture) => {
                let Some(tx) = server.pane_tx(capture.pane) else {
                    outbound.fail(id, "no-such-pane", format!("pane {}", capture.pane));
                    return;
                };
                let (reply, answer) = oneshot::channel();
                if tx
                    .send(TermMsg::Capture {
                        scrollback: capture.scrollback,
                        reply,
                    })
                    .is_err()
                {
                    outbound.fail(id, "pane-gone", format!("pane {}", capture.pane));
                    return;
                }
                match answer.await {
                    Ok(text) => {
                        let result =
                            serde_json::to_value(CaptureResult { text }).unwrap_or_default();
                        outbound.respond(id, result);
                    }
                    Err(_) => outbound.fail(id, "pane-gone", format!("pane {}", capture.pane)),
                }
            }
            Err(err) => outbound.fail(id, "bad-params", err.to_string()),
        },
        "pane.scrollback" => match serde_json::from_value::<ScrollbackParams>(params) {
            Ok(request) => {
                let Some(tx) = server.pane_tx(request.pane) else {
                    outbound.fail(id, "no-such-pane", format!("pane {}", request.pane));
                    return;
                };
                let (reply, answer) = oneshot::channel();
                if tx
                    .send(TermMsg::Scrollback {
                        from: request.from,
                        to: request.to,
                        reply,
                    })
                    .is_err()
                {
                    outbound.fail(id, "pane-gone", format!("pane {}", request.pane));
                    return;
                }
                match answer.await {
                    Ok(page) => {
                        outbound.respond(id, serde_json::to_value(page).unwrap_or_default());
                    }
                    Err(_) => outbound.fail(id, "pane-gone", format!("pane {}", request.pane)),
                }
            }
            Err(err) => outbound.fail(id, "bad-params", err.to_string()),
        },
        "pane.attach" => match serde_json::from_value::<AttachParams>(params) {
            Ok(attach) => {
                let Some(tx) = server.pane_tx(attach.pane) else {
                    outbound.fail(id, "no-such-pane", format!("pane {}", attach.pane));
                    return;
                };
                let attachment = server.next_attachment.fetch_add(1, Ordering::Relaxed);
                let stream = attachment;
                let (queue_tx, queue_rx) = mpsc::channel(server.queue_depth);
                let (reply, answer) = oneshot::channel();
                let message = TermMsg::Attach {
                    attachment,
                    stream,
                    mode: attach.mode,
                    cols: attach.cols,
                    rows: attach.rows,
                    queue: queue_tx,
                    notify: Arc::clone(&outbound.notify),
                    reply,
                };
                if tx.send(message).is_err() {
                    outbound.fail(id, "pane-gone", format!("pane {}", attach.pane));
                    return;
                }
                let snapshot: Snapshot = match answer.await {
                    Ok(snapshot) => snapshot,
                    Err(_) => {
                        outbound.fail(id, "pane-gone", format!("pane {}", attach.pane));
                        return;
                    }
                };
                attachments.insert(stream, (attach.pane, attachment));
                let result = serde_json::to_value(AttachResult {
                    attachment,
                    stream,
                    snapshot,
                })
                .unwrap_or_default();
                // Order on the wire, not just at the terminal. The attach result carries the
                // snapshot on the control stream; the attachment's own stream carries what
                // follows it. The writer drains control before any attachment, so queueing the
                // response *before* registering the attachment's queue is what guarantees the
                // client cannot be handed output for a stream it has not been told about yet.
                // Found the hard way: with the two in the other order, a client lost the twelve
                // lines printed between the snapshot and its own first read.
                outbound.respond(id, result);
                let _ = newcomers.send((stream, queue_rx));
            }
            Err(err) => outbound.fail(id, "bad-params", err.to_string()),
        },
        "pane.detach" => {
            let attachment = params.get("attachment").and_then(serde_json::Value::as_u64);
            let Some(attachment) = attachment else {
                outbound.fail(id, "bad-params", "no attachment".to_owned());
                return;
            };
            let attachment = attachment as u32;
            let found: Vec<u32> = attachments
                .iter()
                .filter(|(_, (_, held))| *held == attachment)
                .map(|(stream, _)| *stream)
                .collect();
            for stream in found {
                if let Some((pane, held)) = attachments.remove(&stream)
                    && let Some(tx) = server.pane_tx(pane)
                {
                    let _ = tx.send(TermMsg::Detach { attachment: held });
                }
            }
            outbound.respond(id, serde_json::json!({"detached": attachment}));
        }
        "pane.input" => {
            let pane = params
                .get("pane")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as u32;
            let text = params
                .get("text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let Some(tx) = server.pane_tx(pane) else {
                outbound.fail(id, "no-such-pane", format!("pane {pane}"));
                return;
            };
            let _ = tx.send(TermMsg::Input(text.as_bytes().to_vec()));
            outbound.respond(id, serde_json::json!({"sent": text.len()}));
        }
        "pane.resize" => match serde_json::from_value::<ResizeParams>(params) {
            Ok(resize) => {
                let Some(tx) = server.pane_tx(resize.pane) else {
                    outbound.fail(id, "no-such-pane", format!("pane {}", resize.pane));
                    return;
                };
                let _ = tx.send(TermMsg::Resize {
                    cols: resize.cols,
                    rows: resize.rows,
                });
                outbound.respond(id, serde_json::json!({"pane": resize.pane}));
            }
            Err(err) => outbound.fail(id, "bad-params", err.to_string()),
        },
        "pane.attachments" => {
            let pane = params
                .get("pane")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as u32;
            let Some(tx) = server.pane_tx(pane) else {
                outbound.fail(id, "no-such-pane", format!("pane {pane}"));
                return;
            };
            let (reply, answer) = oneshot::channel();
            if tx.send(TermMsg::Attachments { reply }).is_err() {
                outbound.fail(id, "pane-gone", format!("pane {pane}"));
                return;
            }
            match answer.await {
                Ok(sizes) => {
                    let listed: Vec<serde_json::Value> = sizes
                        .into_iter()
                        .map(|attached| {
                            serde_json::json!({
                                "attachment": attached.attachment,
                                "cols": attached.cols,
                                "rows": attached.rows,
                                "mode": attached.mode,
                                "sent": attached.sent,
                                "dropped": attached.dropped,
                                "overflowed": attached.overflowed,
                            })
                        })
                        .collect();
                    outbound.respond(id, serde_json::json!({"attachments": listed}));
                }
                Err(_) => outbound.fail(id, "pane-gone", format!("pane {pane}")),
            }
        }
        "server.stats" => {
            let mut all: Vec<Stats> = Vec::new();
            for pane in server.pane_ids() {
                if let Some(tx) = server.pane_tx(pane) {
                    let (reply, answer) = oneshot::channel();
                    if tx.send(TermMsg::Stats { reply }).is_ok()
                        && let Ok(stats) = answer.await
                    {
                        all.push(stats);
                    }
                }
            }
            // The wire counters are the whole process's, not one pane's, so they are reported
            // beside the panes rather than inside each of them.
            outbound.respond(
                id,
                serde_json::json!({
                    "panes": all,
                    "wire_bytes": server.counters.wire_bytes.load(Ordering::Relaxed),
                    "wire_frames": server.counters.wire_frames.load(Ordering::Relaxed),
                }),
            );
        }
        "server.shutdown" => {
            outbound.respond(id, serde_json::json!({"stopping": true}));
            tokio::time::sleep(Duration::from_millis(50)).await;
            server.shutdown.notify_waiters();
        }
        other => outbound.fail(id, "no-such-method", format!("no method {other}")),
    }
}

/// Control first, then one frame from each attachment in turn. Nothing polls: the terminal
/// threads notify after queueing, so an idle connection costs nothing.
async fn write_loop<W>(
    mut out: FrameWriter<W>,
    mut control: mpsc::UnboundedReceiver<Frame>,
    mut newcomers: mpsc::UnboundedReceiver<(u32, mpsc::Receiver<Frame>)>,
    notify: Arc<Notify>,
    counters: Arc<Counters>,
) where
    W: AsyncWrite + Unpin + Send + 'static,
{
    let mut streams: Vec<(u32, mpsc::Receiver<Frame>)> = Vec::new();
    let mut accounted = 0u64;
    loop {
        while let Ok(newcomer) = newcomers.try_recv() {
            streams.push(newcomer);
        }

        let mut wrote = false;
        while let Ok(frame) = control.try_recv() {
            if out.send(&frame).await.is_err() {
                return;
            }
            wrote = true;
        }
        for (_, queue) in &mut streams {
            if let Ok(frame) = queue.try_recv() {
                if out.send(&frame).await.is_err() {
                    return;
                }
                wrote = true;
            }
        }

        let written = out.bytes_written();
        if written > accounted {
            counters
                .wire_bytes
                .fetch_add(written - accounted, Ordering::Relaxed);
            counters.wire_frames.fetch_add(1, Ordering::Relaxed);
            accounted = written;
        }

        if !wrote {
            tokio::select! {
                () = notify.notified() => {}
                () = tokio::time::sleep(Duration::from_millis(50)) => {}
            }
        }
    }
}
