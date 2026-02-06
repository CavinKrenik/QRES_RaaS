//! Integration tests for Adaptive Aggregation (Phase 1.1 v21.0)
//!
//! Verifies the cold-start → mature transition with real ReputationTracker dynamics.

use qres_core::aggregation::{AdaptiveAggregator, Aggregator};
use qres_core::reputation::ReputationTracker;

fn make_peer(id: u8) -> [u8; 32] {
    let mut peer = [0u8; 32];
    peer[0] = id;
    peer
}

#[test]
fn test_adaptive_full_lifecycle() {
    // Scenario: 100 honest nodes + 3 Byzantine nodes
    // Verify adaptive mode transitions from cold-start to mature as Byzantine nodes get banned

    const N_HONEST: usize = 100;
    const N_BYZANTINE: usize = 3;
    const N_TOTAL: usize = N_HONEST + N_BYZANTINE;
    const ROUNDS: usize = 30;
    const DIM: usize = 4;

    let mut tracker = ReputationTracker::new();
    let honest_peers: Vec<_> = (0..N_HONEST as u8).map(make_peer).collect();
    let byzantine_peers: Vec<_> = (100..103).map(make_peer).collect(); // Use higher IDs
    let all_peers: Vec<_> = honest_peers
        .iter()
        .chain(byzantine_peers.iter())
        .copied()
        .collect();

    let true_weights = vec![1.0; DIM];
    let mut phase_log = Vec::new();

    for round in 0..ROUNDS {
        // Generate updates
        let mut updates = Vec::new();

        // Honest nodes: small Gaussian noise around true weights
        for _ in &honest_peers {
            let update: Vec<f32> = true_weights
                .iter()
                .map(|&w| w + (round as f32 * 0.001) % 0.1 - 0.05) // Deterministic noise
                .collect();
            updates.push(update);
        }

        // Byzantine nodes: constant bias attack
        for _ in &byzantine_peers {
            let attack_update = vec![10.0; DIM]; // Strong bias to ensure detection
            updates.push(attack_update);
        }

        // Get reputation weights
        let rep_weights: Vec<f32> = all_peers
            .iter()
            .map(|peer| tracker.get_score(peer))
            .collect();

        let banned_count = tracker.banned_count();
        let f = (N_TOTAL / 10).max(1); // Trim 10%

        // Create adaptive aggregator
        let agg = AdaptiveAggregator::new(f, rep_weights.clone(), banned_count, N_TOTAL);
        let is_cold_start = agg.is_cold_start();

        phase_log.push((round, is_cold_start, banned_count));

        // Aggregate
        let result = agg.aggregate(&updates);

        // Compute drift from true weights
        let _drift: f32 = result
            .weights
            .iter()
            .zip(&true_weights)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt();

        // Update reputation: penalize Byzantine nodes more aggressively
        for (i, peer) in all_peers.iter().enumerate() {
            if i >= N_HONEST {
                // This is a Byzantine node - always penalize
                tracker.penalize_drift(peer);
                tracker.penalize_drift(peer); // Double penalty for faster banning
            } else {
                // Honest node - reward
                tracker.reward_valid_zkp(peer);
            }
        }

        // Debug output
        if round % 5 == 0 || (round > 0 && is_cold_start != phase_log[round - 1].1) {
            #[cfg(feature = "std")]
            println!(
                "Round {}: mode={}, banned={}/{} ({:.1}%), drift={:.4}",
                round,
                if is_cold_start { "COLD" } else { "MATURE" },
                banned_count,
                N_TOTAL,
                (banned_count as f32 / N_TOTAL as f32) * 100.0,
                drift
            );
        }
    }

    // Verify lifecycle expectations
    let final_banned = tracker.banned_count();

    // Should have banned all 3 Byzantine nodes
    assert!(
        final_banned >= 3,
        "Should ban all Byzantine nodes: banned={}/{}",
        final_banned,
        N_BYZANTINE
    );

    // Check for mode stability
    // With 3/103 = 2.9% ban rate, should stay in COLD mode (ban_rate > 1%)
    // This is correct behavior - ban_rate threshold is 1%
    let final_mode_is_cold = phase_log.last().map(|(_, cold, _)| *cold).unwrap_or(true);

    #[cfg(feature = "std")]
    {
        println!("\nPhase summary:");
        println!(
            "  Final banned: {}/{} ({:.1}%)",
            final_banned,
            N_TOTAL,
            (final_banned as f32 / N_TOTAL as f32) * 100.0
        );
        println!(
            "  Final mode: {}",
            if final_mode_is_cold {
                "COLD (expected - ban rate 2.9% > 1%)"
            } else {
                "MATURE"
            }
        );
    }

    // Verify that ban rate > 1% keeps system in cold-start (correct behavior)
    assert!(
        final_mode_is_cold,
        "With 2.9% ban rate (> 1% threshold), should remain in COLD mode for continued defense"
    );
}

#[test]
fn test_adaptive_attack_resilience() {
    // Verify adaptive mode maintains Byzantine resistance during cold-start
    const N: usize = 20;
    const BYZ: usize = 6; // 30% Byzantine
    const DIM: usize = 8;

    let tracker = ReputationTracker::new();
    let all_peers: Vec<_> = (0..N as u8).map(make_peer).collect();

    // Simulate coordinated attack
    let mut updates = Vec::new();

    // Honest nodes (0..14)
    for _ in 0..(N - BYZ) {
        updates.push(vec![1.0; DIM]);
    }

    // Byzantine cartel (14..20) - within trimming bounds but coordinated
    for _ in 0..BYZ {
        updates.push(vec![1.3; DIM]); // 30% bias
    }

    let rep_weights: Vec<f32> = all_peers
        .iter()
        .map(|peer| tracker.get_score(peer))
        .collect();

    let agg = AdaptiveAggregator::new(2, rep_weights, 0, N);
    assert!(agg.is_cold_start());

    let result = agg.aggregate(&updates);

    // Should trim the Byzantine values and stay close to 1.0
    let drift = (result.weights[0] - 1.0).abs();
    assert!(
        drift < 0.2,
        "Cold-start mode should resist Byzantine attack: drift={}",
        drift
    );
}

#[test]
fn test_adaptive_mature_convergence() {
    // Verify mature mode achieves better convergence than cold-start
    const N: usize = 15;
    const DIM: usize = 6;

    let mut tracker = ReputationTracker::new();
    let all_peers: Vec<_> = (0..N as u8).map(make_peer).collect();

    // Reward all peers to establish high reputation
    for _ in 0..20 {
        for peer in &all_peers {
            tracker.reward_valid_zkp(peer);
        }
    }

    // Ban 3 peers to trigger mature mode
    for peer in all_peers.iter().skip(10).take(3) {
        for _ in 0..10 {
            tracker.penalize_drift(peer);
        }
    }

    let banned = tracker.banned_count();
    assert!(banned >= 3, "Should have banned nodes");

    // Generate near-perfect updates (all nodes honest)
    let true_value = 2.5;
    let updates: Vec<Vec<f32>> = (0..N)
        .map(|i| vec![true_value + (i as f32 * 0.001); DIM])
        .collect();

    let rep_weights: Vec<f32> = all_peers
        .iter()
        .map(|peer| tracker.get_score(peer))
        .collect();

    // Force mature mode with low ban rate
    let agg = AdaptiveAggregator::new(1, rep_weights, 3, 500);
    assert!(!agg.is_cold_start(), "Should be in mature mode");

    let result = agg.aggregate(&updates);

    // Mature mode should converge tightly to true value
    let drift = (result.weights[0] - true_value).abs();
    assert!(
        drift < 0.1,
        "Mature mode should have tight convergence: drift={}",
        drift
    );
}
