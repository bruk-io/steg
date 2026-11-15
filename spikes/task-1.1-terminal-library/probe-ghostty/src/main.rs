//! Measure `libghostty-vt`. Every mode lives in `probe-core`, so both candidates are measured
//! by the same code.

mod adapter;

use adapter::Ghostty;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    probe_core::measure::run::<Ghostty>(probe_core::measure::parse(&args)?)
}
