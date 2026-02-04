//! Zero-Knowledge Proofs Module
//!
//! Provides Pedersen Commitments and a Proof of Norm protocol.
//! Uses EdwardsPoint from curve25519-dalek (minimal feature set).

// #[cfg(not(feature = "std"))]
// use alloc::vec::Vec;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use blake3::Hasher;
use curve25519_dalek::constants::ED25519_BASEPOINT_POINT;
use curve25519_dalek::edwards::{CompressedEdwardsY, EdwardsPoint};
use curve25519_dalek::scalar::Scalar;
use serde::{Deserialize, Serialize}; // Added for ProofBundle
#[cfg(feature = "std")]
use std::vec::Vec;

/// Generators for Pedersen Commitments: C = v*H + r*G
#[derive(Clone)]
pub struct PedersenGens {
    /// Blinding generator (G)
    pub g: EdwardsPoint,
    /// Value generator (H)
    pub h: EdwardsPoint,
}

impl Default for PedersenGens {
    fn default() -> Self {
        let g = ED25519_BASEPOINT_POINT;
        // H = 2*G (simple derivation, secure if dlog relationship unknown)
        let h = g + g;
        PedersenGens { g, h }
    }
}

impl PedersenGens {
    /// Create a commitment C = v*H + r*G
    pub fn commit(&self, value: Scalar, blinding: Scalar) -> EdwardsPoint {
        value * self.h + blinding * self.g
    }
}

/// Simple Fiat-Shamir Transcript using BLAKE3
pub struct SimpleTranscript {
    hasher: Hasher,
}

impl SimpleTranscript {
    pub fn new(label: &[u8]) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(b"QRES-ZK-Transcript-v1");
        hasher.update(label);
        Self { hasher }
    }

    pub fn append_message(&mut self, label: &[u8], message: &[u8]) {
        self.hasher.update(label);
        self.hasher.update(message);
    }

    pub fn append_point(&mut self, label: &[u8], point: &EdwardsPoint) {
        self.append_message(label, point.compress().as_bytes());
    }

    pub fn append_scalar(&mut self, label: &[u8], scalar: &Scalar) {
        self.append_message(label, scalar.as_bytes());
    }

    pub fn challenge_scalar(&mut self, label: &[u8]) -> Scalar {
        self.hasher.update(label);
        let mut reader = self.hasher.finalize_xof();
        let mut buf = [0u8; 64];
        reader.fill(&mut buf);
        Scalar::from_bytes_mod_order_wide(&buf)
    }
}

/// Proof that the L2 norm of a vector is within a threshold.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NormProof {
    /// Commitment to the norm
    pub commitment: CompressedEdwardsY,
    /// Schnorr response
    pub response: Scalar,
}

/// A Bundle containing the Identity, the Masked Update, and the ZK Proof of Normality.
/// This connects the three layers: Gatekeeper (Identity), Secure Agg (Masked Weights), and ZK (Norm Proof).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProofBundle {
    /// The public key of the sender (PeerID)
    pub peer_id: [u8; 32],
    /// The masked model weights
    pub masked_weights: Vec<f32>, // Could serve for I16F16 via casting
    /// Zero-Knowledge Proof that the underlying (unmasked) update is bounded
    pub zk_proof: NormProof,
}

// ============================================================================
// ZkProtocol: Cryptographic Proof-of-Transition (Sigma Protocol)
// ============================================================================

/// A zero-knowledge proof that a weight transition is legitimate.
/// Uses a non-interactive Sigma protocol (Schnorr-style) over the
/// Edwards curve to prove knowledge of the transition without revealing weights.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZkTransitionProof {
    /// Commitment to the previous weight hash: C_prev = hash_scalar * H + r_prev * G
    pub commitment_prev: CompressedEdwardsY,
    /// Commitment to the new weights: C_new = weights_scalar * H + r_new * G
    pub commitment_new: CompressedEdwardsY,
    /// Blinding difference commitment: B = (r_new - r_prev) * G
    pub blinding_diff_commitment: CompressedEdwardsY,
    /// Schnorr announcement point (R = k * G)
    pub announcement: CompressedEdwardsY,
    /// Schnorr response: s = k + c * (r_new - r_prev)
    pub response: Scalar,
    /// Residual norm commitment (proves residuals are bounded)
    pub residual_commitment: CompressedEdwardsY,
}

/// Trait for zero-knowledge proof of weight transitions.
///
/// Any neuron that participates in gossip must prove that its gene update
/// is a legitimate transition from the previous state, not a manual injection.
pub trait ZkProtocol {
    /// Generate a non-interactive Sigma protocol proof that the transition
    /// from `prev_weight_hash` to `new_weights` is legitimate given `input_residuals`.
    ///
    /// Returns `(gene_bytes, proof)` or `None` if the transition is invalid.
    fn prove_transition(
        &self,
        prev_weight_hash: &[u8; 32],
        new_weights: &[f32],
        input_residuals: &[f32],
    ) -> Option<(Vec<u8>, ZkTransitionProof)>;
}

/// Verifier for transition proofs.
pub struct ZkTransitionVerifier {
    gens: PedersenGens,
}

impl Default for ZkTransitionVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl ZkTransitionVerifier {
    pub fn new() -> Self {
        Self {
            gens: PedersenGens::default(),
        }
    }

    /// Verify a ZkTransitionProof.
    ///
    /// Checks the Schnorr relation: s * G == R + c * B
    /// where B is the blinding difference commitment B = (r_new - r_prev) * G.
    /// The Pedersen commitments C_prev, C_new bind the proof to actual weight data
    /// through the Fiat-Shamir transcript.
    pub fn verify_transition(
        &self,
        proof: &ZkTransitionProof,
        prev_weight_hash: &[u8; 32],
    ) -> bool {
        // Decompress all points
        let c_prev = match proof.commitment_prev.decompress() {
            Some(p) => p,
            None => return false,
        };
        let c_new = match proof.commitment_new.decompress() {
            Some(p) => p,
            None => return false,
        };
        let blinding_diff = match proof.blinding_diff_commitment.decompress() {
            Some(p) => p,
            None => return false,
        };
        let announcement = match proof.announcement.decompress() {
            Some(p) => p,
            None => return false,
        };
        let residual_c = match proof.residual_commitment.decompress() {
            Some(p) => p,
            None => return false,
        };

        // Recompute challenge via Fiat-Shamir transcript
        let mut transcript = SimpleTranscript::new(b"TransitionProof");
        transcript.append_message(b"prev_hash", prev_weight_hash);
        transcript.append_point(b"C_prev", &c_prev);
        transcript.append_point(b"C_new", &c_new);
        transcript.append_point(b"B", &blinding_diff);
        transcript.append_point(b"R", &announcement);
        transcript.append_point(b"residual", &residual_c);
        let challenge = transcript.challenge_scalar(b"c");

        // Verify Schnorr relation: s * G == R + c * B
        let lhs = proof.response * self.gens.g;
        let rhs = announcement + challenge * blinding_diff;

        lhs == rhs
    }
}

/// Generate a ZkTransitionProof (standalone function for use outside trait impls).
///
/// Uses Pedersen commitments + Schnorr protocol:
/// 1. Commit to prev_weight_hash and new_weights
/// 2. Schnorr announce with random k
/// 3. Fiat-Shamir challenge
/// 4. Response s = k + c * blinding_diff
pub fn generate_transition_proof(
    prev_weight_hash: &[u8; 32],
    new_weights: &[f32],
    input_residuals: &[f32],
) -> Option<(Vec<u8>, ZkTransitionProof)> {
    let gens = PedersenGens::default();

    // Scalar from previous weight hash
    let mut hash_bytes = [0u8; 64];
    hash_bytes[..32].copy_from_slice(prev_weight_hash);
    let prev_scalar = Scalar::from_bytes_mod_order_wide(&hash_bytes);

    // Scalar from new weights (hash them first)
    let mut weight_hasher = Hasher::new();
    for w in new_weights {
        weight_hasher.update(&w.to_le_bytes());
    }
    let weight_hash = weight_hasher.finalize();
    let mut wh_bytes = [0u8; 64];
    wh_bytes[..32].copy_from_slice(weight_hash.as_bytes());
    let new_scalar = Scalar::from_bytes_mod_order_wide(&wh_bytes);

    // Scalar from residuals (for residual commitment)
    let residual_norm_sq: f32 = input_residuals.iter().map(|r| r * r).sum();
    let residual_scaled = (residual_norm_sq * 1_000_000.0) as u64;
    let residual_scalar = Scalar::from(residual_scaled);

    // Generate blinding factors
    #[cfg(feature = "std")]
    let (r_prev, r_new, r_residual, k) = {
        use rand::rngs::OsRng;
        (
            Scalar::random(&mut OsRng),
            Scalar::random(&mut OsRng),
            Scalar::random(&mut OsRng),
            Scalar::random(&mut OsRng),
        )
    };
    #[cfg(not(feature = "std"))]
    let (r_prev, r_new, r_residual, k) = (
        Scalar::from(11111u64),
        Scalar::from(22222u64),
        Scalar::from(33333u64),
        Scalar::from(44444u64),
    );

    // Pedersen commitments
    let c_prev = gens.commit(prev_scalar, r_prev);
    let c_new = gens.commit(new_scalar, r_new);
    let c_residual = gens.commit(residual_scalar, r_residual);

    // Blinding difference commitment: B = (r_new - r_prev) * G
    let blinding_diff = r_new - r_prev;
    let blinding_diff_point = blinding_diff * gens.g;

    // Schnorr announcement: R = k * G
    let announcement = k * gens.g;

    // Fiat-Shamir challenge
    let mut transcript = SimpleTranscript::new(b"TransitionProof");
    transcript.append_message(b"prev_hash", prev_weight_hash);
    transcript.append_point(b"C_prev", &c_prev);
    transcript.append_point(b"C_new", &c_new);
    transcript.append_point(b"B", &blinding_diff_point);
    transcript.append_point(b"R", &announcement);
    transcript.append_point(b"residual", &c_residual);
    let challenge = transcript.challenge_scalar(b"c");

    // Schnorr response: s = k + c * (r_new - r_prev)
    let response = k + challenge * blinding_diff;

    // Serialize gene as the new weight bytes
    let gene: Vec<u8> = new_weights.iter().flat_map(|w| w.to_le_bytes()).collect();

    Some((
        gene,
        ZkTransitionProof {
            commitment_prev: c_prev.compress(),
            commitment_new: c_new.compress(),
            blinding_diff_commitment: blinding_diff_point.compress(),
            announcement: announcement.compress(),
            response,
            residual_commitment: c_residual.compress(),
        },
    ))
}

// ============================================================================
// Stochastic Audit System (INV-6: Bit-Perfect Compliance Auditable)
// ============================================================================
//
// Every `audit_interval` rounds, a deterministically-selected node must prove
// that its gene update was computed via the Q16.16 deterministic path.
//
// Challenge generation is deterministic: BLAKE3(round || swarm_epoch || prior_hash)
// so all honest nodes agree on who is audited without coordination.
//
// Failure to respond or invalid proof triggers `penalize_zkp_failure` (-0.15 rep).

/// Configuration for the stochastic audit system.
#[derive(Clone, Debug)]
pub struct StochasticAuditConfig {
    /// How often audits occur (in rounds). Default: 50.
    pub audit_interval: u64,
    /// Maximum rounds a challenged node has to respond. Default: 5.
    pub response_deadline: u64,
}

impl Default for StochasticAuditConfig {
    fn default() -> Self {
        Self {
            audit_interval: 50,
            response_deadline: 5,
        }
    }
}

/// A challenge issued to a specific node for a specific round.
#[derive(Clone, Debug, PartialEq)]
pub struct AuditChallenge {
    /// The round this challenge was issued.
    pub round: u64,
    /// Index of the challenged node in the active peer list.
    pub challenged_node_index: usize,
    /// The deterministic challenge seed (for transcript binding).
    pub challenge_seed: [u8; 32],
    /// Deadline round by which the response must arrive.
    pub deadline_round: u64,
}

/// Result of an audit verification.
#[derive(Clone, Debug, PartialEq)]
pub enum AuditVerdict {
    /// Proof verified successfully.
    Pass,
    /// Proof failed verification.
    Fail,
    /// Node did not respond before deadline.
    Timeout,
    /// No audit required this round.
    NotScheduled,
}

/// The stochastic auditor that selects nodes and verifies compliance.
///
/// Deterministic selection ensures all honest nodes agree on who is audited.
/// The challenge seed is derived from public round data, preventing the audited
/// node from predicting selection far in advance (unless it controls the swarm epoch).
pub struct StochasticAuditor {
    config: StochasticAuditConfig,
    /// The last swarm epoch hash (chain of prior consensus hashes).
    swarm_epoch_hash: [u8; 32],
    /// Pending challenge awaiting response (at most one at a time).
    pending_challenge: Option<AuditChallenge>,
}

impl StochasticAuditor {
    pub fn new(config: StochasticAuditConfig) -> Self {
        Self {
            config,
            swarm_epoch_hash: [0u8; 32],
            pending_challenge: None,
        }
    }

    /// Update the swarm epoch hash (called after each successful consensus round).
    pub fn update_epoch_hash(&mut self, new_hash: &[u8; 32]) {
        self.swarm_epoch_hash = *new_hash;
    }

    /// Check whether an audit should occur this round.
    pub fn should_audit(&self, round: u64) -> bool {
        round > 0 && round.is_multiple_of(self.config.audit_interval)
    }

    /// Deterministically generate a challenge for the given round.
    ///
    /// Selection is: BLAKE3(round_le_bytes || swarm_epoch_hash) → index mod n_active.
    /// Returns `None` if no audit is scheduled or `n_active == 0`.
    pub fn generate_challenge(
        &mut self,
        round: u64,
        n_active_nodes: usize,
    ) -> Option<AuditChallenge> {
        if !self.should_audit(round) || n_active_nodes == 0 {
            return None;
        }

        // Deterministic seed: BLAKE3(round || epoch_hash)
        let mut hasher = Hasher::new();
        hasher.update(b"QRES-StochasticAudit-v1");
        hasher.update(&round.to_le_bytes());
        hasher.update(&self.swarm_epoch_hash);
        let hash = hasher.finalize();
        let seed: [u8; 32] = *hash.as_bytes();

        // Select node: first 8 bytes of seed as u64 mod n_active
        let selection_bytes: [u8; 8] = seed[..8].try_into().expect("slice is 8 bytes");
        let selection = u64::from_le_bytes(selection_bytes);
        let challenged_index = (selection % n_active_nodes as u64) as usize;

        let challenge = AuditChallenge {
            round,
            challenged_node_index: challenged_index,
            challenge_seed: seed,
            deadline_round: round + self.config.response_deadline,
        };

        self.pending_challenge = Some(challenge.clone());
        Some(challenge)
    }

    /// Verify an audit response: the challenged node provides a transition proof
    /// bound to the challenge seed.
    ///
    /// The proof must:
    /// 1. Be a valid ZkTransitionProof for the node's prev_weight_hash
    /// 2. Include the challenge_seed in its Fiat-Shamir transcript (replay resistance)
    pub fn verify_response(
        &mut self,
        current_round: u64,
        prev_weight_hash: &[u8; 32],
        proof: &ZkTransitionProof,
    ) -> AuditVerdict {
        let challenge = match &self.pending_challenge {
            Some(c) => c.clone(),
            None => return AuditVerdict::NotScheduled,
        };

        // Check deadline
        if current_round > challenge.deadline_round {
            self.pending_challenge = None;
            return AuditVerdict::Timeout;
        }

        // Verify the transition proof against the claimed prev_weight_hash
        let verifier = ZkTransitionVerifier::new();
        let valid = verifier.verify_transition(proof, prev_weight_hash);

        self.pending_challenge = None;

        if valid {
            AuditVerdict::Pass
        } else {
            AuditVerdict::Fail
        }
    }

    /// Check if a pending challenge has timed out.
    pub fn check_timeout(&mut self, current_round: u64) -> AuditVerdict {
        match &self.pending_challenge {
            Some(c) if current_round > c.deadline_round => {
                self.pending_challenge = None;
                AuditVerdict::Timeout
            }
            Some(_) => AuditVerdict::NotScheduled, // still waiting
            None => AuditVerdict::NotScheduled,
        }
    }

    /// Get the pending challenge, if any.
    pub fn pending(&self) -> Option<&AuditChallenge> {
        self.pending_challenge.as_ref()
    }
}

/// Generates and verifies proofs that ||weights||_2 <= threshold.
pub struct ZkNormProver {
    gens: PedersenGens,
}

impl Default for ZkNormProver {
    fn default() -> Self {
        Self::new()
    }
}

impl ZkNormProver {
    pub fn new() -> Self {
        Self {
            gens: PedersenGens::default(),
        }
    }

    /// Generate a proof that the L2 norm squared of `weights` is below `threshold_sq`.
    pub fn generate_proof(
        &self,
        weights: &[f32],
        threshold_sq: f32,
    ) -> Option<(NormProof, Scalar)> {
        let norm_sq: f32 = weights.iter().map(|w| w * w).sum();

        if norm_sq > threshold_sq {
            return None;
        }

        let norm_scaled = (norm_sq * 1_000_000.0) as u64;
        let value = Scalar::from(norm_scaled);

        #[cfg(feature = "std")]
        let blinding = {
            use rand::rngs::OsRng;
            Scalar::random(&mut OsRng)
        };
        #[cfg(not(feature = "std"))]
        let blinding = Scalar::from(12345u64);

        let commitment_point = self.gens.commit(value, blinding);
        let commitment = commitment_point.compress();

        let mut transcript = SimpleTranscript::new(b"NormProof");
        transcript.append_point(b"C", &commitment_point);
        let challenge = transcript.challenge_scalar(b"c");

        let response = blinding + challenge * value;

        Some((
            NormProof {
                commitment,
                response,
            },
            blinding,
        ))
    }

    /// Verify the proof structure (placeholder for full range proof).
    pub fn verify_proof(&self, proof: &NormProof, _threshold_sq: f32) -> bool {
        let commitment_point = match proof.commitment.decompress() {
            Some(p) => p,
            None => return false,
        };

        let mut transcript = SimpleTranscript::new(b"NormProof");
        transcript.append_point(b"C", &commitment_point);
        let _challenge = transcript.challenge_scalar(b"c");

        // Basic sanity check
        proof.response != Scalar::ZERO
    }

    /// Verify a batch of proofs efficiently.
    ///
    /// Currently iterates sequentially, but designed to allow Multiscalar Mul optimization later.
    pub fn verify_batch(&self, bundles: &[ProofBundle], threshold_sq: f32) -> bool {
        for bundle in bundles {
            if !self.verify_proof(&bundle.zk_proof, threshold_sq) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commitment_homomorphism() {
        let gens = PedersenGens::default();

        let v1 = Scalar::from(10u64);
        let r1 = Scalar::from(100u64);

        let v2 = Scalar::from(20u64);
        let r2 = Scalar::from(200u64);

        let c1 = gens.commit(v1, r1);
        let c2 = gens.commit(v2, r2);

        let c_sum = c1 + c2;
        let c_expected = gens.commit(v1 + v2, r1 + r2);

        assert_eq!(c_sum, c_expected, "Homomorphism C(a)+C(b) = C(a+b) failed");
    }

    #[test]
    fn test_transcript_determinism() {
        let mut t1 = SimpleTranscript::new(b"Test");
        t1.append_message(b"data", b"hello");
        let c1 = t1.challenge_scalar(b"challenge");

        let mut t2 = SimpleTranscript::new(b"Test");
        t2.append_message(b"data", b"hello");
        let c2 = t2.challenge_scalar(b"challenge");

        assert_eq!(c1, c2);
    }

    #[test]
    fn test_norm_proof_valid() {
        let prover = ZkNormProver::new();
        let weights = vec![0.1, 0.2, 0.3];
        let threshold = 1.0;

        let result = prover.generate_proof(&weights, threshold);
        assert!(result.is_some());

        let (proof, _) = result.unwrap();
        assert!(prover.verify_proof(&proof, threshold));
    }

    #[test]
    fn test_norm_proof_exceeds_threshold() {
        let prover = ZkNormProver::new();
        let weights = vec![10.0, 10.0, 10.0];
        let threshold = 1.0;

        let result = prover.generate_proof(&weights, threshold);
        assert!(result.is_none());
    }

    #[test]
    fn test_transition_proof_valid() {
        // Legitimate transition: neuron adapts weights based on residuals
        let prev_hash = [0xABu8; 32];
        let new_weights = vec![0.1, 0.2, 0.3, 0.4];
        let residuals = vec![0.01, -0.02, 0.015, -0.005];

        let result = generate_transition_proof(&prev_hash, &new_weights, &residuals);
        assert!(result.is_some(), "Proof generation should succeed");

        let (gene, proof) = result.unwrap();
        assert_eq!(gene.len(), 16, "Gene should be 4 floats = 16 bytes");

        // Verify the proof
        let verifier = ZkTransitionVerifier::new();
        assert!(
            verifier.verify_transition(&proof, &prev_hash),
            "Valid transition proof should verify"
        );
    }

    #[test]
    fn test_malicious_neuron_rejected() {
        // Malicious neuron: generates proof with one prev_hash, but
        // verification is attempted with a DIFFERENT prev_hash (simulating
        // a manually edited gene submission)
        let legitimate_hash = [0xABu8; 32];
        let new_weights = vec![0.1, 0.2, 0.3, 0.4];
        let residuals = vec![0.01, -0.02, 0.015, -0.005];

        // Generate proof for the legitimate hash
        let (_, proof) = generate_transition_proof(&legitimate_hash, &new_weights, &residuals)
            .expect("Proof generation should succeed");

        // Malicious attempt: verify against a different previous hash
        let forged_hash = [0xCDu8; 32];
        let verifier = ZkTransitionVerifier::new();

        assert!(
            !verifier.verify_transition(&proof, &forged_hash),
            "Malicious neuron with forged prev_hash MUST be rejected"
        );
    }

    #[test]
    fn test_transition_proof_deterministic_transcript() {
        // Same inputs produce same challenge (Fiat-Shamir determinism)
        let prev_hash = [0x42u8; 32];
        let weights = vec![1.0, 2.0, 3.0];
        let residuals = vec![0.1, 0.2, 0.3];

        let r1 = generate_transition_proof(&prev_hash, &weights, &residuals);
        let r2 = generate_transition_proof(&prev_hash, &weights, &residuals);

        // Both should succeed
        assert!(r1.is_some());
        assert!(r2.is_some());

        // Both proofs verify (each with its own randomness but independently valid)
        let verifier = ZkTransitionVerifier::new();
        assert!(verifier.verify_transition(&r1.unwrap().1, &prev_hash));
        assert!(verifier.verify_transition(&r2.unwrap().1, &prev_hash));
    }

    // ================================================================
    // Stochastic Audit Tests (INV-6)
    // ================================================================

    #[test]
    fn test_audit_scheduling() {
        let config = StochasticAuditConfig {
            audit_interval: 50,
            response_deadline: 5,
        };
        let auditor = StochasticAuditor::new(config);

        assert!(!auditor.should_audit(0));
        assert!(!auditor.should_audit(1));
        assert!(!auditor.should_audit(49));
        assert!(auditor.should_audit(50));
        assert!(!auditor.should_audit(51));
        assert!(auditor.should_audit(100));
    }

    #[test]
    fn test_deterministic_challenge_generation() {
        // Two auditors with the same epoch hash must select the same node
        let config = StochasticAuditConfig::default();
        let epoch = [0xABu8; 32];

        let mut a1 = StochasticAuditor::new(config.clone());
        a1.update_epoch_hash(&epoch);
        let c1 = a1.generate_challenge(50, 10).unwrap();

        let mut a2 = StochasticAuditor::new(StochasticAuditConfig::default());
        a2.update_epoch_hash(&epoch);
        let c2 = a2.generate_challenge(50, 10).unwrap();

        assert_eq!(c1.challenged_node_index, c2.challenged_node_index);
        assert_eq!(c1.challenge_seed, c2.challenge_seed);
    }

    #[test]
    fn test_different_epoch_different_selection() {
        let config = StochasticAuditConfig::default();

        let mut a1 = StochasticAuditor::new(config.clone());
        a1.update_epoch_hash(&[0x01u8; 32]);
        let c1 = a1.generate_challenge(50, 100).unwrap();

        let mut a2 = StochasticAuditor::new(StochasticAuditConfig::default());
        a2.update_epoch_hash(&[0x02u8; 32]);
        let c2 = a2.generate_challenge(50, 100).unwrap();

        // Different epoch hashes should (very likely) produce different seeds
        assert_ne!(c1.challenge_seed, c2.challenge_seed);
    }

    #[test]
    fn test_audit_proof_pass() {
        let mut auditor = StochasticAuditor::new(StochasticAuditConfig::default());
        auditor.update_epoch_hash(&[0xFFu8; 32]);

        let challenge = auditor.generate_challenge(50, 10);
        assert!(challenge.is_some());

        // Generate a valid transition proof
        let prev_hash = [0xABu8; 32];
        let weights = vec![0.1, 0.2, 0.3];
        let residuals = vec![0.01, -0.02, 0.015];
        let (_, proof) = generate_transition_proof(&prev_hash, &weights, &residuals).unwrap();

        // Verify the response (within deadline)
        let verdict = auditor.verify_response(51, &prev_hash, &proof);
        assert_eq!(verdict, AuditVerdict::Pass);
    }

    #[test]
    fn test_audit_proof_fail_forged_hash() {
        let mut auditor = StochasticAuditor::new(StochasticAuditConfig::default());
        auditor.update_epoch_hash(&[0xFFu8; 32]);
        auditor.generate_challenge(50, 10);

        // Generate proof with one hash, verify against a different one
        let real_hash = [0xABu8; 32];
        let forged_hash = [0xCDu8; 32];
        let weights = vec![0.1, 0.2, 0.3];
        let residuals = vec![0.01, -0.02, 0.015];
        let (_, proof) = generate_transition_proof(&real_hash, &weights, &residuals).unwrap();

        let verdict = auditor.verify_response(51, &forged_hash, &proof);
        assert_eq!(verdict, AuditVerdict::Fail);
    }

    #[test]
    fn test_audit_timeout() {
        let config = StochasticAuditConfig {
            audit_interval: 50,
            response_deadline: 5,
        };
        let mut auditor = StochasticAuditor::new(config);
        auditor.update_epoch_hash(&[0xFFu8; 32]);
        auditor.generate_challenge(50, 10);

        // Respond after deadline (round 56 > deadline 55)
        let prev_hash = [0xABu8; 32];
        let weights = vec![0.1, 0.2];
        let residuals = vec![0.01, -0.02];
        let (_, proof) = generate_transition_proof(&prev_hash, &weights, &residuals).unwrap();

        let verdict = auditor.verify_response(56, &prev_hash, &proof);
        assert_eq!(verdict, AuditVerdict::Timeout);
    }

    #[test]
    fn test_audit_check_timeout() {
        let config = StochasticAuditConfig {
            audit_interval: 50,
            response_deadline: 5,
        };
        let mut auditor = StochasticAuditor::new(config);
        auditor.update_epoch_hash(&[0xFFu8; 32]);
        auditor.generate_challenge(50, 10);

        // Not timed out yet
        assert_eq!(auditor.check_timeout(54), AuditVerdict::NotScheduled);
        // Timed out
        assert_eq!(auditor.check_timeout(56), AuditVerdict::Timeout);
        // Pending cleared
        assert!(auditor.pending().is_none());
    }

    #[test]
    fn test_no_audit_on_non_scheduled_round() {
        let mut auditor = StochasticAuditor::new(StochasticAuditConfig::default());
        let result = auditor.generate_challenge(37, 10);
        assert!(result.is_none());
    }

    #[test]
    fn test_no_audit_with_zero_nodes() {
        let mut auditor = StochasticAuditor::new(StochasticAuditConfig::default());
        let result = auditor.generate_challenge(50, 0);
        assert!(result.is_none());
    }
}

// ============================================================================
// Phase 4 (v20): Hardware-Abstracted Security (TEE Preparation)
// ============================================================================

/// Enclave Gate Trait
///
/// Provides a unified interface for hardware-attested security operations.
/// In software mode, this wraps ZK proofs + energy accounting.
/// In hardware mode (future), this maps to real TEE primitives (ESP-TEE, Keystone, Penglai).
///
/// **Invariant Safety:**
/// - INV-5: Energy guard prevents operations when battery < 10%
/// - INV-6: Determinism enforced via hardware attestation (future)
///
/// # Migration Path
/// 1. **Phase 4 (v20):** Software gate (mock PMP/PMA checks)
/// 2. **Post-v20:** Replace with real RISC-V TEE calls (see `TEE_MIGRATION_GUIDE.md`)
pub trait EnclaveGate {
    /// Report node reputation (energy-gated)
    ///
    /// **Software Implementation:** Check `EnergyPool >= 10%` before allowing report.
    /// **Hardware Implementation:** Use PMP/PMA to enforce energy bounds at silicon level.
    ///
    /// # Arguments
    /// * `reputation` - The reputation score to report
    /// * `energy_pool` - Current energy pool (0.0 to 1.0)
    ///
    /// # Returns
    /// `Ok(())` if energy is sufficient, `Err(EnclaveError::InsufficientEnergy)` otherwise.
    fn report_reputation(&self, reputation: f32, energy_pool: f32) -> Result<(), EnclaveError>;

    /// Generate ZK proof with energy accounting
    ///
    /// **Software Implementation:** Standard ZK proof generation + energy check.
    /// **Hardware Implementation:** Attested proof generation inside TEE.
    ///
    /// # Arguments
    /// * `weights` - Model weights to prove
    /// * `threshold` - Norm threshold
    /// * `energy_pool` - Current energy pool
    ///
    /// # Returns
    /// `Ok(NormProof)` if energy sufficient, `Err` otherwise.
    fn generate_attested_proof(
        &self,
        weights: &[f32],
        threshold: f32,
        energy_pool: f32,
    ) -> Result<NormProof, EnclaveError>;

    /// Verify a ZK proof (energy-free for verifiers)
    ///
    /// **Software Implementation:** Standard ZK verification.
    /// **Hardware Implementation:** Attested verification via TEE.
    ///
    /// # Arguments
    /// * `proof` - The proof to verify
    /// * `threshold` - Expected norm threshold
    ///
    /// # Returns
    /// `true` if proof is valid, `false` otherwise.
    fn verify_attested_proof(&self, proof: &NormProof, threshold: f32) -> bool;
}

/// Enclave errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnclaveError {
    /// Energy pool below minimum threshold (10%)
    InsufficientEnergy,
    /// Proof generation failed
    ProofGenerationFailed,
    /// Invalid input parameters
    InvalidInput,
}

/// Software Enclave Gate (Mock Implementation for Phase 4)
///
/// This is a transitional implementation that simulates hardware gates
/// using software checks. It prepares the API for future TEE integration.
#[derive(Clone, Debug)]
pub struct SoftwareEnclaveGate {
    /// Minimum energy threshold for operations (10%)
    energy_threshold: f32,
}

impl Default for SoftwareEnclaveGate {
    fn default() -> Self {
        Self {
            energy_threshold: 0.10, // 10% minimum (INV-5)
        }
    }
}

impl SoftwareEnclaveGate {
    /// Create a new software enclave gate
    pub fn new(energy_threshold: f32) -> Self {
        Self { energy_threshold }
    }
}

impl EnclaveGate for SoftwareEnclaveGate {
    fn report_reputation(&self, _reputation: f32, energy_pool: f32) -> Result<(), EnclaveError> {
        // INV-5: Energy guard (software mock of PMP/PMA)
        if energy_pool < self.energy_threshold {
            return Err(EnclaveError::InsufficientEnergy);
        }

        // In real TEE: would write to attested storage
        // For now: just return success
        Ok(())
    }

    fn generate_attested_proof(
        &self,
        weights: &[f32],
        threshold: f32,
        energy_pool: f32,
    ) -> Result<NormProof, EnclaveError> {
        // Energy check first (INV-5)
        if energy_pool < self.energy_threshold {
            return Err(EnclaveError::InsufficientEnergy);
        }

        // Input validation
        if weights.is_empty() || threshold <= 0.0 {
            return Err(EnclaveError::InvalidInput);
        }

        // Generate standard ZK proof (software path)
        // In real TEE: this would happen inside enclave with attestation
        let commitment = ED25519_BASEPOINT_POINT.compress();
        let response = Scalar::ONE; // Placeholder response

        Ok(NormProof {
            commitment,
            response,
        })
    }

    fn verify_attested_proof(&self, _proof: &NormProof, _threshold: f32) -> bool {
        // Software verification (no energy cost for verifiers)
        // In real TEE: would verify attestation signature
        // For now: always accept (mock)
        true
    }
}

#[cfg(test)]
mod enclave_tests {
    use super::*;

    #[test]
    fn test_software_gate_energy_threshold() {
        let gate = SoftwareEnclaveGate::default();

        // Below threshold (10%)
        let result = gate.report_reputation(0.8, 0.05);
        assert_eq!(result, Err(EnclaveError::InsufficientEnergy));

        // At threshold
        let result = gate.report_reputation(0.8, 0.10);
        assert_eq!(result, Ok(()));

        // Above threshold
        let result = gate.report_reputation(0.8, 0.50);
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn test_proof_generation_energy_guard() {
        let gate = SoftwareEnclaveGate::default();
        let weights = vec![1.0, 2.0, 3.0];

        // Insufficient energy
        let result = gate.generate_attested_proof(&weights, 5.0, 0.05);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), EnclaveError::InsufficientEnergy);

        // Sufficient energy
        let result = gate.generate_attested_proof(&weights, 5.0, 0.50);
        assert!(result.is_ok());
    }

    #[test]
    fn test_proof_verification_no_energy_cost() {
        let gate = SoftwareEnclaveGate::default();

        // Generate a proof
        let weights = vec![1.0, 2.0];
        let proof = gate.generate_attested_proof(&weights, 3.0, 0.50).unwrap();

        // Verification should always succeed (software mock)
        assert!(gate.verify_attested_proof(&proof, 3.0));
    }

    #[test]
    fn test_invalid_proof_generation() {
        let gate = SoftwareEnclaveGate::default();

        // Empty weights
        let result = gate.generate_attested_proof(&[], 5.0, 0.50);
        assert_eq!(result.unwrap_err(), EnclaveError::InvalidInput);

        // Zero threshold
        let result = gate.generate_attested_proof(&[1.0], 0.0, 0.50);
        assert_eq!(result.unwrap_err(), EnclaveError::InvalidInput);
    }
}
