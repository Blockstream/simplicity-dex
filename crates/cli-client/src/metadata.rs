use serde::{Deserialize, Serialize};

use crate::error::Error;

/// Represents a single action in the contract's history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// The action type (e.g., "created", "funded", "`swap_created`", "`swap_taken`", etc.)
    pub action: String,
    /// Blockchain transaction ID associated with this action
    pub txid: Option<String>,
    /// NOSTR event ID associated with this action
    pub nostr_event_id: Option<String>,
    /// Unix timestamp when this action occurred
    pub timestamp: i64,
    /// Additional context or details about the action
    pub details: Option<String>,
}

impl HistoryEntry {
    /// Create a new history entry with a blockchain transaction.
    #[must_use]
    pub fn with_txid(action: &str, txid: &str, timestamp: i64) -> Self {
        Self {
            action: action.to_string(),
            txid: Some(txid.to_string()),
            nostr_event_id: None,
            timestamp,
            details: None,
        }
    }

    /// Create a new history entry with both txid and NOSTR event.
    #[must_use]
    pub fn with_txid_and_nostr(action: &str, txid: &str, nostr_event_id: &str, timestamp: i64) -> Self {
        Self {
            action: action.to_string(),
            txid: Some(txid.to_string()),
            nostr_event_id: Some(nostr_event_id.to_string()),
            timestamp,
            details: None,
        }
    }

    /// Create a new history entry with only NOSTR event.
    #[must_use]
    #[allow(dead_code)]
    pub fn with_nostr(action: &str, nostr_event_id: &str, timestamp: i64) -> Self {
        Self {
            action: action.to_string(),
            txid: None,
            nostr_event_id: Some(nostr_event_id.to_string()),
            timestamp,
            details: None,
        }
    }

    /// Add details to the history entry.
    #[must_use]
    #[allow(dead_code)]
    pub fn with_details(mut self, details: &str) -> Self {
        self.details = Some(details.to_string());
        self
    }
}

/// Metadata for contracts stored in the database.
///
/// This is stored in the `app_metadata` column and contains additional information
/// that is not part of the contract arguments. The contract arguments themselves
/// are stored separately in the `arguments` column to avoid duplication.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContractMetadata {
    /// Nostr event ID if this contract was synced from Nostr
    pub nostr_event_id: Option<String>,
    /// Nostr author public key if synced from Nostr
    pub nostr_author: Option<String>,
    /// Timestamp when the contract was created
    pub created_at: Option<i64>,
    /// Parent event ID for linked contracts (e.g., swap linked to option)
    pub parent_event_id: Option<String>,
    /// Full history of actions taken on this contract
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
}

impl ContractMetadata {
    /// Create metadata for a locally created contract.
    #[must_use]
    #[allow(dead_code)]
    pub const fn from_local(created_at: i64) -> Self {
        Self {
            nostr_event_id: None,
            nostr_author: None,
            created_at: Some(created_at),
            parent_event_id: None,
            history: Vec::new(),
        }
    }

    /// Create metadata for a locally created contract with initial history.
    #[must_use]
    #[allow(dead_code)]
    pub const fn from_local_with_history(created_at: i64, history: Vec<HistoryEntry>) -> Self {
        Self {
            nostr_event_id: None,
            nostr_author: None,
            created_at: Some(created_at),
            parent_event_id: None,
            history,
        }
    }

    /// Create metadata for a contract synced from Nostr.
    #[must_use]
    pub const fn from_nostr(event_id: String, author: String, created_at: i64) -> Self {
        Self {
            nostr_event_id: Some(event_id),
            nostr_author: Some(author),
            created_at: Some(created_at),
            parent_event_id: None,
            history: Vec::new(),
        }
    }

    /// Create metadata for a contract synced from Nostr with initial history.
    #[must_use]
    pub const fn from_nostr_with_history(
        event_id: String,
        author: String,
        created_at: i64,
        history: Vec<HistoryEntry>,
    ) -> Self {
        Self {
            nostr_event_id: Some(event_id),
            nostr_author: Some(author),
            created_at: Some(created_at),
            parent_event_id: None,
            history,
        }
    }

    /// Create metadata for a contract synced from Nostr with a parent relationship.
    #[must_use]
    pub const fn from_nostr_with_parent(
        event_id: String,
        author: String,
        created_at: i64,
        parent_event_id: String,
    ) -> Self {
        Self {
            nostr_event_id: Some(event_id),
            nostr_author: Some(author),
            created_at: Some(created_at),
            parent_event_id: Some(parent_event_id),
            history: Vec::new(),
        }
    }

    /// Add a history entry to this metadata.
    pub fn add_history(&mut self, entry: HistoryEntry) {
        self.history.push(entry);
    }

    /// Add a history entry only if it doesn't already exist (by action + txid).
    /// Returns true if the entry was added, false if it was a duplicate.
    pub fn add_history_if_new(&mut self, entry: HistoryEntry) -> bool {
        // Check for existing entry with same action and txid
        let exists = self.history.iter().any(|e| {
            e.action == entry.action
                && match (&e.txid, &entry.txid) {
                    (Some(a), Some(b)) => a == b,
                    (None, None) => {
                        // If no txid, check nostr_event_id
                        match (&e.nostr_event_id, &entry.nostr_event_id) {
                            (Some(a), Some(b)) => a == b,
                            _ => false,
                        }
                    }
                    _ => false,
                }
        });

        if exists {
            false
        } else {
            self.history.push(entry);
            true
        }
    }

    /// Get the history entries.
    #[must_use]
    #[allow(dead_code)]
    pub fn history(&self) -> &[HistoryEntry] {
        &self.history
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        bincode::serde::encode_to_vec(self, bincode::config::standard()).map_err(Error::MetadataEncode)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let (metadata, _): (Self, usize) =
            bincode::serde::decode_from_slice(bytes, bincode::config::standard()).map_err(Error::MetadataDecode)?;
        Ok(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_roundtrip() {
        let metadata = ContractMetadata::from_nostr("event123".to_string(), "npub1abc".to_string(), 1_704_067_200);

        let bytes = metadata.to_bytes().unwrap();
        let restored = ContractMetadata::from_bytes(&bytes).unwrap();

        assert_eq!(restored.nostr_event_id, Some("event123".to_string()));
        assert_eq!(restored.nostr_author, Some("npub1abc".to_string()));
        assert_eq!(restored.created_at, Some(1_704_067_200));
        assert_eq!(restored.parent_event_id, None);
        assert!(restored.history.is_empty());
    }

    #[test]
    fn test_metadata_with_parent_roundtrip() {
        let metadata = ContractMetadata::from_nostr_with_parent(
            "swap456".to_string(),
            "npub1xyz".to_string(),
            1_704_153_600,
            "option123".to_string(),
        );

        let bytes = metadata.to_bytes().unwrap();
        let restored = ContractMetadata::from_bytes(&bytes).unwrap();

        assert_eq!(restored.nostr_event_id, Some("swap456".to_string()));
        assert_eq!(restored.parent_event_id, Some("option123".to_string()));
    }

    #[test]
    fn test_default_metadata() {
        let metadata = ContractMetadata::default();

        assert!(metadata.nostr_event_id.is_none());
        assert!(metadata.nostr_author.is_none());
        assert!(metadata.created_at.is_none());
        assert!(metadata.parent_event_id.is_none());
        assert!(metadata.history.is_empty());
    }

    #[test]
    fn test_local_metadata_roundtrip() {
        let metadata = ContractMetadata::from_local(1_704_067_200);

        let bytes = metadata.to_bytes().unwrap();
        let restored = ContractMetadata::from_bytes(&bytes).unwrap();

        assert_eq!(restored.nostr_event_id, None);
        assert_eq!(restored.nostr_author, None);
        assert_eq!(restored.created_at, Some(1_704_067_200));
        assert_eq!(restored.parent_event_id, None);
        assert!(restored.history.is_empty());
    }

    #[test]
    fn test_history_entry_with_txid() {
        let entry = HistoryEntry::with_txid("option_created", "abc123", 1_704_067_200);

        assert_eq!(entry.action, "option_created");
        assert_eq!(entry.txid, Some("abc123".to_string()));
        assert_eq!(entry.nostr_event_id, None);
        assert_eq!(entry.timestamp, 1_704_067_200);
        assert_eq!(entry.details, None);
    }

    #[test]
    fn test_history_entry_with_txid_and_nostr() {
        let entry = HistoryEntry::with_txid_and_nostr("swap_created", "tx123", "event456", 1_704_067_200);

        assert_eq!(entry.action, "swap_created");
        assert_eq!(entry.txid, Some("tx123".to_string()));
        assert_eq!(entry.nostr_event_id, Some("event456".to_string()));
    }

    #[test]
    fn test_history_entry_with_details() {
        let entry =
            HistoryEntry::with_txid("option_funded", "xyz789", 1_704_067_200).with_details("Funded with 1000 sats");

        assert_eq!(entry.details, Some("Funded with 1000 sats".to_string()));
    }

    #[test]
    fn test_metadata_with_history_roundtrip() {
        let history = vec![
            HistoryEntry::with_txid("option_created", "tx1", 1_704_067_200),
            HistoryEntry::with_txid_and_nostr("option_funded", "tx2", "event1", 1_704_067_300),
        ];

        let metadata = ContractMetadata::from_local_with_history(1_704_067_200, history);

        let bytes = metadata.to_bytes().unwrap();
        let restored = ContractMetadata::from_bytes(&bytes).unwrap();

        assert_eq!(restored.history.len(), 2);
        assert_eq!(restored.history[0].action, "option_created");
        assert_eq!(restored.history[1].action, "option_funded");
        assert_eq!(restored.history[1].nostr_event_id, Some("event1".to_string()));
    }

    #[test]
    fn test_add_history() {
        let mut metadata = ContractMetadata::from_local(1_704_067_200);
        assert!(metadata.history.is_empty());

        metadata.add_history(HistoryEntry::with_txid("action1", "tx1", 1_704_067_200));
        assert_eq!(metadata.history.len(), 1);

        metadata.add_history(HistoryEntry::with_txid("action2", "tx2", 1_704_067_300));
        assert_eq!(metadata.history.len(), 2);
    }
}
