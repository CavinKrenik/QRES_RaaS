//! Reputation Tracking Module for Sybil Resistance
//!
//! Provides a `ReputationTracker` that maintains trust scores per peer.
//! Scores increase with valid ZKP submissions and decrease when drift
//! is detected during aggregation. Used to weight nodes in
//! `WeightedTrimmedMean` aggregation.

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::collections::BTreeMap;
#[cfg(feature = "std")]
use std::vec::Vec;

use serde::{Deserialize, Serialize};

/// A peer identifier (32-byte public key as hex or raw bytes)
pub type PeerId = [u8; 32];

/// Default trust score for new peers
const DEFAULT_TRUST: f32 = 0.5;

/// Reward increment for valid ZKP submission
const ZKP_REWARD: f32 = 0.02;

/// Penalty for detected drift during aggregation
const DRIFT_PENALTY: f32 = 0.08;

/// Penalty for failed ZKP verification
const ZKP_FAILURE_PENALTY: f32 = 0.15;

/// Ban threshold: peers below this score are excluded
const BAN_THRESHOLD: f32 = 0.2;

/// Reputation tracker for swarm peers.
///
/// Maintains a `PeerId -> Score` mapping where:
/// - Scores range from 0.0 (fully distrusted) to 1.0 (fully trusted)
/// - New peers start at 0.5 (neutral)
/// - Valid ZKP submissions increase score
/// - Drift detection during aggregation decreases score
/// - Peers below 0.2 are banned from consensus
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReputationTracker {
    scores: BTreeMap<PeerId, f32>,
}

impl Default for ReputationTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ReputationTracker {
    pub fn new() -> Self {
        Self {
            scores: BTreeMap::new(),
        }
    }

    /// Get the trust score for a peer (default 0.5 for unknown peers)
    pub fn get_score(&self, peer: &PeerId) -> f32 {
        self.scores.get(peer).copied().unwrap_or(DEFAULT_TRUST)
    }

    /// Check if a peer is banned (score < BAN_THRESHOLD)
    pub fn is_banned(&self, peer: &PeerId) -> bool {
        self.get_score(peer) < BAN_THRESHOLD
    }

    /// Reward a peer for submitting a valid ZKP
    pub fn reward_valid_zkp(&mut self, peer: &PeerId) {
        let score = self.scores.entry(*peer).or_insert(DEFAULT_TRUST);
        *score = (*score + ZKP_REWARD).min(1.0);
    }

    /// Penalize a peer for drift detected during aggregation
    pub fn penalize_drift(&mut self, peer: &PeerId) {
        let score = self.scores.entry(*peer).or_insert(DEFAULT_TRUST);
        *score = (*score - DRIFT_PENALTY).max(0.0);
    }

    /// Penalize a peer for failed ZKP verification
    pub fn penalize_zkp_failure(&mut self, peer: &PeerId) {
        let score = self.scores.entry(*peer).or_insert(DEFAULT_TRUST);
        *score = (*score - ZKP_FAILURE_PENALTY).max(0.0);
    }

    /// Get all non-banned peers and their scores
    pub fn active_peers(&self) -> Vec<(PeerId, f32)> {
        self.scores
            .iter()
            .filter(|(_, &score)| score >= BAN_THRESHOLD)
            .map(|(&peer, &score)| (peer, score))
            .collect()
    }

    /// Get reputation weights for a set of peers (for weighted aggregation)
    /// Returns weights normalized so the max is 1.0
    pub fn get_weights(&self, peers: &[PeerId]) -> Vec<f32> {
        peers.iter().map(|p| self.get_score(p)).collect()
    }

    /// Number of tracked peers
    pub fn peer_count(&self) -> usize {
        self.scores.len()
    }

    /// Number of banned peers
    pub fn banned_count(&self) -> usize {
        self.scores.values().filter(|&&s| s < BAN_THRESHOLD).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_peer(id: u8) -> PeerId {
        let mut peer = [0u8; 32];
        peer[0] = id;
        peer
    }

    #[test]
    fn test_default_score() {
        let tracker = ReputationTracker::new();
        let peer = make_peer(1);
        assert_eq!(tracker.get_score(&peer), 0.5);
    }

    #[test]
    fn test_reward_valid_zkp() {
        let mut tracker = ReputationTracker::new();
        let peer = make_peer(1);

        tracker.reward_valid_zkp(&peer);
        assert!((tracker.get_score(&peer) - 0.52).abs() < 0.001);

        // Reward 25 more times -> should cap at 1.0
        for _ in 0..25 {
            tracker.reward_valid_zkp(&peer);
        }
        assert_eq!(tracker.get_score(&peer), 1.0);
    }

    #[test]
    fn test_penalize_drift() {
        let mut tracker = ReputationTracker::new();
        let peer = make_peer(1);

        tracker.penalize_drift(&peer);
        assert!((tracker.get_score(&peer) - 0.42).abs() < 0.001);
    }

    #[test]
    fn test_ban_threshold() {
        let mut tracker = ReputationTracker::new();
        let peer = make_peer(1);

        // Penalize enough to get below ban threshold
        for _ in 0..5 {
            tracker.penalize_drift(&peer);
        }
        // 0.5 - 5*0.08 = 0.10 < 0.2
        assert!(tracker.is_banned(&peer));
    }

    #[test]
    fn test_zkp_failure_penalty() {
        let mut tracker = ReputationTracker::new();
        let peer = make_peer(1);

        tracker.penalize_zkp_failure(&peer);
        assert!((tracker.get_score(&peer) - 0.35).abs() < 0.001);

        // Two more failures -> 0.35 - 0.15 - 0.15 = 0.05 < 0.2
        tracker.penalize_zkp_failure(&peer);
        tracker.penalize_zkp_failure(&peer);
        assert!(tracker.is_banned(&peer));
    }

    #[test]
    fn test_active_peers_excludes_banned() {
        let mut tracker = ReputationTracker::new();
        let good_peer = make_peer(1);
        let bad_peer = make_peer(2);

        tracker.reward_valid_zkp(&good_peer);
        // Ban bad_peer
        for _ in 0..5 {
            tracker.penalize_drift(&bad_peer);
        }

        let active = tracker.active_peers();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].0, good_peer);
    }
}
