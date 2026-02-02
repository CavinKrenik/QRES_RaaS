use alloc::vec;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Regime {
    Calm,
    /// Pre-Storm: entropy derivative exceeds threshold, preemptively
    /// increasing adaptation rates before critical failure.
    PreStorm,
    Storm,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegimeChange {
    None,
    Drift { current_error: f32, threshold: f32 },
}

pub struct RegimeDetector {
    window_size: usize,
    history: Vec<f32>,
    /// Running sum of values in the window
    sum: f32,
    /// Running sum of squares in the window
    sum_sq: f32,
    /// Current write index in the ring buffer
    idx: usize,
    /// Number of samples observed so far
    count: usize,
    /// Entropy threshold for storm detection
    entropy_threshold: f32,
    /// Throughput threshold (bytes/sec) for storm detection
    throughput_threshold: f32,
    /// Current regime
    current_regime: Regime,

    // --- Throughput Tracking Fields ---
    /// Last update timestamp (ms)
    last_update_ms: u64,
    /// Accumulated bytes since last update
    accumulated_bytes: u64,
    /// Current throughput metric (bytes/sec)
    current_throughput: f32,

    // --- 3-Point Moving Average Entropy Fields ---
    /// 3-point entropy history for moving average
    entropy_ma_buffer: [f32; 3],
    /// Write index for the 3-point buffer
    entropy_ma_idx: usize,
    /// Number of entropy samples observed
    entropy_ma_count: usize,
    /// Previous smoothed entropy (for derivative calculation)
    prev_smoothed_entropy: f32,
    /// Entropy derivative threshold for Pre-Storm trigger
    entropy_derivative_threshold: f32,

    // --- Strategic Silence Fields ---
    /// Consecutive Calm observations (for stability-based silence)
    calm_observation_count: usize,
    /// Variance threshold for "stable enough for silence" determination
    silence_variance_threshold: f32,
}

impl RegimeDetector {
    pub fn new(window_size: usize, entropy_threshold: f32, throughput_threshold: f32) -> Self {
        Self {
            window_size,
            history: vec![0.0; window_size],
            sum: 0.0,
            sum_sq: 0.0,
            idx: 0,
            count: 0,
            entropy_threshold,
            throughput_threshold,
            current_regime: Regime::Calm,
            last_update_ms: 0,
            accumulated_bytes: 0,
            current_throughput: 0.0,
            entropy_ma_buffer: [0.0; 3],
            entropy_ma_idx: 0,
            entropy_ma_count: 0,
            prev_smoothed_entropy: 0.0,
            // Default: if entropy increases by 0.3 per update, trigger Pre-Storm
            entropy_derivative_threshold: 0.3,
            // Strategic Silence defaults
            calm_observation_count: 0,
            silence_variance_threshold: 0.001, // Very stable = variance < 0.1%
        }
    }

    /// Set the entropy derivative threshold for Pre-Storm detection.
    pub fn set_entropy_derivative_threshold(&mut self, threshold: f32) {
        self.entropy_derivative_threshold = threshold;
    }

    /// Get the current 3-point smoothed entropy.
    pub fn smoothed_entropy(&self) -> f32 {
        if self.entropy_ma_count == 0 {
            return 0.0;
        }
        let n = self.entropy_ma_count.min(3);
        let sum: f32 = self.entropy_ma_buffer[..n].iter().sum();
        sum / n as f32
    }

    /// Get the entropy derivative (rate of change of smoothed entropy).
    pub fn entropy_derivative(&self) -> f32 {
        self.smoothed_entropy() - self.prev_smoothed_entropy
    }

    pub fn current_regime(&self) -> Regime {
        self.current_regime
    }

    /// Get the current variance from the observation window.
    pub fn current_variance(&self) -> f32 {
        if self.count < 2 {
            return f32::MAX; // Not enough data
        }
        let n = self.count.min(self.window_size) as f32;
        let mean = self.sum / n;
        let mean_sq = self.sum_sq / n;
        (mean_sq - mean * mean).max(0.0)
    }

    /// Number of consecutive Calm observations.
    pub fn calm_streak(&self) -> usize {
        self.calm_observation_count
    }

    /// Check if the regime is stable enough for strategic silence.
    /// Returns true if:
    /// - In Calm regime for at least 100 observations
    /// - Variance is below the silence threshold (default 0.001)
    pub fn is_stable_enough_for_silence(&self) -> bool {
        self.current_regime == Regime::Calm
            && self.calm_observation_count >= 100
            && self.current_variance() < self.silence_variance_threshold
    }

    /// Set the variance threshold for silence (default: 0.001)
    pub fn set_silence_variance_threshold(&mut self, threshold: f32) {
        self.silence_variance_threshold = threshold;
    }

    /// Update regime based on entropy and throughput.
    ///
    /// Uses a 3-point moving average on entropy to smooth noise, then
    /// checks the derivative (rate of change). If the derivative exceeds
    /// the threshold, triggers Pre-Storm before full Storm is reached.
    ///
    /// # Arguments
    /// * `entropy` - Current raw entropy value
    /// * `packet_size` - Size of the current packet in bytes
    /// * `now_ms` - Current system timestamp in milliseconds
    pub fn update(&mut self, entropy: f32, packet_size: usize, now_ms: u64) {
        // 1. Initialize timer on first run
        if self.last_update_ms == 0 {
            self.last_update_ms = now_ms;
        }

        // 2. Accumulate bytes
        self.accumulated_bytes += packet_size as u64;

        // 3. Check Time Window
        let elapsed = now_ms.saturating_sub(self.last_update_ms);
        if elapsed >= 1000 {
            self.current_throughput = (self.accumulated_bytes as f32) / (elapsed as f32 / 1000.0);
            self.last_update_ms = now_ms;
            self.accumulated_bytes = 0;
        }

        // 4. Update 3-point moving average of entropy
        let old_smoothed = self.smoothed_entropy();
        self.entropy_ma_buffer[self.entropy_ma_idx % 3] = entropy;
        self.entropy_ma_idx = (self.entropy_ma_idx + 1) % 3;
        if self.entropy_ma_count < 3 {
            self.entropy_ma_count += 1;
        }
        self.prev_smoothed_entropy = old_smoothed;

        let smoothed = self.smoothed_entropy();
        let derivative = smoothed - self.prev_smoothed_entropy;

        // 5. Tri-state regime detection with Pre-Storm
        // Storm uses RAW entropy (immediate response to critical levels)
        // Pre-Storm uses DERIVATIVE of smoothed entropy (predictive early warning)
        let new_regime = if entropy > self.entropy_threshold
            || self.current_throughput > self.throughput_threshold
        {
            Regime::Storm
        } else if derivative > self.entropy_derivative_threshold {
            // Entropy is rising fast -> Pre-Storm: preemptively increase adaptation
            Regime::PreStorm
        } else {
            Regime::Calm
        };

        // Update calm observation counter for Strategic Silence
        if new_regime == Regime::Calm {
            self.calm_observation_count = self.calm_observation_count.saturating_add(1);
        } else {
            self.calm_observation_count = 0; // Reset on any non-Calm state
        }

        self.current_regime = new_regime;
    }

    /// Observe a new residual (absolute error).
    /// Returns a RegimeChange event if anomaly detected.
    pub fn observe(&mut self, error: f32) -> RegimeChange {
        let abs_error = error.abs();

        // 1. Check for anomaly BEFORE updating stats (compare against *historical* baseline)
        // Only check if we have enough data (full window)
        let result = if self.count >= self.window_size {
            let mean = self.sum / self.window_size as f32;
            let mean_sq = self.sum_sq / self.window_size as f32;
            // Variance = E[X^2] - (E[X])^2
            let variance = (mean_sq - mean * mean).max(0.0);
            let std_dev = variance.sqrt();

            // Threshold: Mean + 3 * StdDev
            let threshold = mean + 3.0 * std_dev;

            if abs_error > threshold {
                RegimeChange::Drift {
                    current_error: abs_error,
                    threshold,
                }
            } else {
                RegimeChange::None
            }
        } else {
            RegimeChange::None
        };

        // 2. Update Window (Ring Buffer)
        let old_val = self.history[self.idx];
        self.history[self.idx] = abs_error;

        // Update running stats
        self.sum = self.sum - old_val + abs_error;
        self.sum_sq = self.sum_sq - (old_val * old_val) + (abs_error * abs_error);

        // Advance index
        self.idx = (self.idx + 1) % self.window_size;
        self.count += 1;

        result
    }

    pub fn reset(&mut self) {
        self.sum = 0.0;
        self.sum_sq = 0.0;
        self.idx = 0;
        self.count = 0;
        for x in &mut self.history {
            *x = 0.0;
        }
        // Reset throughput tracking
        self.last_update_ms = 0;
        self.accumulated_bytes = 0;
        self.current_throughput = 0.0;
        // Reset entropy MA
        self.entropy_ma_buffer = [0.0; 3];
        self.entropy_ma_idx = 0;
        self.entropy_ma_count = 0;
        self.prev_smoothed_entropy = 0.0;
        // Reset silence tracking
        self.calm_observation_count = 0;
    }
}
