//! Runs `wrk --latency` and parses its text report, so no wrk Lua script is needed.

use std::process::{Command, Output};

use serde::{Deserialize, Serialize};

/// Latencies in microseconds.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Latency {
    pub mean_us: f64,
    pub p50_us: f64,
    pub p75_us: f64,
    pub p90_us: f64,
    pub p99_us: f64,
    pub max_us: f64,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Errors {
    pub connect: u64,
    pub read: u64,
    pub write: u64,
    pub timeout: u64,
    /// Responses outside 2xx/3xx.
    pub status: u64,
}

impl Errors {
    pub fn socket(&self) -> u64 {
        self.connect + self.read + self.write + self.timeout
    }

    pub fn total(&self) -> u64 {
        self.socket() + self.status
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct WrkResult {
    pub requests: u64,
    pub duration_s: f64,
    pub rps: f64,
    pub latency: Latency,
    pub errors: Errors,
}

pub struct WrkRun<'a> {
    pub url: &'a str,
    pub threads: usize,
    pub connections: u32,
    pub seconds: u32,
    /// CPU list for `taskset -c`; `None` runs wrk unpinned.
    pub cpus: Option<&'a str>,
}

pub fn run(run: &WrkRun) -> Result<WrkResult, String> {
    let mut command: Command = match run.cpus {
        Some(cpus) => {
            let mut command: Command = Command::new("taskset");
            command.args(["-c", cpus, "wrk"]);
            command
        }
        None => Command::new("wrk"),
    };

    command.args([
        "--latency".to_owned(),
        format!("-t{}", run.threads),
        format!("-c{}", run.connections),
        format!("-d{}s", run.seconds),
        run.url.to_owned(),
    ]);

    let output: Output = command.output().map_err(|e| format!("failed to run wrk: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "wrk exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    parse(&String::from_utf8_lossy(&output.stdout))
}

/// Parses the report printed by `wrk --latency`.
pub fn parse(report: &str) -> Result<WrkResult, String> {
    let mut result: WrkResult = WrkResult::default();
    let mut has_requests: bool = false;
    let mut has_rps: bool = false;

    for line in report.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();

        match fields.as_slice() {
            ["Latency", mean, _stdev, max, _within_stdev] => {
                result.latency.mean_us = time_us(mean)?;
                result.latency.max_us = time_us(max)?;
            }
            ["50%", value] => result.latency.p50_us = time_us(value)?,
            ["75%", value] => result.latency.p75_us = time_us(value)?,
            ["90%", value] => result.latency.p90_us = time_us(value)?,
            ["99%", value] => result.latency.p99_us = time_us(value)?,
            [count, "requests", "in", duration, ..] => {
                result.requests = number(count)?;
                result.duration_s = time_us(duration.trim_end_matches(','))? / 1_000_000.0;
                has_requests = true;
            }
            ["Socket", "errors:", pairs @ ..] => {
                for pair in pairs.chunks(2) {
                    let [name, value] = pair else {
                        return Err(format!("malformed socket errors line: {line:?}"));
                    };
                    let value: u64 = number(value.trim_end_matches(','))?;

                    match *name {
                        "connect" => result.errors.connect = value,
                        "read" => result.errors.read = value,
                        "write" => result.errors.write = value,
                        "timeout" => result.errors.timeout = value,
                        _ => return Err(format!("unknown socket error kind {name:?}")),
                    }
                }
            }
            ["Non-2xx", "or", "3xx", "responses:", count] => result.errors.status = number(count)?,
            ["Requests/sec:", rps] => {
                result.rps = rps.parse().map_err(|_| format!("invalid Requests/sec {rps:?}"))?;
                has_rps = true;
            }
            _ => {}
        }
    }

    if !(has_requests && has_rps) {
        return Err(format!("unexpected wrk output:\n{report}"));
    }

    Ok(result)
}

fn number(value: &str) -> Result<u64, String> {
    value.parse().map_err(|_| format!("invalid number {value:?}"))
}

/// Converts a wrk time such as `258.12us`, `2.65ms`, `2.10s` or `1.00m` to microseconds.
fn time_us(value: &str) -> Result<f64, String> {
    let split: usize = value
        .find(|c: char| c.is_ascii_alphabetic())
        .ok_or_else(|| format!("time without unit {value:?}"))?;

    let (amount, unit) = value.split_at(split);
    let amount: f64 = amount.parse().map_err(|_| format!("invalid time {value:?}"))?;

    let scale: f64 = match unit {
        "us" => 1.0,
        "ms" => 1_000.0,
        "s" => 1_000_000.0,
        "m" => 60_000_000.0,
        "h" => 3_600_000_000.0,
        _ => return Err(format!("unknown time unit in {value:?}")),
    };

    Ok(amount * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OK: &str = include_str!("../tests/fixtures/wrk-ok.txt");
    const ERRORS: &str = include_str!("../tests/fixtures/wrk-errors.txt");

    #[test]
    fn parses_successful_run() {
        let result: WrkResult = parse(OK).expect("valid report");

        assert_eq!(result.requests, 561_211);
        assert_eq!(result.duration_s, 2.1);
        assert_eq!(result.rps, 267_279.48);
        assert_eq!(result.latency.mean_us, 258.12);
        assert_eq!(result.latency.p50_us, 108.0);
        assert_eq!(result.latency.p99_us, 2_650.0);
        assert_eq!(result.latency.max_us, 19_220.0);
        assert_eq!(result.errors.total(), 0);
    }

    #[test]
    fn parses_status_and_socket_errors() {
        let result: WrkResult = parse(ERRORS).expect("valid report");

        assert_eq!(result.errors.status, 103_954);
        assert_eq!(result.errors.read, 103_937);
        assert_eq!(result.errors.socket(), 103_937);
        assert_eq!(result.latency.p99_us, 860.0);
    }

    #[test]
    fn rejects_unexpected_output() {
        assert!(parse("unable to connect to 127.0.0.1:3000 Connection refused").is_err());
    }

    #[test]
    fn converts_time_units() {
        assert_eq!(time_us("12.50us"), Ok(12.5));
        assert_eq!(time_us("2.65ms"), Ok(2_650.0));
        assert_eq!(time_us("1.00s"), Ok(1_000_000.0));
        assert_eq!(time_us("2.00m"), Ok(120_000_000.0));
        assert!(time_us("12.5").is_err());
        assert!(time_us("3.0xs").is_err());
    }
}
