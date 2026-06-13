//! Server-metrics value objects.
//!
//! A [`ServerMetrics`] is a single point-in-time snapshot of a remote host's
//! resource usage, gathered by the metrics collector and surfaced to the UI's
//! live dashboard. All sizes are expressed in **kibibytes** (the unit `/proc`
//! and `df -k` report), letting the presentation layer format them uniformly.

use serde::{Deserialize, Serialize};

/// A complete snapshot of a server's current resource usage.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ServerMetrics {
    /// The host's reported node name (`uname -n`), empty if unknown.
    pub hostname: String,
    /// A human-friendly OS description (e.g. `Ubuntu 22.04.4 LTS`).
    pub os: String,
    /// Seconds since boot.
    pub uptime_seconds: u64,
    /// 1/5/15-minute load averages.
    pub load: LoadAverage,
    /// Aggregate and per-core CPU utilisation.
    pub cpu: CpuMetrics,
    /// Physical memory usage.
    pub memory: MemoryMetrics,
    /// Swap usage.
    pub swap: SwapMetrics,
    /// Per-filesystem disk usage (pseudo-filesystems excluded).
    pub disks: Vec<DiskUsage>,
    /// The most CPU-hungry processes, already sorted descending.
    pub processes: Vec<ProcessInfo>,
    /// Set when the host did not expose the expected interfaces (e.g. a
    /// non-Linux server without `/proc`); the UI shows a friendly notice.
    pub unsupported: bool,
}

/// 1/5/15-minute load averages from `/proc/loadavg`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct LoadAverage {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
}

/// CPU utilisation, derived from two `/proc/stat` samples.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CpuMetrics {
    /// Aggregate busy percentage across all cores, `0.0..=100.0`.
    pub usage_percent: f64,
    /// Logical core count.
    pub cores: u32,
    /// Model name (e.g. `Intel(R) Xeon(R) ...`), empty if unknown.
    pub model: String,
    /// Per-core busy percentages, in core order.
    pub per_core: Vec<f64>,
}

/// Physical memory usage in KiB. `used = total - available`, matching how
/// `free`/`top` report "used" (i.e. excluding reclaimable buffers/cache).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub total_kb: u64,
    pub used_kb: u64,
    pub available_kb: u64,
    pub used_percent: f64,
}

/// Swap usage in KiB.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct SwapMetrics {
    pub total_kb: u64,
    pub used_kb: u64,
    pub used_percent: f64,
}

/// Usage of one mounted filesystem, in KiB.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DiskUsage {
    pub filesystem: String,
    pub mount: String,
    pub total_kb: u64,
    pub used_kb: u64,
    pub available_kb: u64,
    pub used_percent: f64,
}

/// A single process row from `ps`, sorted by CPU usage.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub cpu_percent: f64,
    pub mem_percent: f64,
    pub command: String,
}
