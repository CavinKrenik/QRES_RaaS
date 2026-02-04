use crate::zk_proofs::NormProof;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

pub mod fragmentation;

/// The "Ghost" Update Packet
///
/// This structure brings together the three layers of the Phase 3 Security architecture:
/// 1. **Differential Privacy**: `masked_weights` contain noise. `dp_epsilon` tracks the budget.
/// 2. **Secure Aggregation**: `masked_weights` are masked with pairwise keys.
/// 3. **Zero-Knowledge Proofs**: `zk_proof` ensures the potentially garbage-looking masked data
///    came from a valid, bounded update.
///
/// **Phase 1 (v20): Viral Protocol** adds epidemic gossip with cure threshold detection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GhostUpdate {
    /// Sender Identity (Ed25519 Public Key Bytes)
    pub peer_id: [u8; 32],
    /// The payload: Q16.16 weights + Gaussian Noise + Pairwise Masks
    /// Store as i32 bits to accommodate the wrapping arithmetic of Secure Aggregation.
    pub masked_weights: Vec<i32>,
    /// Zero-Knowledge Proof that ||original_weights|| < Threshold
    pub zk_proof: NormProof,
    /// Privacy budget consumed by this update
    pub dp_epsilon: f32,
    /// Phase 1: Residual error from last round (for cure threshold detection)
    #[serde(default)]
    pub residual_error: f32,
    /// Phase 1: Accuracy improvement delta (for cure threshold detection)
    #[serde(default)]
    pub accuracy_delta: f32,
}

/// Phase 1 (v20): Viral Protocol Implementation
impl GhostUpdate {
    /// Cure Threshold Detection
    ///
    /// Returns true when this update represents a "cure-worthy" improvement:
    /// - Residual error < 0.02 (2% threshold, tunable via META_TUNING)
    /// - Accuracy delta > 0.05 (5% improvement minimum)
    ///
    /// When cure threshold is met, the update should trigger immediate high-priority gossip
    /// (respecting energy constraints and rate limits).
    ///
    /// **Invariant Safety:**
    /// - INV-1: Cure threshold is independent of reputation (no influence amplification)
    /// - INV-5: Must be checked AFTER energy guard (never bypasses 15% reserve)
    /// - INV-6: Uses f32 but only for threshold comparison (thresholds are constants)
    pub fn cure_threshold(&self) -> bool {
        const RESIDUAL_THRESHOLD: f32 = 0.02; // 2% error threshold
        const ACCURACY_MIN_DELTA: f32 = 0.05; // 5% improvement minimum

        // Both conditions must be met for epidemic "cure" propagation
        self.residual_error < RESIDUAL_THRESHOLD && self.accuracy_delta > ACCURACY_MIN_DELTA
    }

    /// Check if this update is ready for epidemic gossip ("infection")
    ///
    /// This is the viral protocol entry point. It checks:
    /// 1. Cure threshold is met (high-quality update)
    /// 2. Energy pool is sufficient (15% reserve, INV-5)
    ///
    /// \\\
    /// # use qres_core::packet::GhostUpdate;
    /// # use qres_core::zk_proofs::NormProof;
    /// let update = GhostUpdate {
    ///     peer_id: [0u8; 32],
    ///     masked_weights: vec![],
    ///     zk_proof: NormProof { challenge: [0u8; 32], response: 0 },
    ///     dp_epsilon: 0.1,
    ///     residual_error: 0.01,  // Below threshold (good)
    ///     accuracy_delta: 0.08,  // Above threshold (good)
    /// };
    ///
    /// let energy_pool = 0.20; // 20% battery
    /// assert!(update.can_infect(energy_pool));
    ///
    /// let low_energy = 0.10; // 10% battery (too low)
    /// assert!(!update.can_infect(low_energy));
    /// \\\
    ///
    /// **Invariant Safety:**
    /// - INV-5: Energy guard prevents brownouts (hard 15% floor)
    pub fn can_infect(&self, energy_pool: f32) -> bool {
        const ENERGY_RESERVE_THRESHOLD: f32 = 0.15; // 15% minimum (INV-5)

        // Cure quality check
        if !self.cure_threshold() {
            return false;
        }

        // Energy guard (INV-5): Never gossip if battery < 15%
        if energy_pool < ENERGY_RESERVE_THRESHOLD {
            return false;
        }

        true
    }

    /// Get epidemic priority level
    ///
    /// Returns a priority score (0.0 to 1.0) for gossip scheduling.
    /// Higher priority = more urgent epidemic propagation.
    ///
    /// Priority factors:
    /// - Accuracy improvement (higher = more urgent)
    /// - Low residual error (higher quality = more urgent)
    /// - Energy availability (ensure we don't deplete reserves)
    ///
    /// This does NOT bypass rate limits or energy guards; it only affects
    /// ordering within the allowed gossip budget.
    pub fn epidemic_priority(&self, energy_pool: f32) -> f32 {
        if !self.can_infect(energy_pool) {
            return 0.0; // Not eligible for epidemic gossip
        }

        // Priority = accuracy_delta * (1 - residual_error) * energy_factor
        // This ensures high-quality updates with good energy reserves get priority
        let error_quality = (1.0 - self.residual_error.min(1.0)).max(0.0);
        let energy_factor = (energy_pool - 0.15).max(0.0) / 0.85; // Scale above reserve

        (self.accuracy_delta * error_quality * energy_factor).min(1.0)
    }
}

/// Phase 1.3 (v21): Stochastic Auditing for Class C Collusion Detection
///
/// Detects coordinated nodes that submit gradients within trimming bounds but
/// biased in the same direction (e.g., all bias predictions +0.2).
///
/// **Attack Model (Class C):**
/// - Cartel of n ≥ 3 nodes submits gradients within 1.5σ (evades trimming)
/// - All gradients aligned to bias predictions in same direction
/// - Cannot be detected by coordinate-wise trimming alone
///
/// **Defense Protocol:**
/// 1. Randomly audit 3 nodes every 50 rounds when entropy > threshold
/// 2. Challenge nodes to provide raw prediction + proof it matches submitted gradient
/// 3. Verify gradient = hash(raw_prediction, local_data_hash)
/// 4. Punish nodes that fail verification (AuditFailed reason)
///
/// **Privacy Preservation:**
/// - Only raw predictions transmitted (NOT raw data)
/// - Challenge-response within 2 RTT
/// - Optional: ZK-proof that ||grad - f(pred)||_2 < ε
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditChallenge {
    /// Auditor's peer ID (challenger)
    pub auditor_id: [u8; 32],
    /// Challenged node's peer ID
    pub challenged_id: [u8; 32],
    /// Round number being audited
    pub audit_round: u64,
    /// Random nonce to prevent replay attacks
    pub nonce: [u8; 32],
    /// Timestamp of challenge (for timeout detection)
    pub timestamp: u64,
}

/// Response to audit challenge containing raw prediction and proof
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditResponse {
    /// Challenged node's peer ID
    pub peer_id: [u8; 32],
    /// Raw prediction vector (before gradient computation)
    pub raw_prediction: Vec<i32>, // Q16.16 format
    /// Hash of local data used for gradient computation
    pub local_data_hash: [u8; 32],
    /// The gradient that was submitted in audit_round
    pub submitted_gradient: Vec<i32>, // Q16.16 format
    /// Nonce from original challenge (for matching)
    pub nonce: [u8; 32],
    /// Optional: ZK-proof that gradient computation was correct
    pub zk_proof: Option<NormProof>,
}

impl AuditChallenge {
    /// Create a new audit challenge for a specific node and round
    pub fn new(
        auditor_id: [u8; 32],
        challenged_id: [u8; 32],
        audit_round: u64,
        nonce: [u8; 32],
        timestamp: u64,
    ) -> Self {
        Self {
            auditor_id,
            challenged_id,
            audit_round,
            nonce,
            timestamp,
        }
    }

    /// Check if this challenge has expired (timeout = 2 RTT ≈ 10 seconds)
    pub fn is_expired(&self, current_timestamp: u64) -> bool {
        const AUDIT_TIMEOUT_SECONDS: u64 = 10;
        current_timestamp.saturating_sub(self.timestamp) > AUDIT_TIMEOUT_SECONDS
    }
}

impl AuditResponse {
    /// Create a new audit response
    pub fn new(
        peer_id: [u8; 32],
        raw_prediction: Vec<i32>,
        local_data_hash: [u8; 32],
        submitted_gradient: Vec<i32>,
        nonce: [u8; 32],
        zk_proof: Option<NormProof>,
    ) -> Self {
        Self {
            peer_id,
            raw_prediction,
            local_data_hash,
            submitted_gradient,
            nonce,
            zk_proof,
        }
    }

    /// Verify that the submitted gradient matches the claimed raw prediction
    ///
    /// This checks:
    /// 1. Nonce matches (prevents replay attacks)
    /// 2. L2 distance between claimed gradient and recomputed gradient < tolerance
    ///
    /// Returns true if verification passes, false otherwise.
    ///
    /// **Tolerance:** 0.01 in Q16.16 (allows for minor floating-point errors)
    pub fn verify(&self, expected_nonce: &[u8; 32], recomputed_gradient: &[i32]) -> bool {
        // Check nonce match
        if &self.nonce != expected_nonce {
            return false;
        }

        // Check dimension match
        if self.submitted_gradient.len() != recomputed_gradient.len() {
            return false;
        }

        // Compute L2 distance between submitted and recomputed gradients
        let l2_sq: i64 = self
            .submitted_gradient
            .iter()
            .zip(recomputed_gradient.iter())
            .map(|(a, b)| {
                let diff = (*a as i64) - (*b as i64);
                diff * diff
            })
            .sum();

        // Tolerance: 0.01 in Q16.16 = 655 fixed-point units
        // Squared: 655^2 = 429,025
        const TOLERANCE_SQ: i64 = 429_025;

        l2_sq <= TOLERANCE_SQ * (self.submitted_gradient.len() as i64)
    }
}
