//! Stochastic Auditing Module for Class C Collusion Detection (v21.0)
//!
//! This module implements the ZK-Compliance Tax protocol to detect coordinated
//! nodes that submit gradients within trimming bounds but biased in the same direction.
//!
//! **Attack Model (Class C Collusion):**
//! - Cartel of n ≥ 3 nodes submits gradients within 1.5σ (evades trimming)
//! - All gradients aligned to bias predictions in same direction
//! - Cannot be detected by coordinate-wise trimming alone
//!
//! **Defense Protocol:**
//! 1. Randomly audit 3 nodes every 50 rounds when entropy > threshold
//! 2. Challenge nodes to provide raw prediction + proof it matches submitted gradient
//! 3. Verify gradient = hash(raw_prediction, local_data_hash)
//! 4. Punish nodes that fail verification (AuditFailed reason)

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::packet::AuditChallenge;
use crate::reputation::PeerId;
use blake3::Hasher;

/// Configuration for the collusion-detection audit system
#[derive(Clone, Debug)]
pub struct CollisionAuditConfig {
    /// How often audits occur (in rounds). Default: 50.
    pub audit_interval: u64,
    /// Number of nodes to audit each interval. Default: 3.
    pub nodes_per_audit: usize,
    /// Minimum entropy threshold to trigger audits (spam protection).
    /// Default: 0.3 (only audit during actual activity).
    pub entropy_threshold: f32,
    /// Response timeout in seconds. Default: 10 (2 RTT).
    pub response_timeout_seconds: u64,
}

impl Default for CollisionAuditConfig {
    fn default() -> Self {
        Self {
            audit_interval: 50,
            nodes_per_audit: 3,
            entropy_threshold: 0.3,
            response_timeout_seconds: 10,
        }
    }
}

/// The collusion auditor that selects nodes and verifies gradient authenticity
///
/// Deterministic selection ensures all honest nodes agree on who is audited.
/// The challenge nonce is derived from public round data, preventing prediction.
pub struct CollisionAuditor {
    config: CollisionAuditConfig,
    /// The last consensus epoch hash (for deterministic nonce generation)
    epoch_hash: [u8; 32],
}

impl CollisionAuditor {
    /// Create a new collusion auditor with default configuration
    pub fn new(config: CollisionAuditConfig) -> Self {
        Self {
            config,
            epoch_hash: [0u8; 32],
        }
    }

    /// Update the epoch hash after each consensus round
    pub fn update_epoch_hash(&mut self, new_hash: &[u8; 32]) {
        self.epoch_hash = *new_hash;
    }

    /// Check if audits should occur this round
    ///
    /// Audits are triggered when:
    /// 1. Round number is a multiple of audit_interval
    /// 2. Current entropy exceeds threshold (indicates activity)
    pub fn should_audit(&self, round: u64, current_entropy: f32) -> bool {
        if round == 0 {
            return false;
        }

        let is_audit_round = round % self.config.audit_interval == 0;
        let has_activity = current_entropy >= self.config.entropy_threshold;

        is_audit_round && has_activity
    }

    /// Deterministically select nodes to audit for this round
    ///
    /// Selection algorithm:
    /// 1. Generate seed: BLAKE3("QRES-CollusionAudit-v21" || round || epoch_hash)
    /// 2. For each audit slot (0..nodes_per_audit):
    ///    - Hash(seed || slot_index) → select node index mod n_active
    ///
    /// This ensures:
    /// - All honest nodes agree on selection (deterministic)
    /// - Cannot be predicted in advance (depends on recent epoch_hash)
    /// - No coordination needed (pure function of public data)
    ///
    /// Returns a list of (peer_id, audit_challenge) pairs
    pub fn generate_challenges(
        &self,
        round: u64,
        current_entropy: f32,
        active_peers: &[PeerId],
        current_timestamp: u64,
    ) -> Vec<(PeerId, AuditChallenge)> {
        if !self.should_audit(round, current_entropy) || active_peers.is_empty() {
            return Vec::new();
        }

        // Generate base seed for this audit round
        let mut hasher = Hasher::new();
        hasher.update(b"QRES-CollusionAudit-v21");
        hasher.update(&round.to_le_bytes());
        hasher.update(&self.epoch_hash);
        let base_seed = hasher.finalize();

        let mut challenges = Vec::new();
        let n_active = active_peers.len();
        let audit_count = self.config.nodes_per_audit.min(n_active);

        for slot in 0..audit_count {
            // Generate slot-specific nonce
            let mut slot_hasher = Hasher::new();
            slot_hasher.update(base_seed.as_bytes());
            slot_hasher.update(&(slot as u64).to_le_bytes());
            let slot_hash = slot_hasher.finalize();
            let nonce: [u8; 32] = *slot_hash.as_bytes();

            // Select node index
            let selection_bytes: [u8; 8] = nonce[..8].try_into().expect("slice is 8 bytes");
            let selection = u64::from_le_bytes(selection_bytes);
            let challenged_index = (selection % n_active as u64) as usize;
            let challenged_id = active_peers[challenged_index];

            // Auditor ID is derived from epoch (ensures consistency)
            let auditor_id = self.derive_auditor_id(round);

            let challenge =
                AuditChallenge::new(auditor_id, challenged_id, round, nonce, current_timestamp);

            challenges.push((challenged_id, challenge));
        }

        challenges
    }

    /// Derive a deterministic auditor ID for this round
    ///
    /// Uses epoch hash to generate a consistent "auditor" identity.
    /// In practice, all nodes verify audits, but this provides a canonical ID.
    fn derive_auditor_id(&self, round: u64) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"QRES-AuditorID-v21");
        hasher.update(&round.to_le_bytes());
        hasher.update(&self.epoch_hash);
        *hasher.finalize().as_bytes()
    }

    /// Get the audit rate as a fraction of total nodes
    ///
    /// For example, with 3 audits per 50 rounds and 150 nodes:
    /// Rate = 3/150 = 0.02 = 2% bandwidth overhead
    pub fn audit_rate(&self, n_active_nodes: usize) -> f32 {
        if n_active_nodes == 0 {
            return 0.0;
        }
        self.config.nodes_per_audit as f32 / n_active_nodes as f32
    }

    /// Calculate expected detection rounds for a cartel of size n
    ///
    /// If auditing `k` nodes per `I` rounds out of `N` total active nodes,
    /// the probability of catching at least one cartel member per audit is:
    ///
    /// P(detect) = 1 - [(N-n)/N * (N-n-1)/(N-1) * ... * (N-n-k+1)/(N-k+1)]
    ///
    /// Expected rounds to detection ≈ I / P(detect)
    ///
    /// For k=3, I=50, N=150, n=5:
    /// P(detect) ≈ 0.095 → E[rounds] ≈ 526 rounds
    pub fn expected_detection_rounds(&self, n_active: usize, cartel_size: usize) -> f32 {
        if cartel_size == 0 || n_active == 0 || cartel_size > n_active {
            return f32::INFINITY;
        }

        let k = self.config.nodes_per_audit.min(n_active);
        let n = n_active as f32;
        let m = cartel_size as f32;

        // Probability of missing all cartel members in one audit
        let mut p_miss = 1.0;
        for i in 0..k {
            let honest_remaining = n - m - i as f32;
            let total_remaining = n - i as f32;
            p_miss *= honest_remaining / total_remaining;
        }

        let p_detect = 1.0 - p_miss;

        if p_detect <= 0.0 {
            return f32::INFINITY;
        }

        // Expected audits to detection * interval between audits
        (1.0 / p_detect) * self.config.audit_interval as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_scheduling() {
        let config = CollisionAuditConfig {
            audit_interval: 50,
            nodes_per_audit: 3,
            entropy_threshold: 0.3,
            response_timeout_seconds: 10,
        };
        let auditor = CollisionAuditor::new(config);

        // Not an audit round
        assert!(!auditor.should_audit(0, 0.5));
        assert!(!auditor.should_audit(1, 0.5));
        assert!(!auditor.should_audit(49, 0.5));

        // Audit round but entropy too low
        assert!(!auditor.should_audit(50, 0.2));

        // Valid audit round
        assert!(auditor.should_audit(50, 0.5));
        assert!(auditor.should_audit(100, 0.5));
        assert!(auditor.should_audit(150, 1.0));
    }

    #[test]
    fn test_generate_challenges_deterministic() {
        let config = CollisionAuditConfig::default();
        let mut auditor = CollisionAuditor::new(config);
        let epoch_hash = [0xABu8; 32];
        auditor.update_epoch_hash(&epoch_hash);

        let active_peers = vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], [5u8; 32]];

        let challenges1 = auditor.generate_challenges(50, 0.5, &active_peers, 1000);
        let challenges2 = auditor.generate_challenges(50, 0.5, &active_peers, 1000);

        // Should be deterministic (same inputs = same outputs)
        assert_eq!(challenges1.len(), challenges2.len());
        for (c1, c2) in challenges1.iter().zip(challenges2.iter()) {
            assert_eq!(c1.0, c2.0); // Same peer selected
            assert_eq!(c1.1.nonce, c2.1.nonce); // Same nonce
        }
    }

    #[test]
    fn test_generate_challenges_count() {
        let config = CollisionAuditConfig {
            audit_interval: 50,
            nodes_per_audit: 3,
            entropy_threshold: 0.3,
            response_timeout_seconds: 10,
        };
        let auditor = CollisionAuditor::new(config);

        let active_peers = vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], [5u8; 32]];

        let challenges = auditor.generate_challenges(50, 0.5, &active_peers, 1000);

        // Should generate 3 challenges (nodes_per_audit)
        assert_eq!(challenges.len(), 3);

        // Each challenge should have unique nonce
        let nonce1 = challenges[0].1.nonce;
        let nonce2 = challenges[1].1.nonce;
        let nonce3 = challenges[2].1.nonce;

        assert_ne!(nonce1, nonce2);
        assert_ne!(nonce2, nonce3);
        assert_ne!(nonce1, nonce3);
    }

    #[test]
    fn test_generate_challenges_respects_entropy() {
        let config = CollisionAuditConfig {
            audit_interval: 50,
            nodes_per_audit: 3,
            entropy_threshold: 0.5,
            response_timeout_seconds: 10,
        };
        let auditor = CollisionAuditor::new(config);

        let active_peers = vec![[1u8; 32], [2u8; 32], [3u8; 32]];

        // Entropy below threshold → no challenges
        let challenges_low = auditor.generate_challenges(50, 0.3, &active_peers, 1000);
        assert_eq!(challenges_low.len(), 0);

        // Entropy above threshold → generate challenges
        let challenges_high = auditor.generate_challenges(50, 0.6, &active_peers, 1000);
        assert_eq!(challenges_high.len(), 3);
    }

    #[test]
    fn test_audit_rate_calculation() {
        let config = CollisionAuditConfig {
            audit_interval: 50,
            nodes_per_audit: 3,
            entropy_threshold: 0.3,
            response_timeout_seconds: 10,
        };
        let auditor = CollisionAuditor::new(config);

        // 3 audits per round with 150 nodes = 2% rate
        assert_eq!(auditor.audit_rate(150), 0.02);

        // 3 audits per round with 100 nodes = 3% rate
        assert_eq!(auditor.audit_rate(100), 0.03);

        // Edge case: 0 nodes
        assert_eq!(auditor.audit_rate(0), 0.0);
    }

    #[test]
    fn test_expected_detection_rounds() {
        let config = CollisionAuditConfig {
            audit_interval: 50,
            nodes_per_audit: 3,
            entropy_threshold: 0.3,
            response_timeout_seconds: 10,
        };
        let auditor = CollisionAuditor::new(config);

        // 5-node cartel in 150 nodes
        // P(detect) ≈ 0.095 per audit
        // Expected: ~10.5 audits × 50 rounds = ~526 rounds
        let rounds = auditor.expected_detection_rounds(150, 5);
        assert!(rounds > 400.0 && rounds < 600.0);

        // 10-node cartel (larger) → faster detection
        let rounds_large = auditor.expected_detection_rounds(150, 10);
        assert!(rounds_large < rounds);

        // Edge cases
        assert!(auditor.expected_detection_rounds(0, 5).is_infinite());
        assert!(auditor.expected_detection_rounds(150, 0).is_infinite());
        assert!(auditor.expected_detection_rounds(150, 200).is_infinite());
    }

    #[test]
    fn test_epoch_hash_affects_selection() {
        let config = CollisionAuditConfig::default();
        let mut auditor1 = CollisionAuditor::new(config.clone());
        let mut auditor2 = CollisionAuditor::new(config);

        let epoch1 = [0xABu8; 32];
        let epoch2 = [0xCDu8; 32];

        auditor1.update_epoch_hash(&epoch1);
        auditor2.update_epoch_hash(&epoch2);

        let active_peers = vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], [5u8; 32]];

        let challenges1 = auditor1.generate_challenges(50, 0.5, &active_peers, 1000);
        let challenges2 = auditor2.generate_challenges(50, 0.5, &active_peers, 1000);

        // Different epochs should (likely) produce different selections
        // Note: There's a small chance they could be the same by random chance
        assert_eq!(challenges1.len(), challenges2.len());

        // At least one selection should differ
        let mut differs = false;
        for (c1, c2) in challenges1.iter().zip(challenges2.iter()) {
            if c1.0 != c2.0 || c1.1.nonce != c2.1.nonce {
                differs = true;
                break;
            }
        }
        assert!(
            differs,
            "Different epochs should produce different selections"
        );
    }
}
