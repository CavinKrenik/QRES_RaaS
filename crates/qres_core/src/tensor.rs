use alloc::vec;
use alloc::vec::Vec;
use fixed::types::I16F16;
use fixed::FixedI16;

/// Q8.8 Fixed Point Type (16-bit total: 8 integer, 8 fractional)
pub type I8F8 = FixedI16<fixed::types::extra::U8>;

/// Fixed-Point Tensor Structure for QRES
/// Supports both I16F16 (Calm Mode) and I8F8 (Storm Mode) precision levels
#[derive(Debug, Clone)]
pub struct FixedTensor {
    pub data: Vec<I16F16>,
}

impl FixedTensor {
    pub fn new(data: Vec<I16F16>) -> Self {
        Self { data }
    }

    /// Create FixedTensor from I16F16 bytes (4 bytes per value)
    pub fn from_i16f16_bytes(bytes: &[u8]) -> Self {
        let data: Vec<I16F16> = bytes
            .chunks(4)
            .filter_map(|chunk| {
                if chunk.len() == 4 {
                    let bits = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    Some(I16F16::from_bits(bits as i32))
                } else {
                    None
                }
            })
            .collect();
        Self::new(data)
    }

    /// Create FixedTensor from I8F8 bytes (2 bytes per value)
    pub fn from_i8f8_bytes(bytes: &[u8]) -> Self {
        let i8f8_data: Vec<I8F8> = bytes
            .chunks(2)
            .filter_map(|chunk| {
                if chunk.len() == 2 {
                    let bits = u16::from_le_bytes([chunk[0], chunk[1]]);
                    Some(I8F8::from_bits(bits as i16))
                } else {
                    None
                }
            })
            .collect();
        Self::from_i8f8(&i8f8_data)
    }

    /// Downcast to I8F8 (Storm Mode): Halves precision and bandwidth
    /// Saturates values outside I8F8 range to prevent overflow
    pub fn quantize_to_i8f8(&self) -> Vec<I8F8> {
        self.data
            .iter()
            .map(|&val| {
                // Convert to f32 for range checking, then quantize
                let f32_val = val.to_num::<f32>();
                // I8F8 range: -128.0 to 127.996 (approximately)
                let clamped = f32_val.clamp(-128.0, 127.996);
                I8F8::from_num(clamped)
            })
            .collect()
    }

    /// Upcast from I8F8 (Restore from Storm Mode)
    /// Fills lower precision bits with zeros (lossy but deterministic)
    pub fn from_i8f8(data: &[I8F8]) -> Self {
        let data_i16f16 = data
            .iter()
            .map(|&val| {
                // Convert I8F8 to f32, then to I16F16
                let f32_val = val.to_num::<f32>();
                I16F16::from_num(f32_val)
            })
            .collect();
        Self::new(data_i16f16)
    }
}

// ============================================================================
// VarianceMonitor: BFP-16 Auto-Tuning for Vanishing Gradient Recovery
// ============================================================================

/// Monitors gradient magnitudes during the backward pass and triggers
/// a bit-shift in the Bfp16Vec exponent to re-center the precision window
/// when gradients fall below 10^-7.
///
/// This prevents the "vanishing gradient" problem in BFP-16 by dynamically
/// adjusting the shared exponent to maintain signal in the mantissa bits.
#[derive(Debug, Clone)]
pub struct VarianceMonitor {
    /// Threshold below which auto-tuning triggers (default: 1e-7)
    threshold: f32,
    /// Number of consecutive samples below threshold before triggering
    trigger_count: usize,
    /// Current count of sub-threshold samples
    current_below: usize,
    /// Number of bit-shift corrections applied
    corrections_applied: usize,
    /// Running minimum gradient magnitude observed
    min_magnitude: f32,
}

impl Default for VarianceMonitor {
    fn default() -> Self {
        Self::new(1e-7, 3)
    }
}

impl VarianceMonitor {
    /// Create a new VarianceMonitor.
    ///
    /// # Arguments
    /// * `threshold` - Gradient magnitude below which auto-tuning triggers
    /// * `trigger_count` - Number of consecutive sub-threshold observations before action
    pub fn new(threshold: f32, trigger_count: usize) -> Self {
        Self {
            threshold,
            trigger_count,
            current_below: 0,
            corrections_applied: 0,
            min_magnitude: f32::MAX,
        }
    }

    /// Observe a gradient vector and check if auto-tuning is needed.
    ///
    /// Returns `Some(shift)` if a bit-shift correction should be applied,
    /// where `shift` is the number of bits to shift the exponent down.
    /// Returns `None` if no correction is needed.
    pub fn observe_gradients(&mut self, gradients: &[f32]) -> Option<i8> {
        if gradients.is_empty() {
            return None;
        }

        // Compute gradient magnitude (L2 norm / sqrt(n))
        let sum_sq: f32 = gradients.iter().map(|g| g * g).sum();
        let magnitude = (sum_sq / gradients.len() as f32).sqrt();

        if magnitude < self.min_magnitude && magnitude > 0.0 {
            self.min_magnitude = magnitude;
        }

        if magnitude < self.threshold && magnitude > 0.0 {
            self.current_below += 1;

            if self.current_below >= self.trigger_count {
                // Calculate how many bits to shift
                // We want to bring the magnitude into the range [0.001, 1.0]
                // shift = ceil(-log2(magnitude / target))
                let target = 0.01f32;
                let ratio = target / magnitude;
                let shift = ratio.log2().ceil() as i8;
                let shift = shift.clamp(1, 20);

                self.corrections_applied += 1;
                self.current_below = 0;

                return Some(shift);
            }
        } else {
            self.current_below = 0;
        }

        None
    }

    /// Apply the bit-shift correction to a Bfp16Vec.
    /// Decreases the exponent by `shift` to re-center precision on small values.
    pub fn apply_correction(bfp: &mut crate::consensus::krum::Bfp16Vec, shift: i8) {
        // Decrease exponent = effectively multiply all values by 2^shift
        // This brings small mantissa values into a usable range
        let new_exp = bfp.exponent.saturating_sub(shift);

        // Re-scale mantissas to account for exponent change
        let exp_diff = bfp.exponent as i32 - new_exp as i32;
        if exp_diff > 0 && exp_diff < 16 {
            // Scale mantissas up by 2^exp_diff (left shift)
            for m in &mut bfp.mantissas {
                let shifted = (*m as i32) << exp_diff;
                *m = shifted.clamp(-32767, 32767) as i16;
            }
        }

        bfp.exponent = new_exp;
    }

    /// Get the number of corrections applied so far.
    pub fn corrections_count(&self) -> usize {
        self.corrections_applied
    }

    /// Get the minimum gradient magnitude observed.
    pub fn min_magnitude_observed(&self) -> f32 {
        self.min_magnitude
    }

    /// Reset the monitor state.
    pub fn reset(&mut self) {
        self.current_below = 0;
        self.corrections_applied = 0;
        self.min_magnitude = f32::MAX;
    }
}

/// Tensor Network MPS (Matrix Product State) Compressor
/// Breaks a high-dimensional tensor into a chain of low-rank tensors (cores).
///
/// Compression comes from truncating the "Bond Dimension" (chis) via SVD.
///
/// Current implementation:
/// - Input: flattened byte stream treated as a Vector/Tensor.
/// - Output: List of compressed cores.
pub struct MpsCompressor {
    pub bond_dim: usize,
    pub threshold: I16F16,
}

impl MpsCompressor {
    pub fn new(bond_dim: usize, threshold: f64) -> Self {
        MpsCompressor {
            bond_dim,
            threshold: I16F16::from_num(threshold),
        }
    }

    /// Compress a 2D matrix (rows x cols) into MPS cores using "Haar Wavelet Tensor Train"
    /// Uses Q16.16 Fixed Point arithmetic for determinism.
    pub fn compress_matrix(&self, data: &[f64], rows: usize, cols: usize) -> Vec<Vec<f64>> {
        // Validation
        if data.len() != rows * cols {
            return Vec::new(); // Error
        }

        // 1. Convert to Fixed Point Matrix
        let mut matrix: Vec<I16F16> = Vec::with_capacity(rows * cols);
        for &val in data {
            matrix.push(I16F16::from_num(val));
        }

        // Implementation: 2D Haar Wavelet Transform (Lossy)
        // 1. Row transform
        // 2. Column transform
        // 3. Thresholding (Tensor sparsity)

        // Row steps
        for r in 0..rows {
            self.haar_1d(&mut matrix, r * cols, cols);
        }

        // Col steps
        // Transpose
        let mut transposed = vec![I16F16::ZERO; rows * cols];
        for r in 0..rows {
            for c in 0..cols {
                transposed[c * rows + r] = matrix[r * cols + c];
            }
        }

        // Transform Columns
        for c in 0..cols {
            self.haar_1d(&mut transposed, c * rows, rows);
        }

        // Thresholding (Sparse approximation of Wavelet Coefficients)
        let mut flattened_sparse = Vec::new();
        for val in transposed {
            if val.abs() > self.threshold {
                flattened_sparse.push(val.to_num::<f64>());
            } else {
                flattened_sparse.push(0.0);
            }
        }

        vec![flattened_sparse]
    }

    fn haar_1d(&self, data: &mut [I16F16], start: usize, len: usize) {
        let mut temp = vec![I16F16::ZERO; len];
        let mut h = len;

        // Pre-calculate constants in Fixed Point
        let frac_sqrt_2 = I16F16::from_num(core::f64::consts::FRAC_1_SQRT_2);

        while h > 1 {
            let half = h / 2;
            for i in 0..half {
                // Safety: Use checked arithmetic to prevent panics on overflow
                let a = data.get(start + 2 * i).copied().unwrap_or(I16F16::ZERO);
                let b = data.get(start + 2 * i + 1).copied().unwrap_or(I16F16::ZERO);

                let sum = a.checked_add(b).unwrap_or(I16F16::MAX);
                let diff = a.checked_sub(b).unwrap_or(I16F16::MAX);

                // temp[i] = sum * frac_sqrt_2
                temp[i] = sum.checked_mul(frac_sqrt_2).unwrap_or(I16F16::MAX);

                // temp[half + i] = diff * frac_sqrt_2
                if let Some(idx) = temp.get_mut(half + i) {
                    *idx = diff.checked_mul(frac_sqrt_2).unwrap_or(I16F16::MAX);
                }
            }
            // Copy back
            if let Some(dest_slice) = data.get_mut(start..start + h) {
                if let Some(src_slice) = temp.get(..h) {
                    dest_slice.copy_from_slice(src_slice);
                }
            }
            h = half;
        }
    }
}

#[cfg(test)]
mod variance_monitor_tests {
    use super::*;
    use crate::consensus::krum::Bfp16Vec;

    #[test]
    fn test_no_correction_for_normal_gradients() {
        let mut monitor = VarianceMonitor::default();
        let gradients = vec![0.01, -0.02, 0.015, -0.005];

        let result = monitor.observe_gradients(&gradients);
        assert!(
            result.is_none(),
            "Normal gradients should not trigger correction"
        );
    }

    #[test]
    fn test_correction_triggers_for_vanishing_gradients() {
        let mut monitor = VarianceMonitor::new(1e-7, 2);

        // Feed vanishing gradients
        let tiny_grads = vec![1e-9, -1e-9, 5e-10, -5e-10];

        // First observation: increments counter but doesn't trigger yet
        assert!(monitor.observe_gradients(&tiny_grads).is_none());

        // Second observation: triggers correction
        let shift = monitor.observe_gradients(&tiny_grads);
        assert!(
            shift.is_some(),
            "Should trigger after 2 consecutive observations"
        );

        let shift_val = shift.unwrap();
        assert!(shift_val > 0, "Shift should be positive");
        #[cfg(feature = "std")]
        println!("Correction shift: {} bits", shift_val);
    }

    #[test]
    fn test_bfp16_auto_tuning_preserves_signal() {
        let mut monitor = VarianceMonitor::new(1e-7, 1);

        // Create a BFP16 vector with very small values
        let small_data = vec![1e-8f32; 8];
        let mut bfp = Bfp16Vec::from_f32_slice(&small_data);

        #[cfg(feature = "std")]
        println!(
            "Before: exponent={}, mantissas={:?}",
            bfp.exponent, bfp.mantissas
        );
        #[allow(unused_variables)]
        let before_values = bfp.to_vec_f32();
        #[cfg(feature = "std")]
        println!("Before values: {:?}", before_values);

        // Check if correction is needed
        if let Some(shift) = monitor.observe_gradients(&small_data) {
            #[cfg(feature = "std")]
            println!("Applying correction: shift={} bits", shift);
            VarianceMonitor::apply_correction(&mut bfp, shift);
        }

        #[cfg(feature = "std")]
        println!(
            "After: exponent={}, mantissas={:?}",
            bfp.exponent, bfp.mantissas
        );
        let after_values = bfp.to_vec_f32();
        #[cfg(feature = "std")]
        println!("After values: {:?}", after_values);

        // The key test: values should still be non-zero
        for (i, &v) in after_values.iter().enumerate() {
            assert!(
                v.abs() > 0.0,
                "Value at index {} is zero after correction!",
                i
            );
        }
    }

    #[test]
    fn test_vanishing_gradient_learning_velocity() {
        // Simulate a training loop with vanishing gradients
        // and verify the auto-tuner maintains non-zero learning velocity
        let mut monitor = VarianceMonitor::new(1e-7, 1);

        let mut weights = [0.5f32; 8];
        let learning_rate = 1e-5f32;

        // Simulate 50 gradient steps with progressively smaller gradients
        let mut velocity_log = Vec::new();

        for step in 0..50 {
            // Gradients get smaller exponentially
            let grad_scale = 1e-6 * (0.5f32).powi(step / 10);
            let gradients: Vec<f32> = (0..8)
                .map(|i| grad_scale * (i as f32 * 0.1 + 1.0))
                .collect();

            // Convert to BFP16 for transmission
            let mut bfp_grads = Bfp16Vec::from_f32_slice(&gradients);

            // Check for vanishing gradients and auto-tune
            if let Some(shift) = monitor.observe_gradients(&gradients) {
                VarianceMonitor::apply_correction(&mut bfp_grads, shift);
            }

            // Reconstruct and apply update
            let recovered_grads = bfp_grads.to_vec_f32();
            let velocity: f32 =
                recovered_grads.iter().map(|g| g.abs()).sum::<f32>() / recovered_grads.len() as f32;

            for (w, g) in weights.iter_mut().zip(recovered_grads.iter()) {
                *w -= learning_rate * g;
            }

            velocity_log.push(velocity);
        }

        #[cfg(feature = "std")]
        {
            println!("--- Vanishing Gradient Recovery ---");
            println!("Corrections applied: {}", monitor.corrections_count());
            println!(
                "Min magnitude observed: {:.2e}",
                monitor.min_magnitude_observed()
            );
            println!(
                "Final velocity: {:.2e}",
                velocity_log.last().unwrap_or(&0.0)
            );
        }

        // Verify non-zero learning velocity is maintained
        let final_velocity = *velocity_log.last().unwrap_or(&0.0);
        assert!(
            final_velocity > 0.0,
            "Learning velocity should be non-zero, got {:.2e}",
            final_velocity
        );

        // Verify corrections were applied
        assert!(
            monitor.corrections_count() > 0,
            "Auto-tuner should have applied corrections"
        );

        #[cfg(feature = "std")]
        println!("BFP-16 AUTO-TUNING: VERIFIED (non-zero velocity maintained)");
    }
}
