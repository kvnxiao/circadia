//! Measure local solar-event calculation for explicitly supplied UTC intervals.

use anyhow::Context;
use circadia_solar_validation::Request;
use circadia_solar_validation::calculate;
use serde::Serialize;
use std::io::Read;

#[derive(Serialize)]
struct Measurement {
    first_micros: f64,
    samples_micros: Vec<f64>,
    rises: usize,
    sets: usize,
}

fn measure(request: &Request, iterations: usize) -> anyhow::Result<Measurement> {
    let first_start = std::time::Instant::now();
    let first = std::hint::black_box(calculate(std::hint::black_box(request))?);
    let first_micros = first_start.elapsed().as_secs_f64() * 1_000_000.0;
    let mut samples_micros = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = std::time::Instant::now();
        let result = std::hint::black_box(calculate(std::hint::black_box(request))?);
        samples_micros.push(start.elapsed().as_secs_f64() * 1_000_000.0);
        anyhow::ensure!(
            result.rises.len() == first.rises.len() && result.sets.len() == first.sets.len(),
            "repeated calculation changed its event count"
        );
    }
    Ok(Measurement {
        first_micros,
        samples_micros,
        rises: first.rises.len(),
        sets: first.sets.len(),
    })
}

fn main() -> anyhow::Result<()> {
    let iterations = std::env::args()
        .nth(1)
        .context("provide the number of repetitions")?
        .parse::<usize>()
        .context("parse repetition count")?;
    anyhow::ensure!(
        (1..=10_000).contains(&iterations),
        "repetitions must be between 1 and 10000"
    );
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("read solar requests from stdin")?;
    let requests: Vec<Request> =
        serde_json::from_str(&input).context("parse solar request array")?;
    let results: Vec<Measurement> = requests
        .iter()
        .map(|request| measure(request, iterations))
        .collect::<anyhow::Result<_>>()?;
    serde_json::to_writer(std::io::stdout().lock(), &results)
        .context("serialize timing measurements")?;
    Ok(())
}
