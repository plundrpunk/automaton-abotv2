use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// Tracks which data has been marked as tainted
pub struct TaintTracker {
    tainted_ids: Arc<Mutex<HashSet<String>>>,
}

impl TaintTracker {
    /// Create a new taint tracker
    pub fn new() -> Self {
        Self {
            tainted_ids: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Mark data as tainted
    pub fn taint(&self, id: impl Into<String>) -> Result<(), String> {
        let mut tainted = self
            .tainted_ids
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        tainted.insert(id.into());
        Ok(())
    }

    /// Mark data as untainted
    pub fn untaint(&self, id: impl Into<String>) -> Result<(), String> {
        let mut tainted = self
            .tainted_ids
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        tainted.remove(&id.into());
        Ok(())
    }

    /// Check if data is tainted
    pub fn is_tainted(&self, id: &str) -> Result<bool, String> {
        let tainted = self
            .tainted_ids
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        Ok(tainted.contains(id))
    }

    /// Get count of tainted items
    pub fn tainted_count(&self) -> Result<usize, String> {
        let tainted = self
            .tainted_ids
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        Ok(tainted.len())
    }

    /// Clear all taint marks
    pub fn clear(&self) -> Result<(), String> {
        let mut tainted = self
            .tainted_ids
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        tainted.clear();
        Ok(())
    }

    /// Get all tainted IDs
    pub fn get_tainted(&self) -> Result<Vec<String>, String> {
        let tainted = self
            .tainted_ids
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        Ok(tainted.iter().cloned().collect())
    }
}

impl Default for TaintTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for TaintTracker {
    fn clone(&self) -> Self {
        Self {
            tainted_ids: Arc::clone(&self.tainted_ids),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_taint_and_check() {
        let tracker = TaintTracker::new();
        tracker.taint("data_id_1").unwrap();

        assert!(tracker.is_tainted("data_id_1").unwrap());
        assert!(!tracker.is_tainted("data_id_2").unwrap());
    }

    #[test]
    fn test_untaint() {
        let tracker = TaintTracker::new();
        tracker.taint("data_id").unwrap();
        assert!(tracker.is_tainted("data_id").unwrap());

        tracker.untaint("data_id").unwrap();
        assert!(!tracker.is_tainted("data_id").unwrap());
    }

    #[test]
    fn test_multiple_taint_entries() {
        let tracker = TaintTracker::new();
        tracker.taint("id1").unwrap();
        tracker.taint("id2").unwrap();
        tracker.taint("id3").unwrap();

        assert_eq!(tracker.tainted_count().unwrap(), 3);
        assert!(tracker.is_tainted("id1").unwrap());
        assert!(tracker.is_tainted("id2").unwrap());
        assert!(tracker.is_tainted("id3").unwrap());
    }

    #[test]
    fn test_clear() {
        let tracker = TaintTracker::new();
        tracker.taint("id1").unwrap();
        tracker.taint("id2").unwrap();
        assert_eq!(tracker.tainted_count().unwrap(), 2);

        tracker.clear().unwrap();
        assert_eq!(tracker.tainted_count().unwrap(), 0);
    }

    #[test]
    fn test_get_tainted() {
        let tracker = TaintTracker::new();
        tracker.taint("data1").unwrap();
        tracker.taint("data2").unwrap();
        tracker.taint("data3").unwrap();

        let tainted = tracker.get_tainted().unwrap();
        assert_eq!(tainted.len(), 3);
        assert!(tainted.contains(&"data1".to_string()));
        assert!(tainted.contains(&"data2".to_string()));
    }

    #[test]
    fn test_clone_shares_state() {
        let tracker1 = TaintTracker::new();
        tracker1.taint("shared_id").unwrap();

        let tracker2 = tracker1.clone();
        assert!(tracker2.is_tainted("shared_id").unwrap());

        tracker2.taint("new_id").unwrap();
        assert!(tracker1.is_tainted("new_id").unwrap());
    }
}
