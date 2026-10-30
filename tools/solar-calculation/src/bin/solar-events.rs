//! Calculate local solar-event fixtures for explicitly supplied UTC intervals.

use anyhow::Context;
use circadia_solar_validation::Request;
use circadia_solar_validation::Response;
use circadia_solar_validation::calculate;
use std::io::Read;

#[expect(
    clippy::print_stdout,
    reason = "Probe emits a JSON response for the integration harness"
)]
fn main() -> anyhow::Result<()> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("read solar requests from stdin")?;
    let requests: Vec<Request> =
        serde_json::from_str(&input).context("parse solar request array")?;
    let results: Vec<Response> = requests
        .iter()
        .map(calculate)
        .collect::<anyhow::Result<_>>()?;
    println!(
        "{}",
        serde_json::to_string(&results).context("serialize solar events")?
    );
    Ok(())
}
