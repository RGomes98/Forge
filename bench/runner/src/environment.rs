//! Machine and build information recorded next to every result, so two runs can be compared
//! knowing what changed besides the code.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub date_utc: String,
    pub git_commit: Option<String>,
    pub git_dirty: bool,
    pub kernel: Option<String>,
    pub cpu: Option<String>,
    pub cpu_count: usize,
    pub governor: Option<String>,
    pub turbo: Option<String>,
    pub load_average: Option<String>,
    pub rustc: Option<String>,
    pub wrk: Option<String>,
    pub build: String,
    pub server_cpus: String,
    pub wrk_cpus: String,
    pub duration_s: u32,
    pub warmup_s: u32,
}

pub struct RunSettings<'a> {
    pub server_cpus: &'a str,
    pub wrk_cpus: &'a str,
    pub duration_s: u32,
    pub warmup_s: u32,
}

impl Environment {
    pub fn collect(repo: &Path, settings: &RunSettings) -> Self {
        Self {
            date_utc: utc_now(),
            git_commit: command_line(
                Command::new("git")
                    .args(["rev-parse", "--short", "HEAD"])
                    .current_dir(repo),
            ),
            git_dirty: command_line(Command::new("git").args(["status", "--porcelain"]).current_dir(repo)).is_some(),
            kernel: read("/proc/sys/kernel/osrelease"),
            cpu: cpu_model(),
            cpu_count: thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
            governor: read("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"),
            turbo: turbo(),
            load_average: read("/proc/loadavg").map(|l| l.split_whitespace().take(3).collect::<Vec<_>>().join(" ")),
            rustc: command_line(Command::new("rustc").arg("--version").current_dir(repo)),
            // `wrk --version` prints the version and exits with status 1, so ignore the status.
            wrk: Command::new("wrk")
                .arg("--version")
                .output()
                .ok()
                .and_then(|o| String::from_utf8_lossy(&o.stdout).lines().next().map(str::to_owned)),
            build: "release (opt-level=3, lto=fat, codegen-units=1, panic=abort), mimalloc".to_owned(),
            server_cpus: settings.server_cpus.to_owned(),
            wrk_cpus: settings.wrk_cpus.to_owned(),
            duration_s: settings.duration_s,
            warmup_s: settings.warmup_s,
        }
    }

    pub fn print(&self) {
        let field = |name: &str, value: &dyn std::fmt::Display| println!("    {name:<13} {value}");
        let or_unknown = |value: &Option<String>| value.clone().unwrap_or_else(|| "unknown".to_owned());

        field("date (UTC)", &self.date_utc);
        field(
            "commit",
            &format!(
                "{}{}",
                or_unknown(&self.git_commit),
                if self.git_dirty { " (dirty)" } else { "" }
            ),
        );
        field("kernel", &or_unknown(&self.kernel));
        field("cpu", &format!("{} ({} CPUs)", or_unknown(&self.cpu), self.cpu_count));
        field("governor", &or_unknown(&self.governor));
        field("turbo", &or_unknown(&self.turbo));
        field("load average", &or_unknown(&self.load_average));
        field("rustc", &or_unknown(&self.rustc));
        field("wrk", &or_unknown(&self.wrk));
        field("build", &self.build);
        field("server CPUs", &self.server_cpus);
        field("wrk CPUs", &self.wrk_cpus);
        field(
            "timing",
            &format!("{}s per run after {}s of warm-up", self.duration_s, self.warmup_s),
        );
    }
}

fn read(path: &str) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_owned())
}

/// First line of stdout of a successful command; `None` if it failed or printed nothing.
fn command_line(command: &mut Command) -> Option<String> {
    let output = command.output().ok().filter(|o| o.status.success())?;
    let text: String = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!text.is_empty()).then(|| text.lines().next().unwrap_or_default().to_owned())
}

fn cpu_model() -> Option<String> {
    read("/proc/cpuinfo")?
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split_once(':'))
        .map(|(_, model)| model.trim().to_owned())
}

fn turbo() -> Option<String> {
    if let Some(no_turbo) = read("/sys/devices/system/cpu/intel_pstate/no_turbo") {
        return Some(if no_turbo == "1" { "off" } else { "on" }.to_owned());
    }
    read("/sys/devices/system/cpu/cpufreq/boost").map(|b| if b == "1" { "on" } else { "off" }.to_owned())
}

pub fn utc_now() -> String {
    let secs: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (year, month, day) = civil_from_days((secs / 86_400) as i64);
    let rem: u64 = secs % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 to a (year, month, day) date (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z: i64 = days + 719_468;
    let era: i64 = z.div_euclid(146_097);
    let doe: i64 = z - era * 146_097;
    let yoe: i64 = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy: i64 = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp: i64 = (5 * doy + 2) / 153;
    let day: u32 = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month: u32 = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year: i64 = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_days_to_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(20_754), (2026, 10, 28));
    }
}
