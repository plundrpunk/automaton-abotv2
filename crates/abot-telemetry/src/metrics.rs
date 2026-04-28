use serde::{Deserialize, Serialize};

/// System metrics snapshot
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Memory usage in MB
    pub ram_mb: f64,
    /// CPU utilization percentage (0-100)
    pub cpu_pct: f64,
    /// Uptime in seconds
    pub uptime_secs: u64,
    /// Timestamp when metrics were collected
    pub timestamp: String,
}

impl SystemMetrics {
    /// Collect current system metrics
    pub fn collect(uptime_secs: u64) -> Self {
        let ram_mb = Self::get_memory_mb();
        let cpu_pct = Self::get_cpu_percent();

        Self {
            ram_mb,
            cpu_pct,
            uptime_secs,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Get current process memory usage in MB
    #[cfg(target_os = "linux")]
    fn get_memory_mb() -> f64 {
        use std::fs;

        // Read /proc/self/status to get memory usage
        match fs::read_to_string("/proc/self/status") {
            Ok(content) => content
                .lines()
                .find(|line| line.starts_with("VmRSS:"))
                .and_then(|line| {
                    line.split_whitespace()
                        .nth(1)
                        .and_then(|s| s.parse::<f64>().ok())
                })
                .map(|kb| kb / 1024.0)
                .unwrap_or(0.0),
            Err(_) => 0.0,
        }
    }

    #[cfg(target_os = "macos")]
    fn get_memory_mb() -> f64 {
        // Simplified for macOS - would normally use system calls
        0.0
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    fn get_memory_mb() -> f64 {
        0.0
    }

    /// Get current CPU usage percentage
    fn get_cpu_percent() -> f64 {
        // Placeholder - actual implementation would use /proc/stat or system APIs
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collection() {
        let metrics = SystemMetrics::collect(3600);

        assert_eq!(metrics.uptime_secs, 3600);
        assert!(metrics.cpu_pct >= 0.0 && metrics.cpu_pct <= 100.0);
        assert!(metrics.ram_mb >= 0.0);
    }

    #[test]
    fn test_metrics_serialization() {
        let metrics = SystemMetrics::collect(1000);
        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: SystemMetrics = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.uptime_secs, 1000);
    }
}
