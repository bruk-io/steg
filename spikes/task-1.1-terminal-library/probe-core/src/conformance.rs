//! The R14 feature matrix, plus the two things steg needs beyond R14: an unknown OSC reaching
//! the application (OSC 7501, R49) and reflow on resize.
//!
//! Every check is bytes in, screen or reported fact back. Nothing inspects the emulator's
//! internals, which is the technique `.claude/rules/testing-rust.md` asks for.

use crate::backend::TerminalBackend;

/// What a check found.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The library does it and tells the application: steg wires it up and nothing more.
    Handled,
    /// The library does it wrongly, or disagrees with what the sequence asked for.
    Wrong,
    /// The library reports nothing, so steg has to implement this itself.
    NotReported,
    /// The library does it, but not the way the requirement is written, so steg has to bridge
    /// the difference.
    Differs,
}

impl Outcome {
    pub fn label(self) -> &'static str {
        match self {
            Self::Handled => "handled",
            Self::Wrong => "WRONG",
            Self::NotReported => "not reported",
            Self::Differs => "differs",
        }
    }
}

pub struct Check {
    pub feature: &'static str,
    pub requirement: &'static str,
    pub outcome: Outcome,
    pub detail: String,
}

const COLS: u16 = 80;
const ROWS: u16 = 10;
const SCROLLBACK: usize = 10_000;

/// Run every check against one candidate.
///
/// # Errors
/// If a terminal cannot be created at all.
pub fn run<B: TerminalBackend>() -> anyhow::Result<Vec<Check>> {
    Ok(vec![
        truecolour::<B>()?,
        wide_characters::<B>()?,
        alternate_screen::<B>()?,
        mouse_reporting::<B>()?,
        bracketed_paste::<B>()?,
        kitty_keyboard::<B>()?,
        osc_8_hyperlink::<B>()?,
        osc_52_clipboard::<B>()?,
        osc_7_working_directory::<B>()?,
        osc_133_prompt_marks::<B>()?,
        osc_7501_program_status::<B>()?,
        window_title::<B>()?,
        reflow_on_resize::<B>()?,
        scrollback_limit::<B>()?,
    ])
}

fn open<B: TerminalBackend>() -> anyhow::Result<B> {
    B::open(COLS, ROWS, SCROLLBACK)
}

fn check(
    feature: &'static str,
    requirement: &'static str,
    outcome: Outcome,
    detail: impl Into<String>,
) -> Check {
    Check {
        feature,
        requirement,
        outcome,
        detail: detail.into(),
    }
}

fn truecolour<B: TerminalBackend>() -> anyhow::Result<Check> {
    let mut term = open::<B>()?;
    term.feed(b"\x1b[38;2;255;100;0m\x1b[48;2;0;30;60mX\x1b[0m");
    let snapshot = term.snapshot();
    let cell = snapshot.rows[0].cells[0];
    let outcome = if cell.fg.r == 255 && cell.fg.g == 100 && cell.fg.b == 0 {
        Outcome::Handled
    } else {
        Outcome::Wrong
    };
    Ok(check(
        "truecolour (24-bit SGR)",
        "R14",
        outcome,
        format!(
            "fg #{:06x}, bg #{:06x} for SGR 38;2;255;100;0 on 48;2;0;30;60",
            cell.fg.hex(),
            cell.bg.hex()
        ),
    ))
}

fn wide_characters<B: TerminalBackend>() -> anyhow::Result<Check> {
    let mut term = open::<B>()?;
    term.feed("日本語abc".as_bytes());
    let snapshot = term.snapshot();
    let line = snapshot.lines().remove(0);
    let outcome = if snapshot.cursor_col == 9 && line.starts_with("日本語abc") {
        Outcome::Handled
    } else {
        Outcome::Wrong
    };
    Ok(check(
        "double-width characters",
        "R14",
        outcome,
        format!("cursor at column {} (9 expected), row reads {line:?}", snapshot.cursor_col),
    ))
}

fn alternate_screen<B: TerminalBackend>() -> anyhow::Result<Check> {
    let mut term = open::<B>()?;
    term.feed(b"primary\x1b[?1049h\x1b[Halternate");
    let in_alt = term.snapshot();
    let alt_flag = term.facts().alt_screen;
    term.feed(b"\x1b[?1049l");
    let back = term.snapshot();
    let outcome = match alt_flag {
        Some(true) if in_alt.contains("alternate") && back.contains("primary") => Outcome::Handled,
        Some(_) => Outcome::Wrong,
        None if in_alt.contains("alternate") && back.contains("primary") => Outcome::Handled,
        None => Outcome::Wrong,
    };
    Ok(check(
        "alternate screen (1049)",
        "R14",
        outcome,
        format!(
            "alt screen shows {:?}, primary restored to {:?}, flag reported: {}",
            in_alt.lines()[0],
            back.lines()[0],
            describe(alt_flag),
        ),
    ))
}

fn mouse_reporting<B: TerminalBackend>() -> anyhow::Result<Check> {
    let mut term = open::<B>()?;
    term.feed(b"\x1b[?1000h\x1b[?1006h");
    let reported = term.facts().mouse_tracking;
    Ok(check(
        "mouse reporting (1000/1006)",
        "R14",
        outcome_of(reported),
        format!("after enabling: {}", describe(reported)),
    ))
}

fn bracketed_paste<B: TerminalBackend>() -> anyhow::Result<Check> {
    let mut term = open::<B>()?;
    term.feed(b"\x1b[?2004h");
    let reported = term.facts().bracketed_paste;
    Ok(check(
        "bracketed paste (2004)",
        "R14",
        outcome_of(reported),
        format!("after enabling: {}", describe(reported)),
    ))
}

fn kitty_keyboard<B: TerminalBackend>() -> anyhow::Result<Check> {
    let mut term = open::<B>()?;
    term.feed(b"\x1b[>1u");
    let reported = term.facts().kitty_keyboard;
    Ok(check(
        "kitty keyboard protocol",
        "R14",
        outcome_of(reported),
        format!("after CSI > 1 u: {}", describe(reported)),
    ))
}

fn osc_8_hyperlink<B: TerminalBackend>() -> anyhow::Result<Check> {
    let mut term = open::<B>()?;
    term.feed(b"\x1b]8;;https://example.com/steg\x1b\\link\x1b]8;;\x1b\\");
    let facts = term.facts();
    let outcome = match &facts.hyperlink_at {
        Some(url) if url.contains("example.com/steg") => Outcome::Handled,
        Some(_) => Outcome::Wrong,
        None => Outcome::NotReported,
    };
    Ok(check(
        "OSC 8 hyperlinks",
        "R14, R17",
        outcome,
        format!("url on the first cell: {}", facts.hyperlink_at.unwrap_or_else(|| "none".into())),
    ))
}

fn osc_52_clipboard<B: TerminalBackend>() -> anyhow::Result<Check> {
    let mut term = open::<B>()?;
    term.feed(b"\x1b]52;c;c3RlZw==\x07");
    let facts = term.facts();
    let outcome = match &facts.clipboard_writes {
        Some(writes) if writes.iter().any(|w| w.contains("c3RlZw==") || w == "steg") => {
            Outcome::Handled
        }
        Some(_) => Outcome::Wrong,
        None => Outcome::NotReported,
    };
    Ok(check(
        "OSC 52 clipboard",
        "R14",
        outcome,
        format!("writes: {:?}", facts.clipboard_writes.unwrap_or_default()),
    ))
}

fn osc_7_working_directory<B: TerminalBackend>() -> anyhow::Result<Check> {
    let mut term = open::<B>()?;
    term.feed(b"\x1b]7;file://localhost/private/tmp/steg\x1b\\");
    let facts = term.facts();
    let outcome = match &facts.pwd {
        Some(pwd) if pwd.contains("/tmp/steg") => Outcome::Handled,
        Some(_) => Outcome::Wrong,
        None => Outcome::NotReported,
    };
    Ok(check(
        "OSC 7 working directory",
        "R14, R16",
        outcome,
        format!("pwd: {}", facts.pwd.unwrap_or_else(|| "not reported".into())),
    ))
}

fn osc_133_prompt_marks<B: TerminalBackend>() -> anyhow::Result<Check> {
    let mut term = open::<B>()?;
    term.feed(b"\x1b]133;A\x07$ \x1b]133;B\x07ls\x1b]133;C\x07\r\nout\r\n\x1b]133;D;0\x07");
    let facts = term.facts();
    Ok(check(
        "OSC 133 prompt marks",
        "R14, R18",
        outcome_of(facts.semantic_prompt),
        format!("prompt marks seen: {}", describe(facts.semantic_prompt)),
    ))
}

fn osc_7501_program_status<B: TerminalBackend>() -> anyhow::Result<Check> {
    let mut term = open::<B>()?;
    term.feed(b"\x1b]7501;id=build;state=blocked;title=waiting\x1b\\");
    let facts = term.facts();
    let outcome = match &facts.unknown_osc {
        Some(payload) if payload.contains("7501") => Outcome::Handled,
        Some(_) => Outcome::Wrong,
        None => Outcome::NotReported,
    };
    Ok(check(
        "unknown OSC reaches the app (OSC 7501)",
        "R49",
        outcome,
        format!(
            "payload handed over: {}",
            facts.unknown_osc.unwrap_or_else(|| "dropped by the library".into())
        ),
    ))
}

fn window_title<B: TerminalBackend>() -> anyhow::Result<Check> {
    let mut term = open::<B>()?;
    term.feed(b"\x1b]0;steg title\x07");
    let facts = term.facts();
    let outcome = match &facts.title {
        Some(title) if title == "steg title" => Outcome::Handled,
        Some(_) => Outcome::Wrong,
        None => Outcome::NotReported,
    };
    Ok(check(
        "window title (OSC 0/2)",
        "R16",
        outcome,
        format!("title: {}", facts.title.unwrap_or_else(|| "not reported".into())),
    ))
}

fn reflow_on_resize<B: TerminalBackend>() -> anyhow::Result<Check> {
    let mut term = open::<B>()?;
    let long: String = std::iter::repeat_n('x', 100).collect();
    term.feed(long.as_bytes());
    term.resize(120, ROWS);
    let snapshot = term.snapshot();
    let first = snapshot.lines().remove(0);
    let outcome = if first.len() == 100 {
        Outcome::Handled
    } else {
        Outcome::NotReported
    };
    Ok(check(
        "reflow on resize",
        "R8, R20",
        outcome,
        format!(
            "100 x's wrapped at 80 then widened to 120: first row holds {} of them",
            first.len()
        ),
    ))
}

fn scrollback_limit<B: TerminalBackend>() -> anyhow::Result<Check> {
    // Asked for in lines, as R15 is written. A library that counts bytes instead will hold more
    // than 50 rows of short lines, so the second phase checks that *something* bounds it.
    let mut term = B::open(COLS, ROWS, 50)?;
    for line in 0..200 {
        term.feed(format!("line {line}\r\n").as_bytes());
    }
    let after_200 = term.scrollback_rows();
    for line in 200..5_000 {
        term.feed(format!("line {line}\r\n").as_bytes());
    }
    let after_5000 = term.scrollback_rows();
    let (outcome, how) = if after_200 <= 50 {
        (Outcome::Handled, "pruned to the line count asked for, as R15 is written")
    } else if after_5000 < 4_000 {
        (
            Outcome::Differs,
            "bounded, but not by the line count: this library's limit is a byte estimate, so \
             steg must convert R15's 10,000 lines into bytes",
        )
    } else {
        (Outcome::Wrong, "not bounded at all")
    };
    Ok(check(
        "scrollback honours its limit",
        "R15",
        outcome,
        format!(
            "a 50-line limit: {after_200} rows held after 200 lines, {after_5000} after 5,000 - {how}"
        ),
    ))
}

fn outcome_of(reported: Option<bool>) -> Outcome {
    match reported {
        Some(true) => Outcome::Handled,
        Some(false) => Outcome::Wrong,
        None => Outcome::NotReported,
    }
}

fn describe(reported: Option<bool>) -> String {
    match reported {
        Some(value) => value.to_string(),
        None => "the library exposes nothing".to_owned(),
    }
}

/// The matrix as a Markdown table, ready for the board document.
pub fn table(name: &str, checks: &[Check]) -> String {
    let mut out = format!(
        "| feature | requirement | {name} | detail |\n|---|---|---|---|\n"
    );
    for row in checks {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            row.feature,
            row.requirement,
            row.outcome.label(),
            row.detail
        ));
    }
    out
}
