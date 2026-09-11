use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

/// Entry in the audit trail
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditEntry {
    pub action: String,
    pub timestamp: DateTime<Utc>,
    pub hash: String,
    pub previous_hash: String,
}

/// Merkle audit trail for tamper detection
pub struct MerkleAuditTrail {
    entries: Arc<Mutex<Vec<AuditEntry>>>,
    root_hash: Arc<Mutex<String>>,
}

impl MerkleAuditTrail {
    /// Create a new audit trail
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
            root_hash: Arc::new(Mutex::new(Self::empty_hash().to_string())),
        }
    }

    /// Append an action to the audit trail
    pub fn append(&self, action: &str) -> Result<String, String> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        let previous_hash = self
            .root_hash
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?
            .clone();

        let mut hasher = Sha256::new();
        hasher.update(action.as_bytes());
        hasher.update(previous_hash.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let entry = AuditEntry {
            action: action.to_string(),
            timestamp: Utc::now(),
            hash: hash.clone(),
            previous_hash,
        };

        entries.push(entry);

        // Update root hash
        let mut root = self
            .root_hash
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        *root = hash.clone();

        Ok(hash)
    }

    /// Verify the integrity of the entire chain
    pub fn verify_chain(&self) -> Result<bool, String> {
        let entries = self
            .entries
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        if entries.is_empty() {
            return Ok(true);
        }

        let mut expected_prev = Self::empty_hash().to_string();

        for entry in entries.iter() {
            if entry.previous_hash != expected_prev {
                return Ok(false);
            }

            let mut hasher = Sha256::new();
            hasher.update(entry.action.as_bytes());
            hasher.update(expected_prev.as_bytes());
            let calculated = format!("{:x}", hasher.finalize());

            if calculated != entry.hash {
                return Ok(false);
            }

            expected_prev = entry.hash.clone();
        }

        Ok(true)
    }

    /// Get the root hash of the audit trail
    pub fn get_root_hash(&self) -> Result<String, String> {
        self.root_hash
            .lock()
            .map(|h| h.clone())
            .map_err(|e| format!("Lock error: {}", e))
    }

    /// Get number of entries in the audit trail
    pub fn entry_count(&self) -> Result<usize, String> {
        self.entries
            .lock()
            .map(|e| e.len())
            .map_err(|e| format!("Lock error: {}", e))
    }

    /// Get all entries (for inspection/export)
    pub fn get_entries(&self) -> Result<Vec<AuditEntry>, String> {
        self.entries
            .lock()
            .map(|e| e.clone())
            .map_err(|e| format!("Lock error: {}", e))
    }

    fn empty_hash() -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"");
        format!("{:x}", hasher.finalize())
    }
}

impl Default for MerkleAuditTrail {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_trail_creation() {
        let trail = MerkleAuditTrail::new();
        assert_eq!(trail.entry_count().unwrap(), 0);
    }

    #[test]
    fn test_append_action() {
        let trail = MerkleAuditTrail::new();
        let hash = trail.append("action1").unwrap();
        assert!(!hash.is_empty());
        assert_eq!(trail.entry_count().unwrap(), 1);
    }

    #[test]
    fn test_verify_chain() {
        let trail = MerkleAuditTrail::new();
        trail.append("action1").unwrap();
        trail.append("action2").unwrap();
        trail.append("action3").unwrap();

        assert!(trail.verify_chain().unwrap());
    }

    #[test]
    fn test_root_hash_changes() {
        let trail = MerkleAuditTrail::new();
        let hash1 = trail.get_root_hash().unwrap();

        trail.append("action1").unwrap();
        let hash2 = trail.get_root_hash().unwrap();

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_get_entries() {
        let trail = MerkleAuditTrail::new();
        trail.append("action1").unwrap();
        trail.append("action2").unwrap();

        let entries = trail.get_entries().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].action, "action1");
        assert_eq!(entries[1].action, "action2");
    }
}
