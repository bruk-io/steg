//! The 100 MB print, measured with no renderer attached, which is where the two candidates
//! actually differ (R19's second sentence, and R15's scrollback limit).

use std::fs::File;
use std::io::{BufWriter, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};

use crate::backend::TerminalBackend;
use crate::report::{Percentiles, ms};

/// The size a pseudo-terminal read hands over at a time, so the emulator sees the same chunking
/// it would see from a real `cat`.
pub const CHUNK: usize = 64 * 1024;

pub struct FloodResult {
    pub bytes: u64,
    pub elapsed: Duration,
    /// The slowest single 64 KB chunk. On a UI thread this is the longest stall a frame would
    /// take, so it matters more than the average.
    pub slowest_chunk: Duration,
    pub chunk_percentiles: Option<Percentiles>,
    pub peak_rss_bytes: u64,
    pub scrollback_rows: usize,
    /// Reading the visible screen back once the scrollback is full: the renderer pays this every
    /// frame, so it belongs beside the parse speed.
    pub snapshot_percentiles: Option<Percentiles>,
}

impl FloodResult {
    pub fn megabytes_per_second(&self) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a byte count this side of an exabyte is exact in f64"
        )]
        let megabytes = self.bytes as f64 / (1024.0 * 1024.0);
        megabytes / self.elapsed.as_secs_f64()
    }

    pub fn row(&self, label: &str) -> String {
        #[expect(
            clippy::cast_precision_loss,
            reason = "resident size in bytes is exact in f64 far beyond any machine's memory"
        )]
        let peak_megabytes = self.peak_rss_bytes as f64 / (1024.0 * 1024.0);
        format!(
            "| {label} | {:.1} | {:.0} | {:.2} | {peak_megabytes:.1} | {} |",
            self.elapsed.as_secs_f64(),
            self.megabytes_per_second(),
            ms(self.slowest_chunk),
            self.scrollback_rows,
        )
    }

    pub fn table_header() -> &'static str {
        "| run | seconds | MB/s | slowest 64 KB chunk ms | peak RSS MB | scrollback rows |\n|---|---|---|---|---|---|"
    }
}

/// Write the fixture if it is not already there, and return its path.
///
/// The content is a deterministic mix: mostly plain 80-column lines, every tenth line coloured
/// with truecolour SGR, every hundredth carrying double-width text, which is what a build log or
/// a test run actually looks like.
///
/// # Errors
/// If the file cannot be written.
pub fn fixture(folder: &Path, bytes: u64) -> Result<PathBuf> {
    let path = folder.join(format!("steg-spike-flood-{bytes}.txt"));
    if let Ok(existing) = path.metadata() {
        if existing.len() == bytes {
            return Ok(path);
        }
    }
    let file = File::create(&path).with_context(|| format!("create {}", path.display()))?;
    let mut out = BufWriter::with_capacity(1 << 20, file);
    let mut written = 0_u64;
    let mut line = 0_u64;
    while written < bytes {
        let text = match line % 100 {
            0 => format!(
                "{line:>8} \x1b[38;2;240;120;60m日本語の行\x1b[0m padding padding padding padding\r\n"
            ),
            n if n % 10 == 0 => format!(
                "{line:>8} \x1b[38;2;120;200;255mcoloured\x1b[0m output line with some words in it\r\n"
            ),
            _ => format!("{line:>8} plain output line with a handful of words on it, padded ok\r\n"),
        };
        let take = usize::try_from((bytes - written).min(text.len() as u64))
            .expect("a chunk is at most one line long");
        out.write_all(&text.as_bytes()[..take])
            .context("write the fixture")?;
        written += take as u64;
        line += 1;
    }
    out.flush().context("flush the fixture")?;
    Ok(path)
}

/// Feed a file straight into the emulator, with no renderer, and measure it.
///
/// # Errors
/// If the terminal cannot be created or the fixture cannot be read.
pub fn feed_file<B: TerminalBackend>(
    path: &Path,
    cols: u16,
    rows: u16,
    scrollback: usize,
) -> Result<FloodResult> {
    let mut term = B::open(cols, rows, scrollback)?;
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut buffer = vec![0_u8; CHUNK];
    let mut chunks = Vec::new();
    let mut bytes = 0_u64;

    let started = Instant::now();
    loop {
        let count = file.read(&mut buffer).context("read the fixture")?;
        if count == 0 {
            break;
        }
        let chunk_started = Instant::now();
        term.feed(&buffer[..count]);
        chunks.push(chunk_started.elapsed());
        bytes += count as u64;
    }
    let elapsed = started.elapsed();

    let mut snapshots = Vec::with_capacity(200);
    for _ in 0..200 {
        let snapshot_started = Instant::now();
        let taken = term.snapshot();
        snapshots.push(snapshot_started.elapsed());
        // Keep the read honest: a backend that returned an empty screen would look fastest.
        assert!(!taken.rows.is_empty(), "a snapshot has rows");
    }

    Ok(FloodResult {
        bytes,
        elapsed,
        slowest_chunk: chunks.iter().copied().max().unwrap_or_default(),
        chunk_percentiles: Percentiles::of(&chunks),
        peak_rss_bytes: peak_rss_bytes(),
        scrollback_rows: term.scrollback_rows(),
        snapshot_percentiles: Percentiles::of(&snapshots),
    })
}

/// This process's peak resident size. On macOS `ru_maxrss` is in bytes, unlike Linux.
pub fn peak_rss_bytes() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: getrusage writes a complete `rusage` into the pointer it is given, and RUSAGE_SELF
    // is always a valid target, so the value is initialised when it returns 0.
    let filled = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } == 0;
    if filled {
        // SAFETY: getrusage returned 0, so the struct is initialised.
        u64::try_from(unsafe { usage.assume_init() }.ru_maxrss).unwrap_or(0)
    } else {
        0
    }
}
