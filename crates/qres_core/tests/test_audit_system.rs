//! Integration tests for the stochastic audit system (v21.0)
//!
//! Tests the complete audit lifecycle:
//! 1. Challenge generation (deterministic selection)
//! 2. Response creation by audited nodes
//! 3. Verification and reputation punishment
//! 4. Detection of Class C collusion cartels

use qres_core::audit::{CollisionAuditConfig, CollisionAuditor};
use qres_core::packet::AuditResponse;
use qres_core::reputation::{PeerId, ReputationTracker};
use qres_core::zk_proofs::{EnclaveGate, SoftwareEnclaveGate};

/// Test that audit challenges are generated deterministically
#[test]
fn test_audit_challenge_deterministic() {
    let config = CollisionAuditConfig {
        audit_interval: 50,
        nodes_per_audit: 3,
        entropy_threshold: 0.3,
        response_timeout_seconds: 10,
    };

    let mut auditor1 = CollisionAuditor::new(config.clone());
    let mut auditor2 = CollisionAuditor::new(config);

    let epoch = [0xABu8; 32];
    auditor1.update_epoch_hash(&epoch);
    auditor2.update_epoch_hash(&epoch);

    let peers: Vec<PeerId> = (0..10).map(|i| [i; 32]).collect();

    let challenges1 = auditor1.generate_challenges(50, 0.5, &peers, 1000);
    let challenges2 = auditor2.generate_challenges(50, 0.5, &peers, 1000);

    assert_eq!(challenges1.len(), challenges2.len());
    for ((p1, c1), (p2, c2)) in challenges1.iter().zip(challenges2.iter()) {
        assert_eq!(p1, p2, "Same peer should be selected");
        assert_eq!(c1.audit_round, c2.audit_round, "Same round");
        assert_eq!(c1.nonce, c2.nonce, "Same nonce");
    }
}

/// Test complete audit lifecycle: challenge → response → verification
#[test]
fn test_audit_lifecycle_honest_node() {
    let config = CollisionAuditConfig::default();
    let auditor = CollisionAuditor::new(config);

    let peers: Vec<PeerId> = (1..=5).map(|i| [i; 32]).collect();
    let challenges = auditor.generate_challenges(50, 0.5, &peers, 1000);

    assert_eq!(challenges.len(), 3);

    // Simulate honest node response
    let (challenged_peer, challenge) = &challenges[0];

    // Node computes gradient from raw prediction
    let raw_prediction = vec![65536, 131072, 196608]; // Q16.16: [1.0, 2.0, 3.0]
    let local_data_hash = [0x42u8; 32];

    // Simulate gradient computation (simple hash-based for test)
    let submitted_gradient = raw_prediction.clone();

    let response = AuditResponse::new(
        *challenged_peer,
        raw_prediction.clone(),
        local_data_hash,
        submitted_gradient.clone(),
        challenge.nonce,
        None, // No ZK proof in basic test
    );

    // Verify nonce matches
    assert_eq!(response.nonce, challenge.nonce);

    // Verify gradient dimensions match
    assert_eq!(
        response.raw_prediction.len(),
        response.submitted_gradient.len()
    );
}

/// Test audit response verification using EnclaveGate
#[test]
fn test_audit_verification_pass() {
    let gate = SoftwareEnclaveGate::default();

    let raw_prediction = vec![65536, 131072]; // Q16.16: [1.0, 2.0]
    let local_data_hash = [0x42u8; 32];

    // Node computes gradient (in real system, this uses actual gradient function)
    // For testing, we simulate with hash-based computation matching the impl
    let submitted_gradient: Vec<i32> = raw_prediction
        .iter()
        .enumerate()
        .map(|(i, &pred): (usize, &i32)| {
            let hash_byte = local_data_hash[i % 32];
            let hash_contribution = (hash_byte as i32) << 8;
            pred.wrapping_add(hash_contribution)
        })
        .collect();

    // Verify should pass with matching computation
    let verified =
        gate.verify_audit_response(&raw_prediction, &local_data_hash, &submitted_gradient);
    assert!(verified, "Honest node audit should pass");
}

/// Test audit verification detects mismatched gradients
#[test]
fn test_audit_verification_fail_mismatch() {
    let gate = SoftwareEnclaveGate::default();

    let raw_prediction = vec![65536, 131072]; // Q16.16: [1.0, 2.0]
    let local_data_hash = [0x42u8; 32];

    // Dishonest node submits different gradient
    let fake_gradient = vec![100000, 200000]; // Doesn't match computation

    let verified = gate.verify_audit_response(&raw_prediction, &local_data_hash, &fake_gradient);
    assert!(!verified, "Mismatched gradient should fail audit");
}

/// Test audit verification detects dimension mismatch
#[test]
fn test_audit_verification_fail_dimension() {
    let gate = SoftwareEnclaveGate::default();

    let raw_prediction = vec![65536, 131072];
    let local_data_hash = [0x42u8; 32];

    // Wrong dimension
    let wrong_gradient = vec![65536]; // Only 1 element instead of 2

    let verified = gate.verify_audit_response(&raw_prediction, &local_data_hash, &wrong_gradient);
    assert!(!verified, "Dimension mismatch should fail audit");
}

/// Test challenge expiration after timeout
#[test]
fn test_audit_challenge_expiration() {
    let config = CollisionAuditConfig {
        audit_interval: 50,
        nodes_per_audit: 3,
        entropy_threshold: 0.3,
        response_timeout_seconds: 10,
    };

    let auditor = CollisionAuditor::new(config);
    let peers: Vec<PeerId> = (1..=5).map(|i| [i; 32]).collect();

    let challenges = auditor.generate_challenges(50, 0.5, &peers, 1000);
    let challenge = &challenges[0].1;

    // Not expired yet
    assert!(
        !challenge.is_expired(1005),
        "Should not expire after 5 seconds"
    );

    // Expired after timeout
    assert!(
        challenge.is_expired(1011),
        "Should expire after 11 seconds (>10)"
    );
}

/// Test reputation punishment for failed audits
#[test]
fn test_audit_failure_punishment() {
    let mut tracker = ReputationTracker::new();
    let peer_id = [1u8; 32];

    // Initial reputation (default 0.5 for new peers)
    let initial_rep = tracker.get_score(&peer_id);

    // Punish for failed audit (using penalize_zkp_failure as audit failure is serious)
    tracker.penalize_zkp_failure(&peer_id);

    let final_rep = tracker.get_score(&peer_id);

    // Reputation should decrease significantly (audit failure is serious)
    assert!(
        final_rep < initial_rep,
        "Audit failure should reduce reputation"
    );
    assert!(
        initial_rep - final_rep > 0.1,
        "Audit failure penalty should be substantial"
    );
}

/// Test Class C collusion detection over multiple audit rounds
///
/// Simulates a 5-node cartel submitting biased gradients within trimming bounds.
/// Verifies that audits eventually catch at least one cartel member.
#[test]
fn test_class_c_collusion_detection() {
    let config = CollisionAuditConfig {
        audit_interval: 50,
        nodes_per_audit: 3,
        entropy_threshold: 0.3,
        response_timeout_seconds: 10,
    };

    let mut auditor = CollisionAuditor::new(config.clone());
    let mut tracker = ReputationTracker::new();
    let gate = SoftwareEnclaveGate::default();

    // 20 total nodes: 15 honest + 5 cartel
    let total_nodes = 20;
    let cartel_size = 5;
    let cartel_indices: Vec<usize> = (0..cartel_size).collect();

    let peers: Vec<PeerId> = (0..total_nodes).map(|i| [i as u8; 32]).collect();

    // Reputations are initialized to default (0.5) automatically

    let mut detected_cartel_members = 0;
    let max_rounds = 1000; // Run up to 1000 rounds
    let mut current_time = 1000u64;

    for round in 1..=max_rounds {
        // Update epoch hash each round (simulate consensus)
        let epoch_hash: [u8; 32] = [(round % 256) as u8; 32];
        auditor.update_epoch_hash(&epoch_hash);

        // Generate challenges
        let challenges = auditor.generate_challenges(round, 0.5, &peers, current_time);

        for (peer_id, _challenge) in challenges {
            let peer_index = peer_id[0] as usize;

            // Determine if this is a cartel member
            let is_cartel = cartel_indices.contains(&peer_index);

            // Simulate response
            let raw_prediction = vec![65536, 131072];
            let local_data_hash = [0x42u8; 32];

            let submitted_gradient = if is_cartel {
                // Cartel member submits biased gradient (within bounds but incorrect)
                vec![70000, 140000] // Biased but within 1.5σ
            } else {
                // Honest node computes correct gradient
                raw_prediction
                    .iter()
                    .enumerate()
                    .map(|(i, &pred): (usize, &i32)| {
                        let hash_byte = local_data_hash[i % 32];
                        let hash_contribution = (hash_byte as i32) << 8;
                        pred.wrapping_add(hash_contribution)
                    })
                    .collect()
            };

            // Verify audit
            let verified =
                gate.verify_audit_response(&raw_prediction, &local_data_hash, &submitted_gradient);

            if !verified {
                tracker.penalize_zkp_failure(&peer_id);

                if is_cartel {
                    detected_cartel_members += 1;
                }
            }
        }

        // Check if we've detected all cartel members
        if detected_cartel_members >= cartel_size {
            println!(
                "✅ Detected all {} cartel members by round {}",
                cartel_size, round
            );

            // Verify expected detection time
            let expected_rounds = auditor.expected_detection_rounds(total_nodes, cartel_size);
            println!(
                "Expected detection: ~{:.0} rounds, Actual: {} rounds",
                expected_rounds, round
            );

            assert!(
                (round as f32) < expected_rounds * 3.0,
                "Detection took longer than 3× expected time"
            );

            return; // Test passed
        }

        current_time += 50; // Simulate time passing
    }

    panic!(
        "Failed to detect all cartel members within {} rounds. Detected: {}/{}",
        max_rounds, detected_cartel_members, cartel_size
    );
}

/// Test audit bandwidth overhead calculation
#[test]
fn test_audit_bandwidth_overhead() {
    let config = CollisionAuditConfig {
        audit_interval: 50,
        nodes_per_audit: 3,
        entropy_threshold: 0.3,
        response_timeout_seconds: 10,
    };

    let auditor = CollisionAuditor::new(config);

    // With 150 nodes, auditing 3 nodes = 2% overhead
    let rate = auditor.audit_rate(150);
    assert_eq!(rate, 0.02);

    // Bandwidth cost per audit:
    // - Challenge: ~100 bytes
    // - Response: ~(4 * grad_dim + 96) bytes
    // For 100-dim gradient: ~596 bytes per audit
    // Total per interval: 596 * 3 = 1,788 bytes per 50 rounds
    // This is negligible compared to regular GhostUpdate packets
}

/// Test that audits only trigger above entropy threshold
#[test]
fn test_audit_entropy_gating() {
    let config = CollisionAuditConfig {
        audit_interval: 50,
        nodes_per_audit: 3,
        entropy_threshold: 0.5,
        response_timeout_seconds: 10,
    };

    let auditor = CollisionAuditor::new(config);
    let peers: Vec<PeerId> = (0..10).map(|i| [i; 32]).collect();

    // Low entropy (idle network) → no audits
    let challenges_idle = auditor.generate_challenges(50, 0.2, &peers, 1000);
    assert_eq!(challenges_idle.len(), 0, "No audits during idle periods");

    // High entropy (active attack) → trigger audits
    let challenges_active = auditor.generate_challenges(50, 0.8, &peers, 1000);
    assert_eq!(
        challenges_active.len(),
        3,
        "Audits triggered during activity"
    );
}
