use crate::types::AgentId;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// A cryptographic receipt proving that a tool was successfully executed by the runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolReceipt {
    pub receipt_id: String, // UUIDv7
    pub agent_id: AgentId,
    pub tool_name: String,
    pub call_args_hash: String,      // BLAKE3 of canonical JSON args
    pub result_hash: Option<String>, // BLAKE3 of result bytes (None for intent)
    pub executed_at_ms: u64,
    pub hmac_tag: [u8; 32], // MAC tag protecting the record
}

/// Outcome of validating agent tool claims.
#[derive(Debug, Clone, Default)]
pub struct ReceiptValidationResult {
    pub valid: Vec<String>,
    pub fabricated: Vec<String>, // claimed but no ledger entry
    pub unverified: Vec<String>, // ledger entry exists but tag fails
}

/// Thread-safe ledger of tool execution receipts.
pub struct ToolReceiptLedger {
    /// Per-session MAC key (32 bytes).
    session_key: [u8; 32],
    /// In-memory storage of receipts issued this session.
    receipts: Arc<RwLock<HashMap<String, ToolReceipt>>>,
}

impl ToolReceiptLedger {
    /// Create a new ledger with a session-specific key.
    pub fn new(session_key: [u8; 32]) -> Self {
        Self {
            session_key,
            receipts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a ledger from orchestrator config, using a persistent key if available.
    pub fn from_config(config: &crate::config::OrchestratorConfig) -> Self {
        let mut session_key = [0u8; 32];
        let mut valid_key = false;

        if !config.tool_ledger_key.is_empty() {
            if let Ok(bytes) = hex::decode(&config.tool_ledger_key) {
                if bytes.len() == 32 {
                    session_key.copy_from_slice(&bytes);
                    valid_key = true;
                }
            }
        }

        if !valid_key {
            // Generate ephemeral key if no valid persistent key is provided
            let mut k = [0u8; 32];
            let _ = getrandom::getrandom(&mut k);
            session_key = k;
        }

        Self::new(session_key)
    }

    /// Number of receipts in the ledger.
    pub fn len(&self) -> usize {
        self.receipts.read().len()
    }

    /// Whether the ledger is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a snapshot of all receipts.
    pub fn snapshot(&self) -> HashMap<String, (AgentId, String)> {
        let guard = self.receipts.read();
        let receipts: &HashMap<String, ToolReceipt> = &guard;
        receipts
            .iter()
            .map(|(id, r)| (id.clone(), (r.agent_id, r.tool_name.clone())))
            .collect()
    }

    /// Issue a new receipt for a tool execution intent.
    pub fn issue_intent(&self, agent_id: AgentId, tool_name: &str, args_json: &str) -> ToolReceipt {
        let receipt_id = Uuid::now_v7().to_string();
        let executed_at_ms = chrono::Utc::now().timestamp_millis() as u64;

        let args_hash = blake3::hash(args_json.as_bytes()).to_string();

        let mut hasher = blake3::Hasher::new_keyed(&self.session_key);
        hasher.update(receipt_id.as_bytes());
        hasher.update(&agent_id.0.to_le_bytes());
        hasher.update(tool_name.as_bytes());
        hasher.update(args_hash.as_bytes());
        hasher.update(&executed_at_ms.to_le_bytes());
        let hmac_tag = hasher.finalize().into();

        let receipt = ToolReceipt {
            receipt_id: receipt_id.clone(),
            agent_id,
            tool_name: tool_name.to_string(),
            call_args_hash: args_hash,
            result_hash: None,
            executed_at_ms,
            hmac_tag,
        };

        let mut map = self.receipts.write();
        map.insert(receipt_id, receipt.clone());
        receipt
    }

    /// Update a receipt with the execution result.
    pub fn fulfill_intent(
        &self,
        receipt_id: &str,
        result_json: &str,
    ) -> Result<ToolReceipt, &'static str> {
        let mut map = self.receipts.write();
        let receipt = map.get_mut(receipt_id).ok_or("Receipt not found")?;

        let res_hash = blake3::hash(result_json.as_bytes()).to_string();
        receipt.result_hash = Some(res_hash.clone());

        // Re-compute tag with result
        let mut hasher = blake3::Hasher::new_keyed(&self.session_key);
        hasher.update(receipt.receipt_id.as_bytes());
        hasher.update(&receipt.agent_id.0.to_le_bytes());
        hasher.update(receipt.tool_name.as_bytes());
        hasher.update(receipt.call_args_hash.as_bytes());
        hasher.update(res_hash.as_bytes());
        hasher.update(&receipt.executed_at_ms.to_le_bytes());
        receipt.hmac_tag = hasher.finalize().into();

        Ok(receipt.clone())
    }

    /// Issue a complete receipt (intent + result).
    pub fn issue(
        &self,
        agent_id: AgentId,
        tool_name: &str,
        args_json: &str,
        result_json: &str,
    ) -> ToolReceipt {
        let receipt = self.issue_intent(agent_id, tool_name, args_json);
        self.fulfill_intent(&receipt.receipt_id, result_json)
            .unwrap()
    }

    /// Verify a single receipt by ID.
    pub fn verify(&self, receipt_id: &str) -> Result<(), &'static str> {
        let map = self.receipts.read();
        let receipt = map.get(receipt_id).ok_or("Receipt not found in ledger")?;

        let mut hasher = blake3::Hasher::new_keyed(&self.session_key);
        hasher.update(receipt.receipt_id.as_bytes());
        hasher.update(&receipt.agent_id.0.to_le_bytes());
        hasher.update(receipt.tool_name.as_bytes());
        hasher.update(receipt.call_args_hash.as_bytes());
        if let Some(ref res) = receipt.result_hash {
            hasher.update(res.as_bytes());
        }
        hasher.update(&receipt.executed_at_ms.to_le_bytes());
        let expected_tag: [u8; 32] = hasher.finalize().into();

        if expected_tag == receipt.hmac_tag {
            Ok(())
        } else {
            Err("Receipt HMAC verification failed")
        }
    }

    /// Validate a list of receipt IDs claimed by an agent.
    pub fn validate_agent_claims(&self, claimed_receipt_ids: &[String]) -> ReceiptValidationResult {
        let mut result = ReceiptValidationResult::default();
        for id in claimed_receipt_ids {
            match self.verify(id) {
                Ok(_) => result.valid.push(id.clone()),
                Err("Receipt not found in ledger") => result.fabricated.push(id.clone()),
                Err(_) => result.unverified.push(id.clone()),
            }
        }
        result
    }
}
