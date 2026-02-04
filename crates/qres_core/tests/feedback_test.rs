use qres_core::adaptive::feedback_loop::FeedbackLoop;
use qres_core::adaptive::regime_detector::{Regime, RegimeChange, RegimeDetector};

#[test]
fn test_regime_change_detection() {
    // 1. Setup
    let window_size = 32;
    let mut detector = RegimeDetector::new(window_size, 0.8, 1000.0);

    println!(">> Phase 1: Training on Stable Signal (Sine Wave)");
    // Feed 100 samples of a clean sine wave
    for i in 0..100 {
        let actual = (i as f32 * 0.1).sin();
        let prediction = actual + 0.01; // Small constant error
        let residual = prediction - actual;

        let change = detector.observe(residual);
        assert_eq!(
            change,
            RegimeChange::None,
            "Should not detect drift in stable phase"
        );
    }

    println!(">> Phase 2: Injecting drift (Sudden Spike/Noise)");
    // Suddenly add massive error
    let mut detected = false;
    for _ in 0..10 {
        // Massive error: 1.0 (compared to 0.01)
        let change = detector.observe(1.0);
        if let RegimeChange::Drift {
            current_error,
            threshold,
        } = change
        {
            println!(
                "Drift Detected! Error: {:.4}, Threshold: {:.4}",
                current_error, threshold
            );
            detected = true;
            break;
        }
    }

    assert!(detected, "Failed to detect regime change!");
}

#[test]
fn test_feedback_loop_integration() {
    let mut feedback = FeedbackLoop::new(32);

    // Stable
    for _ in 0..50 {
        feedback.observe(1.0, 1.01);
    }

    // Broken
    // Taking manual observation to verify internal state isn't easily done without
    // internal access or return values, but this ensures it runs without panic.
    // In a real system, we'd check logs or events.
    feedback.observe(1.0, 5.0);
}

/// Tests the Pre-Storm detection: the 3-point moving average entropy derivative
/// should trigger PreStorm before full Storm is reached.
#[test]
fn test_prestorm_detection() {
    let mut detector = RegimeDetector::new(32, 0.8, 1000000.0);
    detector.set_entropy_derivative_threshold(0.15);

    println!(">> Phase 1: Calm regime with low entropy");
    // Feed low entropy values -> should stay Calm
    for i in 0..10 {
        detector.update(0.1, 100, i * 100);
        assert_eq!(
            detector.current_regime(),
            Regime::Calm,
            "Should be Calm at low entropy"
        );
    }

    println!(">> Phase 2: Rapidly increasing entropy -> should trigger Pre-Storm");
    // Now rapidly increase entropy: 0.2 -> 0.4 -> 0.6
    // This creates a large derivative in the 3-point MA
    let mut prestorm_detected = false;
    let entropies = [0.2, 0.4, 0.65, 0.7, 0.75];
    for (i, &e) in entropies.iter().enumerate() {
        detector.update(e, 100, 1000 + i as u64 * 100);
        let regime = detector.current_regime();
        println!(
            "  Entropy={:.2}, Smoothed={:.4}, Derivative={:.4}, Regime={:?}",
            e,
            detector.smoothed_entropy(),
            detector.entropy_derivative(),
            regime
        );
        if regime == Regime::PreStorm {
            prestorm_detected = true;
            println!("  >> Pre-Storm DETECTED at entropy={:.2}", e);
        }
    }

    assert!(
        prestorm_detected,
        "Pre-Storm should be detected during rapid entropy increase"
    );

    println!(">> Phase 3: Entropy exceeds threshold -> Storm");
    // With hysteresis, need 5 consecutive confirmations for PreStorm->Storm
    for i in 0..6 {
        detector.update(0.9, 100, 2000 + i * 100);
    }
    assert_eq!(
        detector.current_regime(),
        Regime::Storm,
        "Should be Storm after 5+ confirmations at high entropy"
    );

    println!(">> Phase 4: Entropy drops -> back to Calm");
    // With hysteresis, need 5 consecutive confirmations for Storm->Calm
    for i in 0..6 {
        detector.update(0.1, 100, 3000 + i * 100);
    }
    assert_eq!(
        detector.current_regime(),
        Regime::Calm,
        "Should return to Calm after 5+ confirmations at low entropy"
    );

    println!("PRE-STORM DETECTION: VERIFIED");
    println!("  Lag-to-Adaptation: nodes enter Storm mode BEFORE critical failure");
}

/// Measures Lag-to-Adaptation time: how many updates before regime transition.
#[test]
fn test_lag_to_adaptation_measurement() {
    let mut detector = RegimeDetector::new(32, 0.8, 1000000.0);
    detector.set_entropy_derivative_threshold(0.1);

    // Establish baseline
    for i in 0..20 {
        detector.update(0.1, 100, i * 100);
    }

    // Begin entropy ramp - steeper to trigger derivative threshold
    let mut ticks_to_prestorm = 0u32;
    let mut ticks_to_storm = 0u32;
    let ramp_start = 0.2;
    let ramp_step = 0.15; // Steeper ramp to trigger derivative threshold

    // Extended ramp to account for hysteresis requirements (5 confirmations)
    for tick in 0..30 {
        let entropy = ramp_start + tick as f32 * ramp_step;
        detector.update(entropy, 100, 2000 + tick as u64 * 100);

        if ticks_to_prestorm == 0 && detector.current_regime() == Regime::PreStorm {
            ticks_to_prestorm = tick + 1;
            println!("  PreStorm detected at tick {} with entropy {:.2}", tick + 1, entropy);
        }
        if ticks_to_storm == 0 && detector.current_regime() == Regime::Storm {
            ticks_to_storm = tick + 1;
            println!("  Storm detected at tick {} with entropy {:.2}", tick + 1, entropy);
        }
    }

    println!("--- Lag-to-Adaptation Measurement ---");
    println!("Ticks to Pre-Storm: {}", ticks_to_prestorm);
    println!("Ticks to Storm: {}", ticks_to_storm);

    assert!(
        ticks_to_prestorm > 0,
        "Pre-Storm should trigger during entropy ramp"
    );
    assert!(
        ticks_to_storm > 0,
        "Storm should trigger during entropy ramp"
    );
    assert!(
        ticks_to_prestorm < ticks_to_storm,
        "Pre-Storm ({}) should trigger BEFORE Storm ({})",
        ticks_to_prestorm,
        ticks_to_storm
    );

    println!(
        "RESULT: Pre-Storm at tick {}, Storm at tick {} -> {} ticks early warning",
        ticks_to_prestorm,
        ticks_to_storm,
        ticks_to_storm - ticks_to_prestorm
    );
}
