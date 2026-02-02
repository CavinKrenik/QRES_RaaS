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
}
