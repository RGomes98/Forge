# Benchmarks

Forge is measured with a local, reproducible harness that lives in [`bench/`](../bench). It runs the same
HTTP scenarios against a minimal Forge server and an [axum](https://github.com/tokio-rs/axum) reference
server, with the load generator and the server pinned to separate CPUs, and fails if any response is not
2xx/3xx or any socket error happens.

This harness is meant for controlled measurements on a local machine. The CI benchmark job (a single
`wrk` run against `/ping` on a shared runner) is noisy by nature and only tracks a coarse trend.

## Layout

| Path | What it is |
|---|---|
| `bench/forge-server` | Minimal Forge server: `/ping`, `/json`, `/users/:id`. No database, no state. |
| `bench/axum-server` | Reference server with the same routes (axum on a multi-threaded tokio runtime). |
| `bench/runner` | The runner (`forge-bench`): builds, starts, measures, reports and compares. |
| `bench/results/` | Raw JSON results of local runs (git-ignored). |

`bench/` is a standalone Cargo workspace, so the reference server and its dependencies never enter the
framework's build, CI or dependency audit. Both servers use mimalloc and the same release profile as the
root workspace (`opt-level=3`, `lto="fat"`, `codegen-units=1`, `panic="abort"`).

## Requirements

* Linux with [`wrk`](https://github.com/wg/wrk) (`apt install wrk`).
* `taskset` (util-linux) to pin CPUs. Without it the runner warns and the server and `wrk` share CPUs.
* An open file limit of at least 4096 (`ulimit -n`); the runner warns when it is lower.

## Running

```sh
cd bench
cargo run -p forge-bench --                  # full run: both servers, every ready scenario
cargo run -p forge-bench -- --quick          # smoke run: 3s per scenario, 1s of warm-up
cargo run -p forge-bench -- --targets forge --scenarios ping-c64,json-c64
cargo run -p forge-bench -- --compare results/A.json results/B.json --threshold 10
```

| Option | Default | Meaning |
|---|---|---|
| `--duration` | `10` | Seconds per measured run. |
| `--warmup` | `3` | Seconds of discarded warm-up before each measured run. |
| `--quick` | | Shortcut for `--duration 3 --warmup 1`. |
| `--targets` | `forge,axum` | Servers to measure. |
| `--scenarios` | every ready one | Scenarios to run (naming one also runs it when pending). |
| `--include-pending` | | Also run scenarios blocked by open issues; their errors do not fail the run. |
| `--server-cpus` / `--wrk-cpus` | first / second half | CPU lists in `taskset -c` syntax. |
| `--base-port` | `18080` | Port of the first server; the next one uses the following port. |
| `--no-build` | | Skip `cargo build --release` of the servers. |
| `--output` | `results/<date>-<commit>.json` | Where to write the JSON results. |
| `--compare A B` | | Compare two result files instead of running. |
| `--threshold` | `10` | Largest accepted req/s difference for `--compare`, in percent. |

Each server gets as many threads as it has pinned CPUs. Before starting a server the runner checks that
nothing else answers on its port (a container published on the same port would otherwise be measured
instead), and after every scenario it checks that the server is still running.

The run exits with status 1 if a ready scenario had any non-2xx/3xx response or socket error, and with
status 2 on setup errors (missing `wrk`, busy port, build failure).

## Scenarios

| Scenario | Request | Connections | Status |
|---|---|---:|---|
| `ping-c1` | `GET /ping` | 1 | ready |
| `ping-c64` | `GET /ping` | 64 | ready |
| `ping-c512` | `GET /ping` | 512 | ready |
| `json-c64` | `GET /json` (serializes a struct) | 64 | ready |
| `param-c64` | `GET /users/42` (path parameter) | 64 | ready |
| `query-c64` | `GET /ping?page=1` | 64 | pending #10: the query string breaks routing (404) |

`wrk` threads are `min(wrk CPUs, connections)`. Results come from parsing the report of `wrk --latency`, so no
`wrk` script is involved. Scenarios that need one are planned for when the server supports them:

* `POST` with a body, once request bodies are read (#15).
* HTTP/1.1 pipelining, once pipelined requests are no longer lost (#9).

## Reading and comparing results

* Use **req/s** to compare runs. Tail latency (p99) on a non-isolated machine moves a lot between runs
  (up to 3x in the runs below) and is only indicative.
* `--compare` prints both runs side by side and fails when the largest req/s difference exceeds
  `--threshold`. **10%** is the default: two consecutive full runs on the reference machine differed by
  at most 8.9%, on a desktop with other work going on.
* To reduce noise: close other workloads, set the `performance` governor, disable turbo, or isolate the
  benchmark CPUs (`isolcpus`). The environment block of every result records governor, turbo and load
  average, so differences in conditions are visible.

Fairness notes: both servers enable `TCP_NODELAY` and run with the same number of threads. axum (hyper)
adds a `Date` header and `charset=utf-8` to `text/plain`, which Forge does not send yet (#36), so its
responses are slightly larger.

## Profiling (flamegraph)

The release profile strips symbols, so build the server with debug info, then record it while the runner
(or `wrk`) applies load:

```sh
cd bench
CARGO_PROFILE_RELEASE_STRIP=false CARGO_PROFILE_RELEASE_DEBUG=true \
  cargo build --release -p forge-bench-server
BENCH_PORT=18080 BENCH_THREADS=3 taskset -c 0-2 ./target/release/forge-bench-server &
taskset -c 3-5 wrk -t3 -c64 -d30s http://127.0.0.1:18080/ping &
perf record -F 999 -g -p "$(pgrep forge-bench-ser)" -- sleep 20
perf script | inferno-collapse-perf | inferno-flamegraph > flamegraph.svg   # cargo install inferno
```

`perf` needs `kernel.perf_event_paranoid` at 1 or lower (`sudo sysctl kernel.perf_event_paranoid=1`) or root.

## Results: `main` at 281c785

Two consecutive full runs (`--duration 10 --warmup 3`) on 2026-10-28:

| Item | Value |
|---|---|
| CPU | Intel Core i5-9400F, 6 cores (no SMT), governor `powersave`, turbo on |
| Kernel | 7.0.0-38-generic |
| Toolchain | rustc 1.99.1, wrk 4.1.0 |
| Pinning | server on CPUs 0-2 (3 threads), wrk on CPUs 3-5 |
| Load average | 2.5 (run A) and 4.8 (run B): a desktop in use, not an isolated machine |

| target | scenario | req/s (A) | req/s (B) | p50 A (ms) | p99 A (ms) | p99 B (ms) | errors |
|---|---|---:|---:|---:|---:|---:|---:|
| forge | ping-c1 | 38,975 | 39,226 | 0.02 | 0.07 | 0.10 | 0 |
| forge | ping-c64 | 288,128 | 276,248 | 0.10 | 0.77 | 2.32 | 0 |
| forge | ping-c512 | 282,584 | 264,039 | 0.88 | 2.49 | 4.76 | 0 |
| forge | json-c64 | 267,699 | 284,351 | 0.11 | 2.48 | 0.97 | 0 |
| forge | param-c64 | 277,941 | 279,159 | 0.11 | 2.10 | 2.24 | 0 |
| axum | ping-c1 | 32,011 | 32,221 | 0.03 | 0.37 | 0.60 | 0 |
| axum | ping-c64 | 212,358 | 193,416 | 0.23 | 2.07 | 2.84 | 0 |
| axum | ping-c512 | 243,669 | 249,422 | 1.91 | 5.40 | 5.74 | 0 |
| axum | json-c64 | 251,859 | 258,617 | 0.22 | 0.83 | 0.67 | 0 |
| axum | param-c64 | 238,579 | 259,823 | 0.23 | 1.63 | 0.84 | 0 |

Largest req/s difference between the two runs: 8.9% (axum, `ping-c64` and `param-c64`).

With `--include-pending`, `query-c64` reproduces #10 on Forge: about 30k req/s with every response a 404
and a closed connection, while axum serves it like `ping-c64`.
