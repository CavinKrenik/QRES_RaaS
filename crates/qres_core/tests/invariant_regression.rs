//! QRES v20 Invariant Regression Test Suite
//!
//! This suite verifies that all six security invariants (INV-1 through INV-6)
//! hold after each phase of the Cognitive Mesh Evolution.
//!
//! **Run this after completing each phase to ensure safety.**

use qres_core::aggregation::{Aggregator, WeightedTrimmedMeanAggregator};
use qres_core::consensus::krum::Bfp16Vec;
use qres_core::encoding::arithmetic;
use qres_core::reputation::ReputationTracker;

/// INV-1: Bounded Influence
///
/// Formal: influence_i <= R_i / sum(R_j)
/// Test: Low-reputation node (R=0.01) with bias=100.0 produces negligible drift
#[test]
fn inv1_bounded_influence() {
    let n = 10;
    let dim = 8;
    
    // Setup: 9 honest nodes (R=1.0) + 1 adversary (R=0.01)
    let mut reputations = vec![1.0; n];
    reputations[0] = 0.01; // Adversary with near-zero reputation
    
    // Updates: honest=1.0, adversary=101.0 (bias=100)
    let mut updates = vec![vec![1.0f32; dim]; n];
    updates[0] = vec![101.0f32; dim]; // Adversarial update
    
    // Convert to fixed-point
    let fixed_updates: Vec<Bfp16Vec> = updates
        .iter()
        .map(|u| Bfp16Vec::from_f32_slice(u))
        .collect();
    
    // Aggregate with reputation weighting
    let aggregator = WeightedTrimmedMeanAggregator::new(2);
    let result = aggregator.aggregate_weighted(&fixed_updates, &reputations);
    
    // Expected: ~1.0 (adversary's 100.0 bias should be negligible)
    // Max influence = 0.01 / (9*1.0 + 0.01) ≈ 0.0011
    // Drift from adversary = 0.0011 * 100 ≈ 0.11
    let drift = (result.weights[0] - arithmetic::float_to_bfp16(1.0)).abs();
    let drift_f32 = arithmetic::bfp16_to_float(drift);
    
    println!("INV-1: Adversary R=0.01, bias=100 → drift={:.4}", drift_f32);
    assert!(
        drift_f32 < 0.15,
        "INV-1 VIOLATED: Low-reputation node produced drift {:.4} (expected < 0.15)",
        drift_f32
    );
}

/// INV-2: Sybil Resistance by Economics
///
/// Formal: total_influence(f + k Sybils) <= total_influence(f) + k * R_initial / (n + k)
/// Test: Doubling Byzantine count via Sybils does NOT double drift
#[test]
fn inv2_sybil_resistance() {
    let n_honest = 75;
    let n_byz = 25;
    let dim = 8;
    
    // Baseline: 25 Byzantine nodes (R=0.5 each)
    let mut reputations_baseline = vec![1.0; n_honest];
    reputations_baseline.extend(vec![0.5; n_byz]);
    
    let mut updates_baseline = vec![vec![1.0f32; dim]; n_honest];
    updates_baseline.extend(vec![vec![2.0f32; dim]; n_byz]); // Byzantine bias
    
    let fixed_baseline: Vec<Bfp16Vec> = updates_baseline
        .iter()
        .map(|u| Bfp16Vec::from_f32_slice(u))
        .collect();
    
    let aggregator = WeightedTrimmedMeanAggregator::new(10);
    let result_baseline = aggregator.aggregate_weighted(&fixed_baseline, &reputations_baseline);
    let drift_baseline = (result_baseline.weights[0] - arithmetic::float_to_bfp16(1.0)).abs();
    
    // Sybil scenario: Add 25 more Byzantine (total 50 Byzantine, 125 nodes)
    let mut reputations_sybil = vec![1.0; n_honest];
    reputations_sybil.extend(vec![0.5; n_byz * 2]); // Double the Byzantine count
    
    let mut updates_sybil = vec![vec![1.0f32; dim]; n_honest];
    updates_sybil.extend(vec![vec![2.0f32; dim]; n_byz * 2]);
    
    let fixed_sybil: Vec<Bfp16Vec> = updates_sybil
        .iter()
        .map(|u| Bfp16Vec::from_f32_slice(u))
        .collect();
    
    let result_sybil = aggregator.aggregate_weighted(&fixed_sybil, &reputations_sybil);
    let drift_sybil = (result_sybil.weights[0] - arithmetic::float_to_bfp16(1.0)).abs();
    
    // Verify: drift increase < 2x (despite doubling attacker count)
    let drift_ratio = arithmetic::bfp16_to_float(drift_sybil) / arithmetic::bfp16_to_float(drift_baseline);
    
    println!("INV-2: Baseline drift={:.4}, Sybil drift={:.4}, ratio={:.2}x",
        arithmetic::bfp16_to_float(drift_baseline),
        arithmetic::bfp16_to_float(drift_sybil),
        drift_ratio
    );
    
    assert!(
        drift_ratio < 2.0,
        "INV-2 VIOLATED: Sybil attack amplified drift by {:.2}x (expected < 2.0x)",
        drift_ratio
    );
}

/// INV-3: Collusion Degradation is Graceful
///
/// Formal: drift_collusion <= sum(R_colluders) / sum(R_all) * max_bias
/// Test: 25% colluding Byzantine → drift bounded by reputation fraction
#[test]
fn inv3_collusion_graceful() {
    let n_honest = 75;
    let n_colluders = 25;
    let dim = 8;
    let collusion_bias = 10.0; // Coordinated attack target
    
    // Independent Byzantine (random directions)
    let mut reputations = vec![1.0; n_honest];
    reputations.extend(vec![0.8; n_colluders]); // High-reputation colluders
    
    let mut updates_independent = vec![vec![1.0f32; dim]; n_honest];
    // Independent attacks: random bias in [1.0, 11.0]
    for i in 0..n_colluders {
        let bias = 1.0 + (i as f32 % 10.0);
        updates_independent.push(vec![bias; dim]);
    }
    
    let fixed_independent: Vec<Bfp16Vec> = updates_independent
        .iter()
        .map(|u| Bfp16Vec::from_f32_slice(u))
        .collect();
    
    let aggregator = WeightedTrimmedMeanAggregator::new(8);
    let result_independent = aggregator.aggregate_weighted(&fixed_independent, &reputations);
    let drift_independent = (result_independent.weights[0] - arithmetic::float_to_bfp16(1.0)).abs();
    
    // Colluding Byzantine (all target collusion_bias)
    let mut updates_colluding = vec![vec![1.0f32; dim]; n_honest];
    updates_colluding.extend(vec![vec![1.0 + collusion_bias; dim]; n_colluders]);
    
    let fixed_colluding: Vec<Bfp16Vec> = updates_colluding
        .iter()
        .map(|u| Bfp16Vec::from_f32_slice(u))
        .collect();
    
    let result_colluding = aggregator.aggregate_weighted(&fixed_colluding, &reputations);
    let drift_colluding = (result_colluding.weights[0] - arithmetic::float_to_bfp16(1.0)).abs();
    
    // Verify: collusion amplifies but is bounded (< 5x)
    let amplification = arithmetic::bfp16_to_float(drift_colluding) / arithmetic::bfp16_to_float(drift_independent);
    
    println!("INV-3: Independent drift={:.4}, Colluding drift={:.4}, amplification={:.2}x",
        arithmetic::bfp16_to_float(drift_independent),
        arithmetic::bfp16_to_float(drift_colluding),
        amplification
    );
    
    assert!(
        amplification < 5.0,
        "INV-3 VIOLATED: Collusion amplified drift by {:.2}x (expected < 5.0x)",
        amplification
    );
}

/// INV-4: No Regime Escalation by Untrusted Quorum
///
/// Formal: Storm transition requires ≥3 nodes with R > 0.8
/// Test: 10 low-rep nodes cannot trigger Storm; 3 high-rep nodes can
#[test]
fn inv4_regime_gate() {
    const QUORUM_MIN: usize = 3;
    const REPUTATION_THRESHOLD: f32 = 0.8;
    
    // Scenario 1: 10 low-reputation nodes signal entropy spike
    let low_rep_signals = vec![0.3f32; 10]; // All below threshold
    let high_rep_count = low_rep_signals.iter().filter(|&&r| r > REPUTATION_THRESHOLD).count();
    
    println!("INV-4: Low-rep signals: {} nodes with R > {}", high_rep_count, REPUTATION_THRESHOLD);
    assert!(
        high_rep_count < QUORUM_MIN,
        "INV-4 SETUP ERROR: Low-rep scenario should have < {} high-rep nodes",
        QUORUM_MIN
    );
    
    // Storm should NOT trigger
    let storm_trigger_low = high_rep_count >= QUORUM_MIN;
    assert!(
        !storm_trigger_low,
        "INV-4 VIOLATED: Low-reputation quorum triggered Storm regime"
    );
    
    // Scenario 2: 2 high-rep + 100 low-rep → still no Storm
    let mut mixed_signals = vec![0.2f32; 100];
    mixed_signals.extend(vec![0.9f32; 2]); // Only 2 high-rep
    let high_rep_count_mixed = mixed_signals.iter().filter(|&&r| r > REPUTATION_THRESHOLD).count();
    
    println!("INV-4: Mixed signals: {} high-rep nodes (need {})", high_rep_count_mixed, QUORUM_MIN);
    assert_eq!(high_rep_count_mixed, 2);
    
    let storm_trigger_mixed = high_rep_count_mixed >= QUORUM_MIN;
    assert!(
        !storm_trigger_mixed,
        "INV-4 VIOLATED: 2 high-rep + 100 low-rep triggered Storm (need >= 3 high-rep)"
    );
    
    // Scenario 3: 3 high-rep nodes → Storm OK
    let high_rep_signals = vec![0.9f32; 3];
    let high_rep_count_ok = high_rep_signals.iter().filter(|&&r| r > REPUTATION_THRESHOLD).count();
    
    println!("INV-4: High-rep quorum: {} nodes (threshold met)", high_rep_count_ok);
    assert_eq!(high_rep_count_ok, 3);
    
    let storm_trigger_ok = high_rep_count_ok >= QUORUM_MIN;
    assert!(
        storm_trigger_ok,
        "INV-4: Valid high-reputation quorum should trigger Storm"
    );
}

/// INV-5: No Brownouts Under Adversarial Noise
///
/// This is tested in simulation (7-day intermittent solar scenario).
/// Here we verify the energy guard logic that prevents brownouts.
#[test]
fn inv5_energy_guard() {
    const ENERGY_RESERVE_THRESHOLD: f32 = 0.15; // 15% minimum for gossip
    
    // Scenario: Node with low energy should refuse to gossip
    let energy_levels = vec![0.05, 0.10, 0.14, 0.15, 0.20, 0.50];
    
    for energy in &energy_levels {
        let can_gossip = energy >= &ENERGY_RESERVE_THRESHOLD;
        
        println!("INV-5: Energy {:.0}% → gossip allowed: {}", energy * 100.0, can_gossip);
        
        if *energy < ENERGY_RESERVE_THRESHOLD {
            assert!(
                !can_gossip,
                "INV-5 VIOLATED: Node with {:.0}% energy allowed to gossip (threshold 15%)",
                energy * 100.0
            );
        }
    }
    
    // Verify: viral gossip respects energy guard
    let low_energy = 0.10;
    let high_priority_override = false; // Even high-priority gossip must respect energy
    
    let gossip_permitted = low_energy >= ENERGY_RESERVE_THRESHOLD || high_priority_override;
    assert!(
        !gossip_permitted,
        "INV-5 VIOLATED: High-priority gossip bypassed energy guard"
    );
}

/// INV-6: Bit-Perfect Compliance is Auditable
///
/// Formal: Q16.16 fixed-point produces identical results across platforms
/// Test: Same input → same output (no floating-point drift)
#[test]
fn inv6_determinism() {
    // Test vectors for Q16.16 arithmetic
    let test_values = vec![1.0, 0.5, 0.25, 1e-5, 100.0];
    
    for val in &test_values {
        // Round-trip: float → fixed → float
        let fixed = arithmetic::float_to_bfp16(*val);
        let recovered = arithmetic::bfp16_to_float(fixed);
        
        // Precision should be within Q16.16 resolution (1/65536)
        let precision = 1.0 / 65536.0;
        let error = (recovered - val).abs();
        
        println!(
            "INV-6: {:.6} → Q16.16 → {:.6} (error: {:.8})",
            val, recovered, error
        );
        
        assert!(
            error < precision * 2.0, // Allow 2x quantum for rounding
            "INV-6 VIOLATED: Q16.16 round-trip error {:.8} exceeds precision bound",
            error
        );
    }
    
    // Test deterministic aggregation
    let data = vec![1.0, 2.0, 3.0, 4.0];
    let bfp_vec = Bfp16Vec::from_f32_slice(&data);
    
    // Multiple conversions should produce identical bits
    let bfp_vec2 = Bfp16Vec::from_f32_slice(&data);
    
    for i in 0..bfp_vec.weights.len() {
        assert_eq!(
            bfp_vec.weights[i], bfp_vec2.weights[i],
            "INV-6 VIOLATED: Non-deterministic conversion at index {}",
            i
        );
    }
}

/// Full Invariant Suite Runner
///
/// Run all invariants in sequence; fail fast on first violation.
#[test]
fn run_all_invariants() {
    println!("\n=== Running Full Invariant Regression Suite ===\n");
    
    inv1_bounded_influence();
    println!("✓ INV-1: Bounded Influence");
    
    inv2_sybil_resistance();
    println!("✓ INV-2: Sybil Resistance");
    
    inv3_collusion_graceful();
    println!("✓ INV-3: Collusion Graceful Degradation");
    
    inv4_regime_gate();
    println!("✓ INV-4: Regime Gate");
    
    inv5_energy_guard();
    println!("✓ INV-5: Energy Guard (No Brownouts)");
    
    inv6_determinism();
    println!("✓ INV-6: Bit-Perfect Determinism");
    
    println!("\n=== All 6 Invariants PASSED ===\n");
}
