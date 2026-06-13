//! Server-metrics collection: run a single, self-contained probe over SSH and
//! parse its output into a [`ServerMetrics`] snapshot.
//!
//! The probe is one shell command that reads `/proc`, samples `/proc/stat`
//! twice (so CPU utilisation can be derived from the delta without any
//! client-side state), and prints each datum under a `@@section` marker. Doing
//! everything in a single `exec` keeps each poll to one round-trip and one
//! channel, and leaves the interactive shell untouched.
//!
//! Parsing lives in the pure [`parse_metrics`] function so it can be unit
//! tested exhaustively without a network. It is intentionally Linux-oriented;
//! on a host without `/proc` the snapshot comes back [`ServerMetrics::unsupported`].

use std::collections::HashMap;
use std::sync::Arc;

use crate::application::ports::CommandRunner;
use crate::domain::error::Result;
use crate::domain::metrics::{
    CpuMetrics, DiskUsage, LoadAverage, MemoryMetrics, ProcessInfo, ServerMetrics, SwapMetrics,
};
use crate::domain::session::SessionId;

/// The probe command. POSIX `sh`-compatible; every external lookup is guarded so
/// a missing file degrades gracefully to an empty section rather than aborting.
///
/// The two `/proc/stat` samples are separated by a short sleep; `parse_metrics`
/// turns the delta into a busy-percentage per core.
pub const METRICS_COMMAND: &str = "\
echo @@host; uname -n 2>/dev/null; (. /etc/os-release 2>/dev/null; echo \"${PRETTY_NAME:-}\");
echo @@uptime; cat /proc/uptime 2>/dev/null;
echo @@load; cat /proc/loadavg 2>/dev/null;
echo @@cpuinfo; grep -c ^processor /proc/cpuinfo 2>/dev/null; grep -m1 'model name' /proc/cpuinfo 2>/dev/null;
echo @@cpu1; grep ^cpu /proc/stat 2>/dev/null;
sleep 0.3;
echo @@cpu2; grep ^cpu /proc/stat 2>/dev/null;
echo @@mem; cat /proc/meminfo 2>/dev/null;
echo @@disk; df -kP 2>/dev/null;
echo @@proc; ps -eo pcpu,pmem,comm --sort=-pcpu 2>/dev/null | sed -n '2,7p';
echo @@end";

/// Collects [`ServerMetrics`] for a session by running the probe and parsing it.
pub struct MetricsService {
    runner: Arc<dyn CommandRunner>,
}

impl std::fmt::Debug for MetricsService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MetricsService")
    }
}

impl MetricsService {
    pub fn new(runner: Arc<dyn CommandRunner>) -> Self {
        Self { runner }
    }

    /// Run the probe on `id` and parse the result. Errors only if the command
    /// itself could not be run (e.g. the session is gone); an unparseable or
    /// non-Linux response yields a snapshot flagged [`ServerMetrics::unsupported`].
    pub async fn collect(&self, id: SessionId) -> Result<ServerMetrics> {
        let out = self.runner.exec(id, METRICS_COMMAND).await?;
        Ok(parse_metrics(&String::from_utf8_lossy(&out.stdout)))
    }
}

/// Round to one decimal place — enough precision for a percentage gauge.
fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// Group the probe output into its `@@`-delimited sections.
fn split_sections(raw: &str) -> HashMap<&str, Vec<&str>> {
    let mut sections: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut current: Option<&str> = None;
    for line in raw.lines() {
        if let Some(name) = line.strip_prefix("@@") {
            current = Some(name.trim());
            sections.entry(name.trim()).or_default();
        } else if let Some(name) = current {
            sections.get_mut(name).unwrap().push(line);
        }
    }
    sections
}

/// Parse a `/proc/stat` cpu line into `(label, jiffy fields)`.
fn parse_cpu_line(line: &str) -> Option<(&str, Vec<u64>)> {
    let mut it = line.split_whitespace();
    let label = it.next()?;
    if !label.starts_with("cpu") {
        return None;
    }
    let vals: Vec<u64> = it.filter_map(|s| s.parse().ok()).collect();
    if vals.is_empty() {
        None
    } else {
        Some((label, vals))
    }
}

/// `(total jiffies, idle jiffies)` for one cpu sample. Idle counts the `idle`
/// and `iowait` fields (indices 3 and 4).
fn total_and_idle(vals: &[u64]) -> (u64, u64) {
    let idle = vals.get(3).copied().unwrap_or(0) + vals.get(4).copied().unwrap_or(0);
    let total: u64 = vals.iter().sum();
    (total, idle)
}

/// Busy percentage between two samples of the same core.
fn busy_percent(prev: &[u64], cur: &[u64]) -> f64 {
    let (t0, i0) = total_and_idle(prev);
    let (t1, i1) = total_and_idle(cur);
    let total_delta = t1.saturating_sub(t0);
    if total_delta == 0 {
        return 0.0;
    }
    let idle_delta = i1.saturating_sub(i0);
    let busy = total_delta.saturating_sub(idle_delta) as f64;
    round1((busy / total_delta as f64 * 100.0).clamp(0.0, 100.0))
}

fn parse_cpu(first: &[&str], second: &[&str], cpuinfo: &[&str]) -> CpuMetrics {
    let sample = |lines: &[&str]| -> HashMap<String, Vec<u64>> {
        lines
            .iter()
            .filter_map(|l| parse_cpu_line(l).map(|(k, v)| (k.to_string(), v)))
            .collect()
    };
    let s1 = sample(first);
    let s2 = sample(second);

    let usage_percent = match (s1.get("cpu"), s2.get("cpu")) {
        (Some(a), Some(b)) => busy_percent(a, b),
        _ => 0.0,
    };

    // Per-core lines are `cpu0`, `cpu1`, ... — collect and order numerically.
    let mut core_indices: Vec<u32> = s1
        .keys()
        .filter_map(|k| k.strip_prefix("cpu").and_then(|n| n.parse::<u32>().ok()))
        .collect();
    core_indices.sort_unstable();
    let per_core: Vec<f64> = core_indices
        .iter()
        .filter_map(|i| {
            let key = format!("cpu{i}");
            match (s1.get(&key), s2.get(&key)) {
                (Some(a), Some(b)) => Some(busy_percent(a, b)),
                _ => None,
            }
        })
        .collect();

    let cores = cpuinfo
        .first()
        .and_then(|l| l.trim().parse::<u32>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(per_core.len() as u32);

    let model = cpuinfo
        .get(1)
        .map(|l| l.split_once(':').map(|(_, v)| v.trim()).unwrap_or(l.trim()))
        .unwrap_or("")
        .to_string();

    CpuMetrics {
        usage_percent,
        cores,
        model,
        per_core,
    }
}

/// Pull a `Key:  value kB` field out of `/proc/meminfo`, in KiB.
fn meminfo_kb(lines: &[&str], key: &str) -> Option<u64> {
    lines.iter().find_map(|l| {
        let (k, rest) = l.split_once(':')?;
        if k.trim() == key {
            rest.split_whitespace().next()?.parse().ok()
        } else {
            None
        }
    })
}

fn parse_memory(lines: &[&str]) -> (MemoryMetrics, SwapMetrics) {
    let total = meminfo_kb(lines, "MemTotal").unwrap_or(0);
    // Prefer the kernel's MemAvailable estimate; fall back to free+buffers+cached.
    let available = meminfo_kb(lines, "MemAvailable").unwrap_or_else(|| {
        meminfo_kb(lines, "MemFree").unwrap_or(0)
            + meminfo_kb(lines, "Buffers").unwrap_or(0)
            + meminfo_kb(lines, "Cached").unwrap_or(0)
    });
    let used = total.saturating_sub(available);
    let memory = MemoryMetrics {
        total_kb: total,
        used_kb: used,
        available_kb: available,
        used_percent: if total == 0 {
            0.0
        } else {
            round1(used as f64 / total as f64 * 100.0)
        },
    };

    let swap_total = meminfo_kb(lines, "SwapTotal").unwrap_or(0);
    let swap_free = meminfo_kb(lines, "SwapFree").unwrap_or(0);
    let swap_used = swap_total.saturating_sub(swap_free);
    let swap = SwapMetrics {
        total_kb: swap_total,
        used_kb: swap_used,
        used_percent: if swap_total == 0 {
            0.0
        } else {
            round1(swap_used as f64 / swap_total as f64 * 100.0)
        },
    };

    (memory, swap)
}

/// True for filesystems that aren't real storage (tmpfs, cgroups, snap loops…)
/// and shouldn't clutter the disk view.
fn is_pseudo_fs(filesystem: &str, mount: &str) -> bool {
    const PSEUDO: [&str; 6] = ["tmpfs", "devtmpfs", "udev", "overlay", "none", "shm"];
    PSEUDO.contains(&filesystem)
        || filesystem.starts_with("/dev/loop")
        || ["/dev", "/sys", "/proc", "/run"]
            .iter()
            .any(|p| mount == *p || mount.starts_with(&format!("{p}/")))
}

fn parse_disks(lines: &[&str]) -> Vec<DiskUsage> {
    let mut disks: Vec<DiskUsage> = lines
        .iter()
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            // df -kP: Filesystem 1024-blocks Used Available Capacity Mounted-on
            if f.len() < 6 || f[0] == "Filesystem" {
                return None;
            }
            let total: u64 = f[1].parse().ok()?;
            let used: u64 = f[2].parse().ok()?;
            let available: u64 = f[3].parse().ok()?;
            let mount = f[5..].join(" ");
            if total == 0 || is_pseudo_fs(f[0], &mount) {
                return None;
            }
            Some(DiskUsage {
                filesystem: f[0].to_string(),
                mount,
                total_kb: total,
                used_kb: used,
                available_kb: available,
                used_percent: round1(used as f64 / total as f64 * 100.0),
            })
        })
        .collect();
    // Root first, then largest volumes — the order users scan in.
    disks.sort_by(|a, b| {
        (a.mount != "/", b.total_kb).cmp(&(b.mount != "/", a.total_kb))
    });
    disks
}

fn parse_processes(lines: &[&str]) -> Vec<ProcessInfo> {
    lines
        .iter()
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            let cpu: f64 = it.next()?.parse().ok()?;
            let mem: f64 = it.next()?.parse().ok()?;
            let command: String = it.collect::<Vec<_>>().join(" ");
            if command.is_empty() {
                return None;
            }
            Some(ProcessInfo {
                cpu_percent: round1(cpu),
                mem_percent: round1(mem),
                command,
            })
        })
        .collect()
}

/// Parse the raw probe output into a [`ServerMetrics`] snapshot. Pure and
/// total: any missing or malformed section simply yields its default.
pub fn parse_metrics(raw: &str) -> ServerMetrics {
    let s = split_sections(raw);
    let get = |name: &str| s.get(name).cloned().unwrap_or_default();

    let host = get("host");
    let hostname = host.first().map(|l| l.trim().to_string()).unwrap_or_default();
    let os = host.get(1).map(|l| l.trim().to_string()).unwrap_or_default();

    let uptime_seconds = get("uptime")
        .first()
        .and_then(|l| l.split_whitespace().next())
        .and_then(|n| n.parse::<f64>().ok())
        .map(|s| s as u64)
        .unwrap_or(0);

    let load = {
        let parts: Vec<f64> = get("load")
            .first()
            .map(|l| {
                l.split_whitespace()
                    .take(3)
                    .filter_map(|n| n.parse().ok())
                    .collect()
            })
            .unwrap_or_default();
        LoadAverage {
            one: parts.first().copied().unwrap_or(0.0),
            five: parts.get(1).copied().unwrap_or(0.0),
            fifteen: parts.get(2).copied().unwrap_or(0.0),
        }
    };

    let cpu = parse_cpu(&get("cpu1"), &get("cpu2"), &get("cpuinfo"));
    let (memory, swap) = parse_memory(&get("mem"));
    let disks = parse_disks(&get("disk"));
    let processes = parse_processes(&get("proc"));

    // If neither CPU stats nor memory were readable, /proc was absent: flag it
    // so the UI can explain rather than show a wall of zeroes.
    let unsupported = memory.total_kb == 0 && get("cpu1").is_empty();

    ServerMetrics {
        hostname,
        os,
        uptime_seconds,
        load,
        cpu,
        memory,
        swap,
        disks,
        processes,
        unsupported,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
@@host
web-01
Ubuntu 22.04.4 LTS
@@uptime
123456.78 987654.32
@@load
0.52 0.48 0.40 2/431 28114
@@cpuinfo
4
model name\t: Intel(R) Xeon(R) CPU E5-2670 0 @ 2.60GHz
@@cpu1
cpu  1000 0 500 8000 100 0 0 0 0 0
cpu0 250 0 125 2000 25 0 0 0 0 0
cpu1 250 0 125 2000 25 0 0 0 0 0
@@cpu2
cpu  1100 0 550 8400 100 0 0 0 0 0
cpu0 280 0 140 2080 25 0 0 0 0 0
cpu1 275 0 135 2090 25 0 0 0 0 0
@@mem
MemTotal:        8000000 kB
MemFree:          500000 kB
MemAvailable:    6000000 kB
Buffers:          200000 kB
Cached:          1000000 kB
SwapTotal:       2000000 kB
SwapFree:        1500000 kB
@@disk
Filesystem     1024-blocks     Used Available Capacity Mounted on
/dev/sda1        100000000 40000000  60000000      40% /
tmpfs              4000000        0   4000000       0% /dev/shm
/dev/sdb1        500000000 50000000 450000000      10% /data
@@proc
 12.5  3.2 nginx
  8.0  1.1 postgres
@@end
";

    #[test]
    fn parses_host_uptime_load() {
        let m = parse_metrics(SAMPLE);
        assert_eq!(m.hostname, "web-01");
        assert_eq!(m.os, "Ubuntu 22.04.4 LTS");
        assert_eq!(m.uptime_seconds, 123456);
        assert_eq!(m.load.one, 0.52);
        assert_eq!(m.load.fifteen, 0.40);
        assert!(!m.unsupported);
    }

    #[test]
    fn computes_cpu_from_delta() {
        let m = parse_metrics(SAMPLE);
        assert_eq!(m.cpu.cores, 4);
        assert!(m.cpu.model.contains("Xeon"));
        assert!(!m.cpu.model.contains("model name"));
        // aggregate: total delta = 200+100+50 = ... busy = non-idle delta.
        // sample1 total=9600 idle(8000+100)=8100; sample2 total=10150 idle=8500.
        // td=550, idled=400, busy=150 -> 27.3%.
        assert!((m.cpu.usage_percent - 27.3).abs() < 0.2, "{}", m.cpu.usage_percent);
        assert_eq!(m.cpu.per_core.len(), 2);
        assert!(m.cpu.per_core.iter().all(|&p| (0.0..=100.0).contains(&p)));
    }

    #[test]
    fn parses_memory_and_swap() {
        let m = parse_metrics(SAMPLE);
        assert_eq!(m.memory.total_kb, 8_000_000);
        assert_eq!(m.memory.available_kb, 6_000_000);
        assert_eq!(m.memory.used_kb, 2_000_000);
        assert_eq!(m.memory.used_percent, 25.0);
        assert_eq!(m.swap.total_kb, 2_000_000);
        assert_eq!(m.swap.used_kb, 500_000);
        assert_eq!(m.swap.used_percent, 25.0);
    }

    #[test]
    fn parses_disks_filtering_pseudo_and_ordering_root_first() {
        let m = parse_metrics(SAMPLE);
        // tmpfs on /dev/shm is dropped.
        assert_eq!(m.disks.len(), 2);
        assert_eq!(m.disks[0].mount, "/");
        assert_eq!(m.disks[0].used_percent, 40.0);
        assert_eq!(m.disks[1].mount, "/data");
        assert!(m.disks.iter().all(|d| d.filesystem != "tmpfs"));
    }

    #[test]
    fn parses_top_processes() {
        let m = parse_metrics(SAMPLE);
        assert_eq!(m.processes.len(), 2);
        assert_eq!(m.processes[0].command, "nginx");
        assert_eq!(m.processes[0].cpu_percent, 12.5);
        assert_eq!(m.processes[1].command, "postgres");
    }

    #[test]
    fn non_linux_host_is_flagged_unsupported() {
        let raw = "@@host\nmac\nmacOS\n@@cpu1\n@@mem\n@@end\n";
        let m = parse_metrics(raw);
        assert!(m.unsupported);
        assert_eq!(m.hostname, "mac");
    }

    #[test]
    fn empty_input_does_not_panic() {
        let m = parse_metrics("");
        assert!(m.unsupported);
        assert_eq!(m.cpu.cores, 0);
        assert!(m.disks.is_empty());
    }
}
