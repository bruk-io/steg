//! Rendering a client's own screen and scrollback to text, which is what the reattach oracle
//! compares, and the percentile maths the measurements print.

use std::time::Duration;

/// R15's window: however much a client has, only the last 10,000 scrollback rows are what the
/// server promises to keep, so that is what a capture shows.
pub const SCROLLBACK_LIMIT: usize = 10_000;

/// One line per row, trailing blanks removed, blank rows kept.
pub fn lines(rows: impl Iterator<Item = String>) -> String {
    let mut out = String::new();
    for row in rows {
        out.push_str(row.trim_end());
        out.push('\n');
    }
    out
}

/// p50, p95, p99 and the largest, in milliseconds, from however many samples there are.
#[must_use]
pub fn percentiles(samples: &mut [Duration]) -> String {
    if samples.is_empty() {
        return "no samples".to_owned();
    }
    samples.sort_unstable();
    let at = |fraction: f64| -> f64 {
        let last = samples.len() - 1;
        let index = ((samples.len() as f64) * fraction).floor() as usize;
        samples[index.min(last)].as_secs_f64() * 1000.0
    };
    format!(
        "n={} p50={:.3}ms p95={:.3}ms p99={:.3}ms max={:.3}ms",
        samples.len(),
        at(0.50),
        at(0.95),
        at(0.99),
        samples
            .last()
            .map_or(0.0, |last| last.as_secs_f64() * 1000.0)
    )
}
