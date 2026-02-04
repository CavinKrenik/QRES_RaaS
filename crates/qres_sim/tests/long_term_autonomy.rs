use csv::Reader;
use qres_core::{
    adaptive::regime_detector::{Regime as RegimeState, RegimeDetector},
    power::MockRadio,
    reputation::ReputationTracker,
    resource_management::EnergyPool,
};
use std::fs::File;
use std::time::Instant;

#[derive(Debug, Clone)]
struct WeatherSample {
    #[allow(dead_code)]
    timestamp: f64,
    temp: f64,
    pressure: f64,
    wind_speed: f64,
}

#[test]
fn test_6month_autonomy() {
    println!("\n=== 6-Month Long-Term Autonomy Test ===\n");

    // Load dataset
    let dataset_path = "../../evaluation/data/long_term/weather_6month_2016-01-01.csv";
    if !std::path::Path::new(dataset_path).exists() {
        panic!(
            "Dataset not found at {}. Please run fetch_weather_6month.py first.",
            dataset_path
        );
    }

    let samples = load_weather_samples(dataset_path).expect("Failed to load 6-month dataset");

    println!(
        "Loaded {} samples ({:.1} days)",
        samples.len(),
        samples.len() as f64 / 144.0
    );

    // Initialize QRES components
    // Window=10, EntropyThresh=0.2 (Lowered for sensitivity), ThroughputThresh=500.0
    let mut detector = RegimeDetector::new(10, 0.2, 500.0);

    // 23760 J capacity (u32)
    let mut energy_pool = EnergyPool::new(23760);
    let mut radio = MockRadio::new();
    let mut reputation = ReputationTracker::new();

    // Simulation state tracking
    let mut regime_history = Vec::new();
    let mut energy_history = Vec::new();
    let mut sleep_intervals = Vec::new();
    let mut transmission_count = 0u64;

    // Solar recharge: 5940J / day. 144 samples/day => ~41 J/sample
    let solar_recharge_per_sample = 41;

    let start_time = Instant::now();

    // Main simulation loop
    for (idx, sample) in samples.iter().enumerate() {
        let entropy = compute_weather_entropy(sample);

        // Update regime detector (time in ms, 10-min steps)
        let now_ms = (idx as u64) * 600 * 1000;
        detector.update(entropy, 0, now_ms);
        let regime = detector.current_regime();

        // Energy gating: force Calm if critically low
        let effective_regime = if energy_pool.is_critical() {
            RegimeState::Calm
        } else {
            regime
        };

        // Determine wake interval based on regime
        let base_interval = match effective_regime {
            RegimeState::Calm => 14400,   // 4 hours in seconds
            RegimeState::PreStorm => 600, // 10 minutes
            RegimeState::Storm => 30,     // 30 seconds
        };

        // Calculate average reputation
        let mut total_score = 0.0;
        for i in 0..100 {
            let mut peer_id = [0u8; 32];
            peer_id[0] = i as u8;
            total_score += reputation.get_score(&peer_id);
        }
        let avg_reputation = total_score / 100.0;

        let wake_interval = base_interval as f64 * (0.5 + 0.5 * avg_reputation as f64);

        // Transmit logic
        // If wake_interval < 600s, we might transmit multiple times.
        // If wake_interval > 600s, we transmit sparsely.
        let sample_duration = 600.0;
        let intervals_per_sample = sample_duration / wake_interval;

        let transmissions_this_step = if intervals_per_sample >= 1.0 {
            intervals_per_sample as u64
        } else {
            // Probabilistic or stride
            let stride = (1.0 / intervals_per_sample) as usize;
            if stride > 0 && idx % stride == 0 {
                1
            } else {
                0
            }
        };

        if transmissions_this_step > 0 {
            // Cost: ~2J per TX (8KB @ 10kbps, 330mW)
            let tx_cost = 2 * transmissions_this_step as u32;

            if energy_pool.can_afford(tx_cost) {
                energy_pool.spend(tx_cost);
                radio.account_transmission(transmissions_this_step as usize);
                transmission_count += transmissions_this_step;

                // Update reputation
                let mut peer_id = [0u8; 32];
                peer_id[0] = (idx % 100) as u8;
                reputation.reward_valid_zkp(&peer_id);
            }
        }

        // Sleep energy: negligible in J integer math (0.01 J), skipping for EnergyPool but MockRadio tracks it
        radio.sleep(now_ms + (wake_interval as u64 * 1000));

        // Apply solar recharge
        energy_pool.recharge(solar_recharge_per_sample);

        // Record state every 24 hours (144 samples)
        if idx % 144 == 0 {
            regime_history.push((idx, effective_regime));
            energy_history.push((idx, energy_pool.current() as f64));
            sleep_intervals.push((idx, wake_interval));
        }

        // Progress indicator every 30 days
        if idx % (144 * 30) == 0 && idx > 0 {
            let days = idx / 144;
            let elapsed = start_time.elapsed();
            println!(
                "  Day {}: Regime={:?}, Energy={}J, Transmissions={}, Runtime={:.1}s",
                days,
                effective_regime,
                energy_pool.current(),
                transmission_count,
                elapsed.as_secs_f64()
            );
        }
    }

    let total_time = start_time.elapsed();

    // Final avg rep
    let mut total_score = 0.0;
    for i in 0..100 {
        let mut peer_id = [0u8; 32];
        peer_id[0] = i as u8;
        total_score += reputation.get_score(&peer_id);
    }
    let final_avg_reputation = total_score / 100.0;

    // Final statistics
    println!("\n=== Simulation Complete ===");
    println!("Real-time runtime: {:.2}s", total_time.as_secs_f64());
    println!("Simulated time: {:.1} days", samples.len() as f64 / 144.0);
    println!("Total transmissions: {}", transmission_count);
    println!(
        "Final energy: {}J / 23760J",
        energy_history.last().unwrap().1
    );
    println!("Avg reputation: {:.3}", final_avg_reputation);

    // Regime distribution
    let calm_count = regime_history
        .iter()
        .filter(|(_, r)| matches!(r, RegimeState::Calm))
        .count();
    let prestorm_count = regime_history
        .iter()
        .filter(|(_, r)| matches!(r, RegimeState::PreStorm))
        .count();
    let storm_count = regime_history
        .iter()
        .filter(|(_, r)| matches!(r, RegimeState::Storm))
        .count();

    println!("\nRegime distribution:");
    println!(
        "  Calm: {:.1}%",
        100.0 * calm_count as f64 / regime_history.len() as f64
    );
    println!(
        "  PreStorm: {:.1}%",
        100.0 * prestorm_count as f64 / regime_history.len() as f64
    );
    println!(
        "  Storm: {:.1}%",
        100.0 * storm_count as f64 / regime_history.len() as f64
    );

    // Export results
    std::fs::create_dir_all("evaluation/results").expect("Failed to create results dir");
    export_results(&regime_history, &energy_history, &sleep_intervals);

    // Assertions for CI
    assert!(
        energy_history.last().unwrap().1 > 1000.0,
        "Battery depleted before 6 months"
    );
    assert!(transmission_count > 100, "Too few transmissions");
    assert!(
        transmission_count < samples.len() as u64 * 10,
        "Transmission sanity check"
    );
}

fn load_weather_samples(path: &str) -> Result<Vec<WeatherSample>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut rdr = Reader::from_reader(file);
    let mut samples = Vec::new();
    for (idx, result) in rdr.records().enumerate() {
        let record = result?;
        // Standard Jena: p (mbar) [1], T (degC) [2], wv (m/s) [12]
        let pressure: f64 = record.get(1).and_then(|s| s.parse().ok()).unwrap_or(1000.0);
        let temp: f64 = record.get(2).and_then(|s| s.parse().ok()).unwrap_or(20.0);
        let wind_speed: f64 = record.get(12).and_then(|s| s.parse().ok()).unwrap_or(0.0);

        samples.push(WeatherSample {
            timestamp: idx as f64 * 600.0,
            temp,
            pressure,
            wind_speed,
        });
    }
    Ok(samples)
}

fn compute_weather_entropy(sample: &WeatherSample) -> f32 {
    let temp_norm = (sample.temp + 20.0) / 40.0;
    let pressure_norm = (sample.pressure - 950.0) / 100.0;
    let wind_norm = sample.wind_speed / 30.0;

    let features = [temp_norm, pressure_norm, wind_norm];
    let mean = features.iter().sum::<f64>() / features.len() as f64;
    let variance = features.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / features.len() as f64;

    variance.clamp(0.0, 1.0) as f32
}

fn export_results(
    regime_history: &[(usize, RegimeState)],
    energy_history: &[(usize, f64)],
    sleep_intervals: &[(usize, f64)],
) {
    use std::io::Write;
    let mut f = File::create("evaluation/results/regime_timeline.csv").unwrap();
    writeln!(f, "day,regime").unwrap();
    for (idx, regime) in regime_history {
        let day = *idx as f64 / 144.0;
        let regime_code = match regime {
            RegimeState::Calm => 0,
            RegimeState::PreStorm => 1,
            RegimeState::Storm => 2,
        };
        writeln!(f, "{:.2},{}", day, regime_code).unwrap();
    }

    let mut f = File::create("evaluation/results/energy_timeline.csv").unwrap();
    writeln!(f, "day,energy_joules").unwrap();
    for (idx, energy) in energy_history {
        let day = *idx as f64 / 144.0;
        writeln!(f, "{:.2},{:.1}", day, energy).unwrap();
    }

    let mut f = File::create("evaluation/results/sleep_intervals.csv").unwrap();
    writeln!(f, "day,interval_seconds").unwrap();
    for (idx, interval) in sleep_intervals {
        let day = *idx as f64 / 144.0;
        writeln!(f, "{:.2},{:.1}", day, interval).unwrap();
    }
    println!("\n✓ Results exported to evaluation/results/");
}
