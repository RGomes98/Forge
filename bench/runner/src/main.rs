//! Forge benchmark runner.
//!
//! Builds the benchmark servers (`forge-server` and the axum reference in `axum-server`), starts
//! each one pinned to its own CPUs, waits for `/ping`, runs every scenario with `wrk` pinned to the
//! remaining CPUs and prints a Markdown summary. A non-2xx/3xx response or a socket error in any
//! scenario fails the run. Raw results are saved as JSON in `bench/results/`.
//!
//! ```text
//! cd bench
//! cargo run -p forge-bench --                        # full run: forge + axum, every ready scenario
//! cargo run -p forge-bench -- --quick                # short smoke run
//! cargo run -p forge-bench -- --compare A.json B.json --threshold 10
//! ```
//!
//! See docs/benchmarks.md.

mod environment;
mod report;
mod wrk;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::time::{Duration, Instant};
use std::{fs, thread};

use clap::Parser;

use crate::environment::{Environment, RunSettings};
use crate::report::{Report, Row};
use crate::wrk::{WrkResult, WrkRun};

struct Target {
    name: &'static str,
    binary: &'static str,
    /// Added to `--base-port`.
    port_offset: u16,
}

const TARGETS: [Target; 2] = [
    Target {
        name: "forge",
        binary: "forge-bench-server",
        port_offset: 0,
    },
    Target {
        name: "axum",
        binary: "axum-bench-server",
        port_offset: 1,
    },
];

struct Scenario {
    name: &'static str,
    path: &'static str,
    connections: u32,
    /// Open issue that keeps the scenario from passing. Pending scenarios only run with
    /// `--include-pending`, which reproduces the known failure.
    pending: Option<&'static str>,
}

const SCENARIOS: [Scenario; 6] = [
    Scenario {
        name: "ping-c1",
        path: "/ping",
        connections: 1,
        pending: None,
    },
    Scenario {
        name: "ping-c64",
        path: "/ping",
        connections: 64,
        pending: None,
    },
    Scenario {
        name: "ping-c512",
        path: "/ping",
        connections: 512,
        pending: None,
    },
    Scenario {
        name: "json-c64",
        path: "/json",
        connections: 64,
        pending: None,
    },
    Scenario {
        name: "param-c64",
        path: "/users/42",
        connections: 64,
        pending: None,
    },
    Scenario {
        name: "query-c64",
        path: "/ping?page=1",
        connections: 64,
        pending: Some("#10"),
    },
];

#[derive(Parser)]
#[command(about = "Runs the Forge HTTP benchmark suite with wrk (see docs/benchmarks.md)")]
struct Args {
    /// Seconds per measured run.
    #[arg(long, default_value_t = 10)]
    duration: u32,

    /// Seconds of discarded warm-up before each measured run.
    #[arg(long, default_value_t = 3)]
    warmup: u32,

    /// Short smoke run (3s per run, 1s of warm-up).
    #[arg(long)]
    quick: bool,

    /// Servers to measure.
    #[arg(long, value_delimiter = ',', default_value = "forge,axum")]
    targets: Vec<String>,

    /// Scenarios to run (default: every ready scenario).
    #[arg(long, value_delimiter = ',')]
    scenarios: Vec<String>,

    /// Also run scenarios blocked by open issues (they are expected to fail on Forge).
    #[arg(long)]
    include_pending: bool,

    /// CPUs for the server, in `taskset -c` syntax (default: first half of the CPUs).
    #[arg(long)]
    server_cpus: Option<String>,

    /// CPUs for wrk, in `taskset -c` syntax (default: second half of the CPUs).
    #[arg(long)]
    wrk_cpus: Option<String>,

    /// Port of the first server; the next ones use the following ports. Each must be free.
    #[arg(long, default_value_t = 18080)]
    base_port: u16,

    /// Skip `cargo build --release` of the servers.
    #[arg(long)]
    no_build: bool,

    /// Where to write the JSON results (default: bench/results/<date>-<commit>.json).
    #[arg(long)]
    output: Option<PathBuf>,

    /// Compare two result files instead of running.
    #[arg(long, num_args = 2, value_names = ["A", "B"])]
    compare: Vec<PathBuf>,

    /// Largest accepted req/s difference for --compare, in percent.
    #[arg(long, default_value_t = 10.0)]
    threshold: f64,
}

/// Kills the server when dropped, also on early returns and panics.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn main() -> ExitCode {
    let mut args: Args = Args::parse();

    let outcome: Result<bool, String> = if args.compare.is_empty() {
        if args.quick {
            (args.duration, args.warmup) = (3, 1);
        }
        run(&args)
    } else {
        compare(&args.compare[0], &args.compare[1], args.threshold)
    };

    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

fn bench_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("runner lives in bench/")
        .to_path_buf()
}

fn run(args: &Args) -> Result<bool, String> {
    let bench: PathBuf = bench_dir();
    let (default_server, default_wrk) = default_cpu_split();
    let server_cpus: String = args.server_cpus.clone().unwrap_or(default_server);
    let wrk_cpus: String = args.wrk_cpus.clone().unwrap_or(default_wrk);
    let pin: bool = Command::new("taskset").arg("-V").output().is_ok();

    if Command::new("wrk").arg("--version").output().is_err() {
        return Err("wrk is not installed".to_owned());
    }
    if !pin {
        eprintln!("warning: taskset not found; the server and wrk will share CPUs");
    }
    warn_on_low_fd_limit();

    let targets: Vec<&Target> = TARGETS
        .iter()
        .filter(|t| args.targets.iter().any(|n| n == t.name))
        .collect();
    let scenarios: Vec<&Scenario> = SCENARIOS
        .iter()
        .filter(|s| args.scenarios.is_empty() || args.scenarios.iter().any(|n| n == s.name))
        .filter(|s| s.pending.is_none() || args.include_pending || !args.scenarios.is_empty())
        .collect();

    if targets.is_empty() || scenarios.is_empty() {
        return Err("no target or scenario selected".to_owned());
    }

    if !args.no_build {
        println!("==> Building the benchmark servers (release)");
        let status = Command::new("cargo")
            .args([
                "build",
                "--release",
                "--locked",
                "-p",
                "forge-bench-server",
                "-p",
                "axum-bench-server",
            ])
            .current_dir(&bench)
            .status()
            .map_err(|e| format!("failed to run cargo: {e}"))?;
        if !status.success() {
            return Err("cargo build failed".to_owned());
        }
    }

    let environment: Environment = Environment::collect(
        bench.parent().unwrap_or(&bench),
        &RunSettings {
            server_cpus: &server_cpus,
            wrk_cpus: &wrk_cpus,
            duration_s: args.duration,
            warmup_s: args.warmup,
        },
    );
    println!("==> Environment");
    environment.print();

    let mut rows: Vec<Row> = Vec::new();
    for target in targets {
        rows.extend(run_target(target, &scenarios, args, &bench, &server_cpus, &wrk_cpus, pin)?);
    }

    println!();
    report::print_table(&rows);

    let results: PathBuf = bench.join("results");
    fs::create_dir_all(&results).map_err(|e| format!("cannot create {}: {e}", results.display()))?;
    let output: PathBuf = args.output.clone().unwrap_or_else(|| {
        let stamp: String = environment
            .date_utc
            .replace(['-', ':'], "")
            .replace('T', "-")
            .replace('Z', "");
        results.join(format!(
            "{stamp}-{}.json",
            environment.git_commit.as_deref().unwrap_or("unknown")
        ))
    });
    let report: Report = Report { environment, rows };
    report.save(&output)?;
    println!("\nResults saved to {}", output.display());

    let failed: Vec<&str> = report
        .rows
        .iter()
        .filter(|r| r.pending.is_none() && r.result.errors.total() > 0)
        .map(|r| r.scenario.as_str())
        .collect();
    if !failed.is_empty() {
        eprintln!("FAILED: errors in {}", failed.join(", "));
    }

    Ok(failed.is_empty())
}

fn run_target(
    target: &Target,
    scenarios: &[&Scenario],
    args: &Args,
    bench: &Path,
    server_cpus: &str,
    wrk_cpus: &str,
    pin: bool,
) -> Result<Vec<Row>, String> {
    let binary: PathBuf = bench.join("target").join("release").join(target.binary);
    let threads: usize = cpu_list(server_cpus)?.len();
    let port: u16 = args.base_port + target.port_offset;

    // Something else answering on the port would be measured instead of our server (for example a
    // container published on the same port), so refuse to start.
    if TcpStream::connect(("127.0.0.1", port)).is_ok() {
        return Err(format!(
            "port {port} is already in use by another process; choose another --base-port"
        ));
    }

    let mut command: Command = if pin {
        let mut command: Command = Command::new("taskset");
        command.args(["-c", server_cpus]).arg(&binary);
        command
    } else {
        Command::new(&binary)
    };

    println!("==> {}: starting {} on CPUs {server_cpus}", target.name, target.binary);
    let child: Child = command
        .env("BENCH_PORT", port.to_string())
        .env("BENCH_THREADS", threads.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to start {}: {e}", binary.display()))?;
    let mut server: Server = Server(child);

    wait_ready(port, &mut server.0, Duration::from_secs(15))?;

    let wrk_threads: usize = cpu_list(wrk_cpus)?.len();
    let mut rows: Vec<Row> = Vec::new();

    for scenario in scenarios {
        let url: String = format!("http://127.0.0.1:{port}{}", scenario.path);
        let mut wrk_run: WrkRun = WrkRun {
            url: &url,
            threads: wrk_threads.min(scenario.connections as usize).max(1),
            connections: scenario.connections,
            seconds: args.warmup,
            cpus: pin.then_some(wrk_cpus),
        };

        print!("    {:<16} ", scenario.name);
        std::io::stdout().flush().ok();

        if args.warmup > 0 {
            wrk::run(&wrk_run)?;
        }
        wrk_run.seconds = args.duration;
        let result: WrkResult = wrk::run(&wrk_run)?;

        // A crashed server would leave the numbers of a half-finished run behind.
        if let Ok(Some(status)) = server.0.try_wait() {
            return Err(format!("{} exited during {} with {status}", target.binary, scenario.name));
        }

        println!(
            "{:>10.0} req/s   p99 {:>7.2} ms   errors {}",
            result.rps,
            result.latency.p99_us / 1_000.0,
            result.errors.total()
        );

        rows.push(Row {
            target: target.name.to_owned(),
            scenario: scenario.name.to_owned(),
            connections: scenario.connections,
            pending: scenario.pending.map(str::to_owned),
            result,
        });
    }

    Ok(rows)
}

/// Polls `GET /ping` until it answers 200, the server exits, or the timeout expires.
fn wait_ready(port: u16, server: &mut Child, timeout: Duration) -> Result<(), String> {
    let deadline: Instant = Instant::now() + timeout;

    while Instant::now() < deadline {
        if let Ok(Some(status)) = server.try_wait() {
            return Err(format!("the server exited early with {status}"));
        }
        if ping(port) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }

    Err(format!(
        "the server on port {port} did not answer /ping within {}s",
        timeout.as_secs()
    ))
}

fn ping(port: u16) -> bool {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));

    let request: &[u8] = b"GET /ping HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
    let mut head: [u8; 12] = [0; 12];
    stream.write_all(request).is_ok() && stream.read_exact(&mut head).is_ok() && head.ends_with(b" 200")
}

fn compare(a: &Path, b: &Path, threshold: f64) -> Result<bool, String> {
    let (a, b) = (Report::load(a)?, Report::load(b)?);
    let worst: f64 = report::compare(&a, &b);
    println!("\nLargest req/s difference: {worst:.1}% (threshold {threshold:.1}%)");
    Ok(worst <= threshold)
}

/// First half of the CPUs for the server, second half for wrk.
fn default_cpu_split() -> (String, String) {
    let count: usize = thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    if count < 2 {
        return ("0".to_owned(), "0".to_owned());
    }
    let half: usize = count / 2;
    (range(0, half - 1), range(half, count - 1))
}

fn range(first: usize, last: usize) -> String {
    if first == last {
        first.to_string()
    } else {
        format!("{first}-{last}")
    }
}

/// Parses a `taskset -c` CPU list such as `0-2`, `3,5` or `0-1,4`.
fn cpu_list(spec: &str) -> Result<Vec<usize>, String> {
    let invalid = || format!("invalid CPU list {spec:?}");
    let mut cpus: Vec<usize> = Vec::new();

    for part in spec.split(',').filter(|p| !p.is_empty()) {
        match part.split_once('-') {
            Some((first, last)) => {
                let (first, last): (usize, usize) =
                    (first.parse().map_err(|_| invalid())?, last.parse().map_err(|_| invalid())?);
                if first > last {
                    return Err(invalid());
                }
                cpus.extend(first..=last);
            }
            None => cpus.push(part.parse().map_err(|_| invalid())?),
        }
    }

    if cpus.is_empty() { Err(invalid()) } else { Ok(cpus) }
}

/// 512 connections on each side need more descriptors than some shells allow by default.
fn warn_on_low_fd_limit() {
    let soft: Option<u64> = fs::read_to_string("/proc/self/limits").ok().and_then(|limits| {
        limits
            .lines()
            .find(|l| l.starts_with("Max open files"))
            .and_then(|l| l.split_whitespace().nth(3))
            .and_then(|v| v.parse().ok())
    });

    if let Some(soft) = soft.filter(|&s| s < 4_096) {
        eprintln!("warning: the open file limit is {soft}; raise it with `ulimit -n 65535` before running");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cpu_lists() {
        assert_eq!(cpu_list("0-2"), Ok(vec![0, 1, 2]));
        assert_eq!(cpu_list("3,5"), Ok(vec![3, 5]));
        assert_eq!(cpu_list("0-1,4"), Ok(vec![0, 1, 4]));
        assert!(cpu_list("2-1").is_err());
        assert!(cpu_list("").is_err());
        assert!(cpu_list("a").is_err());
    }

    #[test]
    fn formats_ranges() {
        assert_eq!(range(0, 0), "0");
        assert_eq!(range(3, 5), "3-5");
    }
}
