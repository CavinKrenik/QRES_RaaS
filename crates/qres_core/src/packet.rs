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
