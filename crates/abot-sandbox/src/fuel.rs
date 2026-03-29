use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Dual-mode fuel metering: tracks both fuel units and epoch-based timeouts
pub struct FuelMeter {
    fuel_limit: u64,
    fuel_consumed: Arc<AtomicU64>,
    epoch_deadline_ms: u64,
    start_time_ms: u64,
}

impl FuelMeter {
    /// Create a new fuel meter with limits
    ///
    /// # Arguments
    /// * `fuel_limit` - Maximum fuel units available
    /// * `epoch_deadline_ms` - Maximum execution time in milliseconds
    pub fn new(fuel_limit: u64, epoch_deadline_ms: u64) -> Self {
        let start_time_ms = Self::current_time_ms();

        Self {
            fuel_limit,
            fuel_consumed: Arc::new(AtomicU64::new(0)),
            epoch_deadline_ms,
            start_time_ms,
        }
    }

    /// Get remaining fuel units
    pub fn remaining(&self) -> u64 {
        let consumed = self.fuel_consumed.load(Ordering::Relaxed);
        self.fuel_limit.saturating_sub(consumed)
    }

    /// Check if fuel is exhausted
    pub fn is_exhausted(&self) -> bool {
        self.remaining() == 0
    }

    /// Get total fuel consumed so far
    pub fn consumed(&self) -> u64 {
        self.fuel_consumed.load(Ordering::Relaxed)
    }

    /// Consume fuel units (called by WASM runtime)
    pub fn consume(&self, amount: u64) -> bool {
        let current = self.fuel_consumed.load(Ordering::Relaxed);
        if current + amount > self.fuel_limit {
            return false; // Out of fuel
        }

        self.fuel_consumed
            .store(current + amount, Ordering::Relaxed);
        true
    }

    /// Check if epoch deadline has been exceeded
    pub fn is_deadline_exceeded(&self) -> bool {
        let elapsed = Self::current_time_ms() - self.start_time_ms;
        elapsed > self.epoch_deadline_ms
    }

    /// Get remaining time until deadline in milliseconds
    pub fn remaining_time_ms(&self) -> u64 {
        let elapsed = Self::current_time_ms() - self.start_time_ms;
        self.epoch_deadline_ms.saturating_sub(elapsed)
    }

    /// Get the epoch deadline in milliseconds
    pub fn deadline_ms(&self) -> u64 {
        self.epoch_deadline_ms
    }

    fn current_time_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

impl Clone for FuelMeter {
    fn clone(&self) -> Self {
        Self {
            fuel_limit: self.fuel_limit,
            fuel_consumed: Arc::clone(&self.fuel_consumed),
            epoch_deadline_ms: self.epoch_deadline_ms,
            start_time_ms: self.start_time_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuel_meter_creation() {
        let meter = FuelMeter::new(1000, 5000);
        assert_eq!(meter.remaining(), 1000);
        assert!(!meter.is_exhausted());
    }

    #[test]
    fn test_fuel_consumption() {
        let meter = FuelMeter::new(1000, 5000);
        assert!(meter.consume(500));
        assert_eq!(meter.consumed(), 500);
        assert_eq!(meter.remaining(), 500);
    }

    #[test]
    fn test_fuel_limit_exceeded() {
        let meter = FuelMeter::new(1000, 5000);
        assert!(meter.consume(900));
        assert!(!meter.consume(200)); // Would exceed limit
        assert_eq!(meter.consumed(), 900);
    }

    #[test]
    fn test_exhaustion_check() {
        let meter = FuelMeter::new(100, 5000);
        meter.consume(100);
        assert!(meter.is_exhausted());
    }

    #[test]
    fn test_deadline_tracking() {
        let meter = FuelMeter::new(1000, 10_000);
        let remaining = meter.remaining_time_ms();
        assert!(remaining > 0 && remaining <= 10_000);
    }
}
