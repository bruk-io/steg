//! Measure `alacritty_terminal`. Every mode lives in `probe-core`, so both candidates are
//! measured by the same code.

mod adapter;

use adapter::Alacritty;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    probe_core::measure::run::<Alacritty>(probe_core::measure::parse(&args)?)
}
