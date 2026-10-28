//! Result files and the Markdown tables printed at the end of a run or by `--compare`.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::environment::Environment;
use crate::wrk::WrkResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    pub target: String,
    pub scenario: String,
    pub connections: u32,
    /// Open issue that keeps the scenario from passing, when it was run with `--include-pending`.
    pub pending: Option<String>,
    pub result: WrkResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub environment: Environment,
    pub rows: Vec<Row>,
}

impl Report {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text: String = fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("invalid result file {}: {e}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json: String = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, json + "\n").map_err(|e| format!("cannot write {}: {e}", path.display()))
    }
}

pub fn print_table(rows: &[Row]) {
    println!("| target | scenario | conns | req/s | p50 (ms) | p99 (ms) | non-2xx | socket errors |");
    println!("|---|---|---:|---:|---:|---:|---:|---:|");

    for row in rows {
        let r: &WrkResult = &row.result;
        let scenario: String = match &row.pending {
            Some(issue) => format!("{} (pending {issue})", row.scenario),
            None => row.scenario.clone(),
        };

        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            row.target,
            scenario,
            row.connections,
            thousands(r.rps),
            ms(r.latency.p50_us),
            ms(r.latency.p99_us),
            r.errors.status,
            r.errors.socket()
        );
    }
}

/// Prints a side-by-side table and returns the largest absolute req/s difference, in percent.
pub fn compare(a: &Report, b: &Report) -> f64 {
    let before: HashMap<(&str, &str), &Row> = a
        .rows
        .iter()
        .map(|r| ((r.target.as_str(), r.scenario.as_str()), r))
        .collect();
    let mut worst: f64 = 0.0;

    println!("| target | scenario | req/s A | req/s B | diff | p99 A (ms) | p99 B (ms) |");
    println!("|---|---|---:|---:|---:|---:|---:|");

    for row in &b.rows {
        let Some(old) = before.get(&(row.target.as_str(), row.scenario.as_str())) else {
            continue;
        };

        let (old_rps, new_rps) = (old.result.rps, row.result.rps);
        let diff: f64 = if old_rps > 0.0 {
            (new_rps - old_rps) / old_rps * 100.0
        } else {
            0.0
        };
        worst = worst.max(diff.abs());

        println!(
            "| {} | {} | {} | {} | {diff:+.1}% | {} | {} |",
            row.target,
            row.scenario,
            thousands(old_rps),
            thousands(new_rps),
            ms(old.result.latency.p99_us),
            ms(row.result.latency.p99_us)
        );
    }

    worst
}

fn ms(microseconds: f64) -> String {
    format!("{:.2}", microseconds / 1_000.0)
}

fn thousands(value: f64) -> String {
    let digits: String = format!("{:.0}", value);
    let mut grouped: String = String::with_capacity(digits.len() + digits.len() / 3);

    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(c);
    }

    grouped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_thousands() {
        assert_eq!(thousands(0.0), "0");
        assert_eq!(thousands(999.4), "999");
        assert_eq!(thousands(1_000.0), "1,000");
        assert_eq!(thousands(267_279.48), "267,279");
        assert_eq!(thousands(1_234_567.0), "1,234,567");
    }
}
