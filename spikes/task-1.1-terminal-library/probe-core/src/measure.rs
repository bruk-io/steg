//! The measurements, and the command line that runs them. Both binaries call straight into
//! here, so neither candidate can be measured by code the other one is not.
//!
//! Every mode runs to completion, prints its table and quits by itself: `asdlc record-check`
//! runs these with nobody present to close a window.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{Sender, channel};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow};
use gpui::{
    AnyWindowHandle, App, AppContext as _, Application, Bounds, Context, Entity, IntoElement, ParentElement as _,
    Render, Styled as _, Window, WindowBounds, WindowOptions, div, prelude::FluentBuilder as _, px,
    rgb, size,
};
use portable_pty::CommandBuilder;

use crate::backend::{Snapshot, TerminalBackend};
use crate::conformance;
use crate::element::TerminalView;
use crate::flood::{self, FloodResult};
use crate::latency::{self, Plan, Probe};
use crate::pty::Pty;
use crate::report::{Percentiles, header, ms};

const COLS: u16 = 120;
const ROWS: u16 = 32;
const SCROLLBACK: usize = 10_000;

pub enum Command {
    /// Feed a few bytes in and print the screen back. The cheapest proof that a candidate works
    /// at all.
    Smoke,
    /// The R14 feature matrix.
    Conformance,
    /// The 100 MB print with no renderer, three times.
    Flood { megabytes: u64, runs: usize },
    /// Typing latency in a GPUI window, one pane.
    Latency { samples: usize },
    /// Typing in one pane while another prints the fixture: R19's second sentence.
    FloodUi { megabytes: u64, samples: usize },
    /// A live shell to look at, which closes itself.
    Window { seconds: u64 },
    /// The terminal owned by one thread, driven over a channel.
    Ownership,
    /// Run a real full-screen program on a pseudo-terminal, with no window, and print the
    /// screen it drew. Covers R14's risk with a program rather than with byte scripts.
    Program { command: Vec<String> },
}

/// Read the command line.
///
/// # Errors
/// If the mode is unknown or a number does not parse.
pub fn parse(args: &[String]) -> Result<Command> {
    let number = |index: usize, fallback: u64| -> Result<u64> {
        match args.get(index) {
            None => Ok(fallback),
            Some(text) => text
                .parse()
                .with_context(|| format!("{text:?} is not a number")),
        }
    };
    match args.first().map(String::as_str) {
        Some("smoke") | None => Ok(Command::Smoke),
        Some("conformance") => Ok(Command::Conformance),
        Some("flood") => Ok(Command::Flood {
            megabytes: number(1, 100)?,
            runs: usize::try_from(number(2, 3)?)?,
        }),
        Some("latency") => Ok(Command::Latency {
            samples: usize::try_from(number(1, 2000)?)?,
        }),
        Some("flood-ui") => Ok(Command::FloodUi {
            megabytes: number(1, 100)?,
            samples: usize::try_from(number(2, 300)?)?,
        }),
        Some("window") => Ok(Command::Window {
            seconds: number(1, 20)?,
        }),
        Some("ownership") => Ok(Command::Ownership),
        Some("program") => {
            let command = args[1..].to_vec();
            if command.is_empty() {
                return Err(anyhow!("program needs a command to run"));
            }
            Ok(Command::Program { command })
        }
        Some(other) => Err(anyhow!(
            "unknown mode {other:?}: smoke, conformance, flood, latency, flood-ui, window, ownership"
        )),
    }
}

/// Run one measurement against one candidate.
///
/// # Errors
/// If the measurement cannot be set up. A measurement that runs always prints its table.
pub fn run<B: TerminalBackend + 'static>(command: Command) -> Result<()> {
    header(B::name(), B::version());
    match command {
        Command::Smoke => smoke::<B>(),
        Command::Conformance => matrix::<B>(),
        Command::Flood { megabytes, runs } => print_flood::<B>(megabytes, runs),
        Command::Latency { samples } => latency_window::<B>(samples),
        Command::FloodUi { megabytes, samples } => flood_window::<B>(megabytes, samples),
        Command::Window { seconds } => live_window::<B>(seconds),
        Command::Ownership => ownership::<B>(),
        Command::Program { command } => program::<B>(&command),
    }
}

fn smoke<B: TerminalBackend>() -> Result<()> {
    let mut term = B::open(40, 4, 100)?;
    term.feed(b"hello from \x1b[38;2;255;120;0msteg\x1b[0m\r\nsecond line");
    let screen = term.snapshot();
    println!("### smoke");
    println!();
    for (index, line) in screen.lines().iter().enumerate() {
        println!("{index}: {line:?}");
    }
    println!(
        "cursor at column {}, row {}, visible {}",
        screen.cursor_col, screen.cursor_row, screen.cursor_visible
    );
    println!("dirty rows reported: {:?}", term.dirty_rows());
    Ok(())
}

fn matrix<B: TerminalBackend>() -> Result<()> {
    let checks = conformance::run::<B>()?;
    println!("### R14 feature matrix");
    println!();
    print!("{}", conformance::table(B::name(), &checks));
    println!();
    let not_reported = checks
        .iter()
        .filter(|check| check.outcome == conformance::Outcome::NotReported)
        .count();
    let wrong = checks
        .iter()
        .filter(|check| check.outcome == conformance::Outcome::Wrong)
        .count();
    let differs = checks
        .iter()
        .filter(|check| check.outcome == conformance::Outcome::Differs)
        .count();
    println!(
        "{} checks: {} handled, {differs} done differently, {not_reported} left to steg, {wrong} wrong",
        checks.len(),
        checks.len() - not_reported - wrong - differs
    );
    Ok(())
}

fn fixture_path(megabytes: u64) -> Result<PathBuf> {
    let folder = std::env::temp_dir();
    flood::fixture(&folder, megabytes * 1024 * 1024)
}

fn print_flood<B: TerminalBackend>(megabytes: u64, runs: usize) -> Result<()> {
    let path = fixture_path(megabytes)?;
    println!("### printing {megabytes} MB with no renderer");
    println!();
    println!("fixture: {}", path.display());
    println!();
    println!("{}", FloodResult::table_header());
    let mut results = Vec::new();
    for run in 1..=runs {
        let result = flood::feed_file::<B>(&path, COLS, ROWS, SCROLLBACK)?;
        println!("{}", result.row(&format!("run {run}")));
        results.push(result);
    }
    println!();
    if let Some(last) = results.last() {
        if let Some(chunks) = &last.chunk_percentiles {
            println!("{}", Percentiles::table_header("one 64 KB chunk"));
            println!("{}", chunks.row("feed"));
        }
        if let Some(snapshots) = &last.snapshot_percentiles {
            println!("{}", snapshots.row("read the screen back"));
        }
    }
    Ok(())
}

/// The window every GPUI measurement opens, sized to the grid.
fn window_options(cx: &mut App, cols: u16, rows: u16, panes: u16) -> WindowOptions {
    let width = px(f32::from(cols * panes) * 7.5 + 24.0);
    let height = px(f32::from(rows) * 17.0 + 24.0);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(width, height),
            cx,
        ))),
        ..Default::default()
    }
}

/// Two panes side by side, or one on its own.
struct Workbench<B: TerminalBackend + 'static> {
    left: Entity<TerminalView<B>>,
    right: Option<Entity<TerminalView<B>>>,
}

impl<B: TerminalBackend + 'static> Render for Workbench<B> {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_row()
            .bg(rgb(0x05_05_08))
            .gap(px(2.0))
            .child(div().flex_1().child(self.left.clone()))
            .when_some(self.right.clone(), |row, right| {
                row.child(div().flex_1().child(right))
            })
    }
}

/// Open the window, wire the panes up, and hand back what the caller needs to drive them.
fn open_panes<B: TerminalBackend + 'static>(
    cx: &mut App,
    left: CommandBuilder,
    right: Option<CommandBuilder>,
) -> Result<(
    AnyWindowHandle,
    Entity<TerminalView<B>>,
    Option<Entity<TerminalView<B>>>,
)> {
    let panes = if right.is_some() { 2 } else { 1 };
    let options = window_options(cx, COLS, ROWS, panes);
    let left_pty = Pty::spawn(left, COLS, ROWS)?;
    let right_pty = right.map(|command| Pty::spawn(command, COLS, ROWS)).transpose()?;
    let built: Rc<RefCell<Option<(Entity<TerminalView<B>>, Option<Entity<TerminalView<B>>>)>>> =
        Rc::new(RefCell::new(None));
    let handle = {
        let built = Rc::clone(&built);
        cx.open_window(options, move |window, cx| {
            let left_view = cx.new(|_| {
                TerminalView::<B>::new(left_pty, COLS, ROWS, SCROLLBACK, Probe::shared())
                    .expect("the emulator opens")
            });
            TerminalView::start(&left_view, window, cx);
            let right_view = right_pty.map(|pty| {
                let view = cx.new(|_| {
                    TerminalView::<B>::new(pty, COLS, ROWS, SCROLLBACK, Probe::shared())
                        .expect("the emulator opens")
                });
                TerminalView::start(&view, window, cx);
                view
            });
            *built.borrow_mut() = Some((left_view.clone(), right_view.clone()));
            cx.new(|_| Workbench {
                left: left_view,
                right: right_view,
            })
        })
        .context("open the window")?
    };
    let (left_view, right_view) = built
        .borrow_mut()
        .take()
        .context("the window builder ran")?;
    Ok((handle.into(), left_view, right_view))
}

fn latency_window<B: TerminalBackend + 'static>(samples: usize) -> Result<()> {
    let plan = Plan {
        samples,
        ..Plan::default()
    };
    Application::new().run(move |cx: &mut App| {
        let (handle, pane, _) = open_panes::<B>(cx, cat(), None).expect("the window opens");
        cx.activate(false);
        let weak = pane.downgrade();
        let probe = pane.read(cx).probe();
        handle
            .update(cx, move |_, window, cx| {
                window
                    .spawn(cx, async move |cx| {
                        // Let the program settle before the clock starts.
                        cx.background_executor()
                            .timer(Duration::from_millis(400))
                            .await;
                        match latency::drive(weak, cx, &plan).await {
                            Ok(blocks) => print_blocks(&blocks),
                            Err(error) => eprintln!("the driver stopped: {error}"),
                        }
                        print_latency("typing into one pane", &probe);
                        cx.update(|_window, cx| cx.quit()).ok();
                    })
                    .detach();
            })
            .ok();
    });
    Ok(())
}

fn flood_window<B: TerminalBackend + 'static>(megabytes: u64, samples: usize) -> Result<()> {
    let path = fixture_path(megabytes)?;
    let plan = Plan {
        samples,
        ..Plan::default()
    };
    println!("### typing in one pane while the other prints {megabytes} MB");
    println!();
    println!("fixture: {}", path.display());
    println!();
    let expected_bytes = megabytes * 1024 * 1024;
    Application::new().run(move |cx: &mut App| {
        let mut printer = CommandBuilder::new("/bin/cat");
        printer.arg(path.as_os_str().to_string_lossy().to_string());
        let (handle, flooding, typed) =
            open_panes::<B>(cx, printer, Some(cat())).expect("the window opens");
        cx.activate(false);
        let typed = typed.expect("two panes were asked for");
        let weak = typed.downgrade();
        let probe = typed.read(cx).probe();
        let flooding = flooding.downgrade();
        handle
            .update(cx, move |_, window, cx| {
                window
                    .spawn(cx, async move |cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(400))
                            .await;
                        let started = Instant::now();
                        // Watch the printing pane so the time a 100 MB print takes in the UI is
                        // a measurement and not an impression.
                        let flood_done: Rc<RefCell<Option<Duration>>> = Rc::new(RefCell::new(None));
                        {
                            let flooding = flooding.clone();
                            let flood_done = Rc::clone(&flood_done);
                            cx.spawn(async move |cx| {
                                let watching = Instant::now();
                                loop {
                                    cx.background_executor()
                                        .timer(Duration::from_millis(50))
                                        .await;
                                    let finished = flooding
                                        .update_in(cx, |pane, _window, _cx| {
                                            // The program exiting only means the pty was
                                            // drained; the pane has caught up when it has fed
                                            // every byte of the file to its emulator.
                                            pane.finished() && pane.fed().bytes >= expected_bytes
                                        })
                                        .unwrap_or(true);
                                    if finished {
                                        *flood_done.borrow_mut() = Some(watching.elapsed());
                                        return;
                                    }
                                }
                            })
                            .detach();
                        }
                        match latency::drive(weak, cx, &plan).await {
                            Ok(blocks) => print_blocks(&blocks),
                            Err(error) => eprintln!("the driver stopped: {error}"),
                        }
                        let typing_took = started.elapsed();
                        let finished = flooding
                            .update_in(cx, |pane, _window, _cx| pane.finished())
                            .unwrap_or(false);
                        print_latency("typing while the other pane floods", &probe);
                        println!();
                        println!(
                            "- the typing run took {:.1}s; the flooding pane had {} by then",
                            typing_took.as_secs_f64(),
                            if finished {
                                "finished"
                            } else {
                                "NOT finished"
                            }
                        );
                        match *flood_done.borrow() {
                            Some(took) => println!(
                                "- the pane fed the whole file to its emulator in {:.2}s ({:.0} MB/s through the UI)",
                                took.as_secs_f64(),
                                megabytes_of(expected_bytes) / took.as_secs_f64()
                            ),
                            None => println!(
                                "- the pane had NOT fed the whole file when the typing run ended"
                            ),
                        }
                        if let Ok(()) = flooding.update_in(cx, |pane, _window, _cx| {
                            let fed = pane.fed();
                            println!(
                                "- the flooding pane: {} pumps, {:.1} MB fed, largest pump {} KB, slowest pump {:.1} ms",
                                fed.pumps,
                                megabytes_of(fed.bytes),
                                fed.largest_pump_bytes / 1024,
                                ms(fed.slowest_pump),
                            );
                        }) {}
                        cx.update(|_window, cx| cx.quit()).ok();
                    })
                    .detach();
            })
            .ok();
    });
    Ok(())
}

fn live_window<B: TerminalBackend + 'static>(seconds: u64) -> Result<()> {
    println!("### a live shell, closing itself after {seconds}s");
    Application::new().run(move |cx: &mut App| {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_owned());
        let mut command = CommandBuilder::new(shell);
        command.env("TERM", "xterm-256color");
        let (handle, pane, _) = open_panes::<B>(cx, command, None).expect("the window opens");
        cx.activate(true);
        let weak = pane.downgrade();
        handle
            .update(cx, move |_, window, cx| {
                window
                    .spawn(cx, async move |cx| {
                        cx.background_executor()
                            .timer(Duration::from_secs(seconds))
                            .await;
                        if let Ok(screen) = weak.update_in(cx, |pane, _window, _cx| pane.screen()) {
                            print_screen(&screen);
                        }
                        cx.update(|_window, cx| cx.quit()).ok();
                    })
                    .detach();
            })
            .ok();
    });
    Ok(())
}

fn print_screen(screen: &Snapshot) {
    println!();
    for line in screen.lines() {
        if !line.is_empty() {
            println!("| {line}");
        }
    }
}

/// Each block of the run on its own line, so a number that drifts over a long run cannot hide
/// inside one percentile.
fn print_blocks(blocks: &[latency::Block]) {
    println!();
    println!("### the run, block by block");
    println!();
    println!("| samples | wall s | paint p50 ms | paint p95 ms | frame p50 ms | frame p95 ms | dropped so far |");
    println!("|---|---|---|---|---|---|---|");
    for block in blocks {
        let paint = Percentiles::of(&block.to_paint);
        let frame = Percentiles::of(&block.to_frame);
        let cell = |value: Option<&Percentiles>, pick: fn(&Percentiles) -> f64| {
            value.map_or_else(|| "-".to_owned(), |p| format!("{:.2}", pick(p)))
        };
        println!(
            "| {}..{} | {:.1} | {} | {} | {} | {} | {} |",
            block.first_sample,
            block.first_sample + block.to_paint.len(),
            block.wall.as_secs_f64(),
            cell(paint.as_ref(), |p| ms(p.p50)),
            cell(paint.as_ref(), |p| ms(p.p95)),
            cell(frame.as_ref(), |p| ms(p.p50)),
            cell(frame.as_ref(), |p| ms(p.p95)),
            block.dropped,
        );
    }
}

fn print_latency(label: &str, probe: &Rc<RefCell<Probe>>) {
    let state = probe.borrow();
    println!();
    println!("### {label}");
    println!();
    println!("{}", Percentiles::table_header("stop point"));
    if let Some(paint) = Percentiles::of(&state.to_paint) {
        println!("{}", paint.row("key to paint"));
    }
    if let Some(frame) = Percentiles::of(&state.to_frame) {
        println!("{}", frame.row("key to next frame"));
    }
    if let Some(intervals) = Percentiles::of(&state.frame_intervals) {
        println!("{}", intervals.row("interval between frames"));
    }
    println!();
    println!("- dropped samples (never drawn within the deadline): {}", state.dropped);
    if let Some(frame) = Percentiles::of(&state.to_frame) {
        let within = state
            .to_frame
            .iter()
            .filter(|sample| ms(**sample) <= 16.0)
            .count();
        #[expect(
            clippy::cast_precision_loss,
            reason = "a sample count in the thousands is exact in f64"
        )]
        let share = within as f64 * 100.0 / frame.count as f64;
        println!("- within R19's 16 ms, to the next frame: {within} of {} ({share:.1}%)", frame.count);
    }
}

/// Run a real program on a pseudo-terminal and print the screen it drew.
fn program<B: TerminalBackend>(command: &[String]) -> Result<()> {
    let mut term = B::open(COLS, ROWS, SCROLLBACK)?;
    let mut builder = CommandBuilder::new(&command[0]);
    for argument in &command[1..] {
        builder.arg(argument);
    }
    builder.env("TERM", "xterm-256color");
    builder.env("LANG", "en_US.UTF-8");
    let mut pty = Pty::spawn(builder, COLS, ROWS)?;

    // Give the program a moment to draw, feeding whatever it writes.
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut bytes = 0_usize;
    while Instant::now() < deadline {
        if let Some(chunk) = pty.next_chunk(Duration::from_millis(100)) {
            bytes += chunk.len();
            term.feed(&chunk);
        }
    }
    pty.kill();

    let screen = term.snapshot();
    println!("### {} on a pseudo-terminal", command.join(" "));
    println!();
    println!("{bytes} bytes of output, {}x{} screen", COLS, ROWS);
    println!();
    for line in screen.lines() {
        println!("| {line}");
    }
    println!();
    println!(
        "cursor at column {}, row {}",
        screen.cursor_col, screen.cursor_row
    );
    Ok(())
}

/// A byte count in megabytes.
#[expect(
    clippy::cast_precision_loss,
    reason = "a byte count this side of an exabyte is exact in f64"
)]
fn megabytes_of(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn cat() -> CommandBuilder {
    // `cat` and not a shell: the tty's own echo brings each keystroke straight back, with no
    // prompt redraw or completion machinery in the sample.
    let mut command = CommandBuilder::new("/bin/cat");
    command.env("TERM", "xterm-256color");
    command
}

/// Can the emulator live on a thread of its own, fed over a channel, as the server needs
/// (R1, and `.claude/rules/architecture.md`)?
enum ToTerminal {
    Feed(Vec<u8>),
    Screen(Sender<Vec<String>>),
    Stop,
}

fn ownership<B: TerminalBackend + 'static>() -> Result<()> {
    println!("### one terminal, one thread, driven over a channel");
    println!();
    let (to_terminal, inbox) = channel::<ToTerminal>();
    let worker = thread::Builder::new()
        .name("terminal".into())
        .spawn(move || -> Result<usize> {
            // The emulator is created here and never leaves: that is what a handle which is not
            // `Send` requires, and it is all it requires.
            let mut term = B::open(COLS, ROWS, SCROLLBACK)?;
            let mut fed = 0_usize;
            while let Ok(message) = inbox.recv() {
                match message {
                    ToTerminal::Feed(bytes) => {
                        fed += bytes.len();
                        term.feed(&bytes);
                    }
                    ToTerminal::Screen(reply) => {
                        let _ = reply.send(term.snapshot().lines());
                    }
                    ToTerminal::Stop => break,
                }
            }
            Ok(fed)
        })
        .context("start the terminal thread")?;

    to_terminal.send(ToTerminal::Feed(b"owned by its own thread\r\n".to_vec()))?;
    let (reply, answers) = channel();
    to_terminal.send(ToTerminal::Screen(reply))?;
    let lines = answers
        .recv_timeout(Duration::from_secs(5))
        .context("the terminal thread answered")?;
    to_terminal.send(ToTerminal::Stop)?;
    let fed = worker
        .join()
        .map_err(|_| anyhow!("the terminal thread panicked"))??;

    println!("- the first row read back from the thread: {:?}", lines.first());
    println!("- bytes fed over the channel: {fed}");
    println!(
        "- so the shape works for {}: the emulator is created on the thread that owns it, and \
         callers send messages (no Arc<Mutex<_>> across tasks)",
        B::name()
    );
    Ok(())
}
