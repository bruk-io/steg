//! Printing the numbers. Every table carries the machine, the versions and the build profile
//! above it, so a number can never be read out of the context it was measured in.

use std::process::Command;
use std::time::Duration;

/// The header printed above every measurement.
pub fn header(backend: &str, version: &str) {
    println!("## {backend} ({version})");
    println!();
    println!("- machine: {}", machine());
    println!("- display: {}", display());
    println!("- rustc: {}", one_line("rustc", &["--version"]));
    println!("- gpui: 0.2.2 (crates.io)");
    println!(
        "- build: {}",
        if cfg!(debug_assertions) {
            "DEBUG - these numbers mean nothing, rebuild with --release"
        } else {
            "release, opt-level 3, thin LTO"
        }
    );
    println!();
}

fn machine() -> String {
    let product = one_line("sw_vers", &["-productVersion"]);
    let chip = one_line("sysctl", &["-n", "machdep.cpu.brand_string"]);
    let cores = one_line("sysctl", &["-n", "hw.ncpu"]);
    format!("macOS {product}, {chip}, {cores} cores")
}

fn display() -> String {
    let raw = one_line_of(
        "system_profiler",
        &["SPDisplaysDataType"],
        |line| line.contains("UI Looks like") || line.contains("Hz"),
    );
    if raw.is_empty() {
        "unknown".to_owned()
    } else {
        raw.trim().to_owned()
    }
}

fn one_line(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|text| text.trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn one_line_of(program: &str, args: &[&str], wanted: impl Fn(&str) -> bool) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|text| {
            text.lines()
                .find(|line| wanted(line))
                .map(|line| line.trim().to_owned())
        })
        .unwrap_or_default()
}

/// The percentiles R19 is judged on, from samples that need not be sorted.
pub struct Percentiles {
    pub count: usize,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub max: Duration,
}

impl Percentiles {
    /// Compute the percentiles. Returns `None` for an empty sample, because a percentile of
    /// nothing would be a lie that later reads as a measurement.
    pub fn of(samples: &[Duration]) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        Some(Self {
            count: sorted.len(),
            p50: at(&sorted, 0.50),
            p95: at(&sorted, 0.95),
            p99: at(&sorted, 0.99),
            max: *sorted.last().expect("the sample is not empty"),
        })
    }

    /// One row of a latency table.
    pub fn row(&self, label: &str) -> String {
        format!(
            "| {label} | {} | {:.2} | {:.2} | {:.2} | {:.2} |",
            self.count,
            ms(self.p50),
            ms(self.p95),
            ms(self.p99),
            ms(self.max),
        )
    }

    /// The header the rows of [`Percentiles::row`] sit under.
    pub fn table_header(first_column: &str) -> String {
        format!(
            "| {first_column} | samples | p50 ms | p95 ms | p99 ms | max ms |\n|---|---|---|---|---|---|"
        )
    }
}

/// Nearest-rank percentile: the value at or above which that share of samples sits.
fn at(sorted: &[Duration], share: f64) -> Duration {
    let last = sorted.len() - 1;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a sample count that loses precision in f64 would take years to collect"
    )]
    let index = ((sorted.len() as f64 * share).ceil() as usize).saturating_sub(1);
    sorted[index.min(last)]
}

/// A duration in milliseconds, which is the unit R19 is written in.
pub fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
