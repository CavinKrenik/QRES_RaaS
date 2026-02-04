//! Robust Aggregation Module for Federated Learning
//!
//! Implements Byzantine-tolerant aggregation algorithms for model updates:
//! - Simple mean averaging (baseline)
//! - Krum algorithm (Phase 2 Item 1 of security roadmap)
//! - Trimmed mean (Phase 2 Item 2, planned)
//! - Median (Phase 2 Item 2, planned)
//!
//! Reference: Blanchard et al., "Machine Learning with Adversaries: Byzantine Tolerant Gradient Descent"

use core::cmp::Ordering;
use fixed::types::I16F16;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::vec::Vec;

/// Trait for pluggable aggregation strategies
///
/// Allows swapping aggregation algorithms at runtime or compile-time.
/// Implementations should be stateless (configuration stored in struct fields).
pub trait Aggregator {
    /// Aggregate multiple model updates into a single update
    fn aggregate(&self, updates: &[Vec<f32>]) -> AggregationResult;

    /// Human-readable name for logging/debugging
    fn name(&self) -> &'static str;
}

/// FedAvg (simple mean) aggregator
#[derive(Clone, Debug, Default)]
pub struct FedAvgAggregator;

impl Aggregator for FedAvgAggregator {
    fn aggregate(&self, updates: &[Vec<f32>]) -> AggregationResult {
        aggregate_updates(updates, &AggregationMode::SimpleMean)
    }

    fn name(&self) -> &'static str {
        "FedAvg"
    }
}

/// Krum aggregator for Byzantine tolerance
#[derive(Clone, Debug)]
pub struct KrumAggregator {
    pub expected_byz: usize,
    pub multi_k: Option<usize>,
}

impl Default for KrumAggregator {
    fn default() -> Self {
        Self {
            expected_byz: 1,
            multi_k: None,
        }
    }
}

impl Aggregator for KrumAggregator {
    fn aggregate(&self, updates: &[Vec<f32>]) -> AggregationResult {
        let mode = match self.multi_k {
            Some(k) => AggregationMode::MultiKrum {
                expected_byz: self.expected_byz,
                k,
            },
            None => AggregationMode::Krum {
                expected_byz: self.expected_byz,
            },
        };
        aggregate_updates(updates, &mode)
    }

    fn name(&self) -> &'static str {
        match self.multi_k {
            Some(_) => "MultiKrum",
            None => "Krum",
        }
    }
}

/// Trimmed Mean aggregator
#[derive(Clone, Debug)]
pub struct TrimmedMeanAggregator {
    pub trim_fraction: f32,
}

impl Default for TrimmedMeanAggregator {
    fn default() -> Self {
        Self { trim_fraction: 0.1 }
    }
}

impl Aggregator for TrimmedMeanAggregator {
    fn aggregate(&self, updates: &[Vec<f32>]) -> AggregationResult {
        aggregate_updates(
            updates,
            &AggregationMode::TrimmedMean {
                trim_fraction: self.trim_fraction,
            },
        )
    }

    fn name(&self) -> &'static str {
        "TrimmedMean"
    }
}

/// Byzantine-tolerant Trimmed Mean aggregator
/// Removes strictly 'f' largest and smallest values per dimension.
#[derive(Clone, Debug)]
pub struct TrimmedMeanByzAggregator {
    pub f: usize,
}

impl Default for TrimmedMeanByzAggregator {
    fn default() -> Self {
        Self { f: 1 }
    }
}

impl Aggregator for TrimmedMeanByzAggregator {
    fn aggregate(&self, updates: &[Vec<f32>]) -> AggregationResult {
        aggregate_updates(updates, &AggregationMode::TrimmedMeanByz { f: self.f })
    }

    fn name(&self) -> &'static str {
        "TrimmedMeanByz"
    }
}

/// Weighted Trimmed Mean aggregator (Sybil-Resistant, Active Defense)
///
/// Each node's contribution is weighted continuously by its reputation score.
/// This provides two key properties:
/// - **Bounded influence**: As `R_i -> 0`, `influence_i -> 0` (no cliff at ban threshold)
/// - **Sybil resistance**: Adding Sybils dilutes per-node power (weights normalized)
///
/// **Order of operations** (frozen for determinism):
/// 1. Sort updates by value per coordinate
/// 2. Trim top `f` and bottom `f` outliers
/// 3. Weight remaining updates by reputation
/// 4. Compute weighted average: `sum(val_i * R_i) / sum(R_i)`
///
/// This is Option A from the security specification: trim first, then weight.
/// Rationale: trimming removes extreme values regardless of reputation,
/// preventing high-reputation compromised nodes from injecting outliers.
#[derive(Clone, Debug)]
pub struct WeightedTrimmedMeanAggregator {
    /// Number of top/bottom values to trim per dimension
    pub f: usize,
    /// Reputation weights per node (same order as updates)
    pub reputation_weights: Vec<f32>,
}

impl WeightedTrimmedMeanAggregator {
    pub fn new(f: usize, reputation_weights: Vec<f32>) -> Self {
        Self {
            f,
            reputation_weights,
        }
    }

    /// Compute the maximum influence a single node can have on the consensus.
    /// Returns `R_i / sum(R_j for j in active set)` for the given node index.
    /// This is an upper bound -- actual influence may be lower due to trimming.
    pub fn max_influence(&self, node_index: usize) -> f32 {
        let n = self.reputation_weights.len();
        if n == 0 {
            return 0.0;
        }
        if self.f * 2 >= n {
            return 1.0 / n as f32;
        }
        let r_i = self
            .reputation_weights
            .get(node_index)
            .copied()
            .unwrap_or(0.0);
        let total: f32 = self.reputation_weights.iter().sum();
        if total <= 0.0 {
            0.0
        } else {
            r_i / total
        }
    }
}

impl Aggregator for WeightedTrimmedMeanAggregator {
    fn aggregate(&self, updates: &[Vec<f32>]) -> AggregationResult {
        weighted_trimmed_mean(updates, self.f, &self.reputation_weights)
    }

    fn name(&self) -> &'static str {
        "WeightedTrimmedMean"
    }
}

/// Weighted trimmed mean: nodes contribute proportionally to their reputation.
/// After trimming top/bottom `f` values per dimension, remaining values are
/// averaged with reputation-based weights.
fn weighted_trimmed_mean(
    updates: &[Vec<f32>],
    f: usize,
    reputation_weights: &[f32],
) -> AggregationResult {
    if updates.is_empty() {
        return AggregationResult {
            weights: Vec::new(),
            selected_indices: Vec::new(),
            rejected_indices: Vec::new(),
        };
    }

    let n = updates.len();
    let d = updates[0].len();

    if f * 2 >= n {
        // Can't trim that much, fallback to weighted mean
        return weighted_mean(updates, reputation_weights);
    }

    let mut result = vec![0.0f32; d];

    for (dim, res_val) in result.iter_mut().enumerate().take(d) {
        // Collect (value, reputation_weight, original_index)
        let mut dim_values: Vec<(f32, f32, usize)> = updates
            .iter()
            .enumerate()
            .map(|(i, u)| {
                let val = u.get(dim).copied().unwrap_or(0.0);
                let rep = reputation_weights.get(i).copied().unwrap_or(0.5);
                (val, rep, i)
            })
            .collect();

        // Sort by value
        dim_values.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(core::cmp::Ordering::Equal));

        // Trim top f and bottom f
        let trimmed = &dim_values[f..(n - f)];

        // Weighted average of remaining
        let total_weight: f32 = trimmed.iter().map(|(_, w, _)| w).sum();
        if total_weight > 0.0 {
            let weighted_sum: f32 = trimmed.iter().map(|(val, w, _)| val * w).sum();
            *res_val = weighted_sum / total_weight;
        }
    }

    AggregationResult {
        weights: result,
        selected_indices: (0..n).collect(),
        rejected_indices: Vec::new(),
    }
}

/// Simple weighted mean (fallback when trimming isn't possible)
fn weighted_mean(updates: &[Vec<f32>], reputation_weights: &[f32]) -> AggregationResult {
    if updates.is_empty() {
        return AggregationResult {
            weights: Vec::new(),
            selected_indices: Vec::new(),
            rejected_indices: Vec::new(),
        };
    }

    let n = updates.len();
    let d = updates[0].len();
    let mut result = vec![0.0f32; d];

    let total_weight: f32 = (0..n)
        .map(|i| reputation_weights.get(i).copied().unwrap_or(0.5))
        .sum();

    if total_weight == 0.0 {
        return AggregationResult {
            weights: result,
            selected_indices: (0..n).collect(),
            rejected_indices: Vec::new(),
        };
    }

    for (i, update) in updates.iter().enumerate() {
        let w = reputation_weights.get(i).copied().unwrap_or(0.5);
        for (j, &val) in update.iter().enumerate() {
            if j < d {
                result[j] += val * w;
            }
        }
    }

    for x in result.iter_mut() {
        *x /= total_weight;
    }

    AggregationResult {
        weights: result,
        selected_indices: (0..n).collect(),
        rejected_indices: Vec::new(),
    }
}

/// Aggregation mode for combining model updates
#[derive(Clone, Debug, Default)]
pub enum AggregationMode {
    /// Simple arithmetic mean (baseline, not robust)
    #[default]
    SimpleMean,
    /// Krum algorithm - selects most representative update
    /// `expected_byz` is the maximum number of Byzantine (malicious) updates expected
    Krum { expected_byz: usize },
    /// Multi-Krum - averages the k most representative updates
    MultiKrum { expected_byz: usize, k: usize },
    /// Coordinate-wise trimmed mean (remove outliers before averaging)
    TrimmedMean { trim_fraction: f32 },
    /// Byzantine-tolerant trimmed mean (remove top/bottom f values)
    TrimmedMeanByz { f: usize },
    /// Coordinate-wise median
    Median,
}

/// Result of aggregation with metadata
#[derive(Clone, Debug)]
pub struct AggregationResult {
    /// The aggregated weights
    pub weights: Vec<f32>,
    /// Indices of updates that were selected (for Krum) or used (for others)
    pub selected_indices: Vec<usize>,
    /// Any updates that were rejected as potential outliers
    pub rejected_indices: Vec<usize>,
}

/// Aggregate multiple model updates using the specified mode
///
/// # Arguments
/// * `updates` - Vector of model weight updates (each is a flattened Vec<f32>)
/// * `mode` - The aggregation algorithm to use
///
/// # Returns
/// Aggregation result containing the combined weights and metadata
pub fn aggregate_updates(updates: &[Vec<f32>], mode: &AggregationMode) -> AggregationResult {
    if updates.is_empty() {
        return AggregationResult {
            weights: Vec::new(),
            selected_indices: Vec::new(),
            rejected_indices: Vec::new(),
        };
    }

    let n = updates.len();
    let d = updates[0].len();

    match mode {
        AggregationMode::SimpleMean => simple_mean(updates, n, d),
        AggregationMode::Krum { expected_byz } => krum(updates, n, d, *expected_byz, 1),
        AggregationMode::MultiKrum { expected_byz, k } => krum(updates, n, d, *expected_byz, *k),
        AggregationMode::TrimmedMean { trim_fraction } => {
            trimmed_mean(updates, n, d, *trim_fraction)
        }
        AggregationMode::TrimmedMeanByz { f } => trimmed_mean_byz(updates, n, d, *f),
        AggregationMode::Median => median_agg(updates, n, d),
    }
}

/// Simple arithmetic mean (baseline)
fn simple_mean(updates: &[Vec<f32>], n: usize, d: usize) -> AggregationResult {
    let mut sum = vec![0.0f32; d];
    for update in updates {
        for (i, &val) in update.iter().enumerate() {
            if i < d {
                sum[i] += val;
            }
        }
    }

    let inv_n = 1.0 / n as f32;
    for x in sum.iter_mut() {
        *x *= inv_n;
    }

    AggregationResult {
        weights: sum,
        selected_indices: (0..n).collect(),
        rejected_indices: Vec::new(),
    }
}

/// Krum algorithm for Byzantine-tolerant aggregation
///
/// For each update, compute the sum of squared distances to its n-q-2 nearest neighbors.
/// Select the update with the smallest sum (most representative).
///
/// For Multi-Krum (k > 1), select the k most representative and average them.
fn krum(
    updates: &[Vec<f32>],
    n: usize,
    d: usize,
    expected_byz: usize,
    k: usize,
) -> AggregationResult {
    let q = expected_byz;

    // Krum requires n > 2q + 2
    if n <= 2 * q + 2 || n < 3 {
        // Fallback to simple mean when we don't have enough updates
        #[cfg(feature = "std")]
        eprintln!(
            "Warning: Too few updates for Krum (n={}, q={}), falling back to mean",
            n, q
        );
        return simple_mean(updates, n, d);
    }

    // Compute pairwise squared Euclidean distances
    let mut distances = vec![vec![0.0f32; n]; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let dist = squared_euclidean(&updates[i], &updates[j]);
            distances[i][j] = dist;
            distances[j][i] = dist;
        }
    }

    // For each update, compute Krum score: sum of distances to n-q-2 nearest neighbors
    let neighbors_count = n - q - 2;
    let mut scores: Vec<(usize, f32)> = Vec::with_capacity(n);

    for (i, row) in distances.iter().enumerate() {
        let mut neighbor_dists: Vec<f32> = row
            .iter()
            .enumerate()
            .filter(|&(j, _)| j != i)
            .map(|(_, &d)| d)
            .collect();

        neighbor_dists.sort_by(|a: &f32, b: &f32| a.partial_cmp(b).unwrap_or(Ordering::Equal));

        // Sum the smallest n-q-2 distances
        let score: f32 = neighbor_dists.iter().take(neighbors_count).sum();
        scores.push((i, score));
    }

    // Sort by score (ascending - smaller is better)
    scores.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(Ordering::Equal));

    // Select k best updates
    let k = k.min(n);
    let selected_indices: Vec<usize> = scores.iter().take(k).map(|(idx, _)| *idx).collect();
    let rejected_indices: Vec<usize> = scores.iter().skip(k).map(|(idx, _)| *idx).collect();

    // Average the selected updates
    let mut result = vec![0.0f32; d];
    for &idx in &selected_indices {
        for (i, &val) in updates[idx].iter().enumerate() {
            if i < d {
                result[i] += val;
            }
        }
    }

    let inv_k = 1.0 / k as f32;
    for x in result.iter_mut() {
        *x *= inv_k;
    }

    AggregationResult {
        weights: result,
        selected_indices,
        rejected_indices,
    }
}

/// Coordinate-wise trimmed mean - removes outliers before averaging
fn trimmed_mean(updates: &[Vec<f32>], n: usize, d: usize, trim_fraction: f32) -> AggregationResult {
    let trim_count = ((n as f32 * trim_fraction) / 2.0).floor() as usize;

    if trim_count * 2 >= n {
        // Can't trim that much, fallback to mean
        return simple_mean(updates, n, d);
    }

    let mut result = vec![0.0f32; d];
    let remaining = n - 2 * trim_count;

    for (dim, res_val) in result.iter_mut().enumerate() {
        // Collect values for this dimension
        let mut values: Vec<f32> = updates
            .iter()
            .map(|u| u.get(dim).copied().unwrap_or(0.0))
            .collect();
        values.sort_by(|a: &f32, b: &f32| a.partial_cmp(b).unwrap_or(Ordering::Equal));

        // Trim extremes and average
        let sum: f32 = values[trim_count..(n - trim_count)].iter().sum();
        *res_val = sum / remaining as f32;
    }

    AggregationResult {
        weights: result,
        selected_indices: (0..n).collect(), // All contribute partially
        rejected_indices: Vec::new(),
    }
}

/// Byzantine-tolerant trimmed mean - removes top/bottom f values
fn trimmed_mean_byz(updates: &[Vec<f32>], n: usize, d: usize, f: usize) -> AggregationResult {
    let trim_count = f;

    if trim_count * 2 >= n {
        // Can't trim that much, fallback to median
        #[cfg(feature = "std")]
        eprintln!(
            "Warning: Too many Byzantine nodes for Trimmed Mean (f={}, n={}). Falling back to Median.",
            f, n
        );
        return median_agg(updates, n, d);
    }

    let mut result = vec![0.0f32; d];
    // Use fixed point division for deterministic result
    let remaining_fixed = I16F16::from_num(n - 2 * trim_count);

    // Check for division by zero (should cover by if check above, but for safety)
    let remaining_inv = if remaining_fixed != 0 {
        I16F16::ONE / remaining_fixed
    } else {
        I16F16::ZERO
    };

    for (dim, res_val) in result.iter_mut().enumerate() {
        // Collect values for this dimension and convert to I16F16 for deterministic sorting/summing
        let mut values: Vec<I16F16> = updates
            .iter()
            .map(|u| I16F16::from_num(u.get(dim).copied().unwrap_or(0.0)))
            .collect();

        values.sort_unstable();

        // Trim extremes and average using fixed point accumulator
        let mut sum = I16F16::ZERO;
        for &val in &values[trim_count..(n - trim_count)] {
            sum = sum.saturating_add(val);
        }

        let avg = sum.saturating_mul(remaining_inv);
        *res_val = avg.to_num();
    }

    AggregationResult {
        weights: result,
        selected_indices: (0..n).collect(),
        rejected_indices: Vec::new(),
    }
}

/// Coordinate-wise median aggregation
fn median_agg(updates: &[Vec<f32>], n: usize, d: usize) -> AggregationResult {
    let mut result = vec![0.0f32; d];

    for (dim, res_val) in result.iter_mut().enumerate() {
        let mut values: Vec<f32> = updates
            .iter()
            .map(|u| u.get(dim).copied().unwrap_or(0.0))
            .collect();
        values.sort_by(|a: &f32, b: &f32| a.partial_cmp(b).unwrap_or(Ordering::Equal));

        *res_val = if n % 2 == 1 {
            values[n / 2]
        } else {
            (values[n / 2 - 1] + values[n / 2]) / 2.0
        };
    }

    AggregationResult {
        weights: result,
        selected_indices: (0..n).collect(),
        rejected_indices: Vec::new(),
    }
}

/// Squared Euclidean distance between two vectors
#[inline]
fn squared_euclidean(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| {
            let diff = x - y;
            diff * diff
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_mean() {
        let updates = vec![
            vec![1.0, 2.0, 3.0],
            vec![2.0, 3.0, 4.0],
            vec![3.0, 4.0, 5.0],
        ];
        let result = aggregate_updates(&updates, &AggregationMode::SimpleMean);
        assert_eq!(result.weights, vec![2.0, 3.0, 4.0]);
        assert_eq!(result.selected_indices.len(), 3);
    }

    #[test]
    fn test_krum_rejects_outlier() {
        // Need n > 2q+2, so for q=1, need n > 4 (n >= 5)
        let updates = vec![
            vec![1.0, 1.0],     // Good
            vec![1.1, 1.1],     // Good
            vec![0.9, 0.9],     // Good
            vec![1.05, 1.05],   // Good
            vec![100.0, 100.0], // Poison/Byzantine
        ];
        let result = aggregate_updates(&updates, &AggregationMode::Krum { expected_byz: 1 });

        // Krum should select one of the good ones (not the outlier at index 4)
        assert!(result.weights[0] < 10.0, "Krum should reject outlier");
        assert!(result.weights[1] < 10.0, "Krum should reject outlier");
        assert!(
            !result.selected_indices.contains(&4),
            "Outlier at index 4 should not be selected"
        );
    }

    #[test]
    fn test_krum_fallback_small_n() {
        // Only 2 updates with q=1, should fallback to mean
        let updates = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let result = aggregate_updates(&updates, &AggregationMode::Krum { expected_byz: 1 });
        assert_eq!(result.weights, vec![2.0, 3.0]); // Falls back to mean
    }

    #[test]
    fn test_multi_krum() {
        let updates = vec![
            vec![1.0, 1.0],
            vec![1.2, 1.2],
            vec![1.1, 1.1],
            vec![100.0, 100.0], // Outlier
            vec![0.9, 0.9],
        ];
        let result = aggregate_updates(
            &updates,
            &AggregationMode::MultiKrum {
                expected_byz: 1,
                k: 3,
            },
        );

        // Should average the 3 best (excluding outlier)
        assert!(
            result.weights[0] < 2.0,
            "Multi-Krum should average good updates"
        );
        assert_eq!(result.selected_indices.len(), 3);
    }

    #[test]
    fn test_trimmed_mean() {
        let updates = vec![
            vec![0.0], // Low outlier
            vec![1.0],
            vec![1.1],
            vec![0.9],
            vec![100.0], // High outlier
        ];
        let result = aggregate_updates(
            &updates,
            &AggregationMode::TrimmedMean { trim_fraction: 0.4 },
        );

        // With 40% trim, we remove 1 from each side, averaging the middle 3
        assert!(
            (result.weights[0] - 1.0).abs() < 0.2,
            "Trimmed mean should be ~1.0"
        );
    }

    #[test]
    fn test_trimmed_mean_byz() {
        // f=1, n=5 -> discards top 1 and bottom 1, averages middle 3
        let updates = vec![
            vec![0.0],   // Discarded (Small)
            vec![1.0],   // Keep
            vec![1.1],   // Keep
            vec![0.9],   // Keep
            vec![100.0], // Discarded (Large)
        ];
        let result = aggregate_updates(&updates, &AggregationMode::TrimmedMeanByz { f: 1 });

        let expected_avg = (1.0 + 1.1 + 0.9) / 3.0;
        assert!(
            (result.weights[0] - expected_avg).abs() < 0.01,
            "Should average middle 3"
        );
    }

    #[test]
    fn test_median() {
        let updates = vec![
            vec![0.0], // Low outlier
            vec![1.0],
            vec![1.1],
            vec![0.9],
            vec![100.0], // High outlier
        ];
        let result = aggregate_updates(&updates, &AggregationMode::Median);

        // Median of [0, 0.9, 1.0, 1.1, 100] = 1.0
        assert_eq!(result.weights[0], 1.0);
    }

    // ================================================================
    // Active Defense Tests (INV-1, INV-2, INV-3 from INVARIANTS.md)
    // ================================================================

    #[test]
    fn test_determinism_weighted_trimmed_mean() {
        // INV-6: Same inputs in same order must produce identical outputs
        let updates = vec![
            vec![1.0, 2.0],
            vec![1.5, 2.5],
            vec![0.5, 1.5],
            vec![100.0, 100.0], // outlier
            vec![1.2, 2.2],
        ];
        let weights = vec![0.9, 0.8, 0.7, 0.1, 0.85];

        let r1 = weighted_trimmed_mean(&updates, 1, &weights);
        let r2 = weighted_trimmed_mean(&updates, 1, &weights);

        assert_eq!(
            r1.weights, r2.weights,
            "Determinism: same input -> same output"
        );
    }

    #[test]
    fn test_monotonicity_higher_rep_more_influence() {
        // Higher reputation should increase a node's contribution.
        // Bookend outliers absorb trimming; the test node sits in the interior.
        let updates = vec![
            vec![-10.0], // Node 0: bookend low (will be trimmed)
            vec![0.3],   // Node 1: biased low — THE node we test
            vec![0.5],   // Node 2: honest
            vec![0.5],   // Node 3: honest
            vec![0.5],   // Node 4: honest
            vec![0.5],   // Node 5: honest
            vec![10.0],  // Node 6: bookend high (will be trimmed)
        ];

        // Case A: Node 1 (value=0.3) has high reputation → pulls result toward 0.3
        let weights_high = vec![0.5, 1.0, 0.5, 0.5, 0.5, 0.5, 0.5];
        let result_high = weighted_trimmed_mean(&updates, 1, &weights_high);

        // Case B: Node 1 (value=0.3) has low reputation → less pull toward 0.3
        let weights_low = vec![0.5, 0.1, 0.5, 0.5, 0.5, 0.5, 0.5];
        let result_low = weighted_trimmed_mean(&updates, 1, &weights_low);

        // With high rep, node 1's "0.3" pulls the weighted average lower
        assert!(
            result_high.weights[0] < result_low.weights[0],
            "Higher reputation should give more influence: high_rep={} < low_rep={}",
            result_high.weights[0],
            result_low.weights[0]
        );
    }

    #[test]
    fn test_bounded_influence_near_zero_rep() {
        // INV-1: Node with near-zero reputation cannot produce large drift.
        // Bookend outliers absorb trimming; the adversarial node sits in the interior.
        //
        // Layout (sorted): [-100, 1.0 x6, 2.0, 100] — trim 1 from each end.
        // Interior: [1.0 x6, 2.0] — the adversarial node (2.0) survives trimming.
        let updates = vec![
            vec![-100.0], // bookend low (trimmed)
            vec![1.0],
            vec![1.0],
            vec![1.0],
            vec![2.0], // Node 4: adversarial but within interior
            vec![1.0],
            vec![1.0],
            vec![1.0],
            vec![100.0], // bookend high (trimmed)
        ];

        // Case A: Node 4 (adversarial) has near-zero reputation
        let mut weights_low = vec![0.8; 9];
        weights_low[4] = 0.001;
        let result_low = weighted_trimmed_mean(&updates, 1, &weights_low);

        // Case B: Node 4 has equal reputation
        let weights_equal = vec![0.8; 9];
        let result_equal = weighted_trimmed_mean(&updates, 1, &weights_equal);

        let drift_low = (result_low.weights[0] - 1.0).abs();
        let drift_equal = (result_equal.weights[0] - 1.0).abs();
        assert!(
            drift_low < drift_equal,
            "Near-zero rep should reduce drift: low={}, equal={}",
            drift_low,
            drift_equal
        );
    }

    #[test]
    fn test_collusion_bounded() {
        // INV-3: Many low-rep colluding nodes cannot overpower fewer high-rep honest nodes
        let mut updates: Vec<Vec<f32>> = Vec::new();
        let mut weights: Vec<f32> = Vec::new();

        // 3 honest nodes (high rep, submit 1.0)
        for _ in 0..3 {
            updates.push(vec![1.0]);
            weights.push(0.95);
        }

        // 7 colluding Byzantine nodes (low rep, all submit 5.0)
        for _ in 0..7 {
            updates.push(vec![5.0]);
            weights.push(0.15);
        }

        let result = weighted_trimmed_mean(&updates, 2, &weights);

        // Despite 7 vs 3, the honest high-rep nodes should dominate
        // Result should be much closer to 1.0 than to 5.0
        assert!(
            result.weights[0] < 3.0,
            "Colluders with low rep should not overpower honest: got {}",
            result.weights[0]
        );
    }

    #[test]
    fn test_max_influence_bound() {
        let weights = vec![0.9, 0.8, 0.1, 0.05, 0.7];
        let agg = WeightedTrimmedMeanAggregator::new(1, weights.clone());

        // Node 2 (rep=0.1): influence should be R_i / sum(R) = 0.1 / 2.55 ≈ 0.039
        let influence = agg.max_influence(2);
        let expected = 0.1 / (0.9 + 0.8 + 0.1 + 0.05 + 0.7);
        assert!(
            (influence - expected).abs() < 0.001,
            "Influence bound: got {}, expected {}",
            influence,
            expected
        );

        // Node with rep=0.05 should have lower influence than node with rep=0.9
        assert!(agg.max_influence(3) < agg.max_influence(0));
    }

    #[test]
    fn test_sybil_dilution() {
        // INV-2: Adding Sybils at default reputation (0.5) should not overpower
        // high-reputation honest nodes. The Byzantine influence is bounded by
        // their reputation weight relative to the honest total.
        let honest_update = vec![1.0];
        let attack_update = vec![1.5]; // Biased within the non-trimmed interior

        // Scenario: 8 honest (high rep) + 4 Byzantine (default rep=0.5), f=1
        // Bookend outliers absorb trimming so all Byzantine updates survive.
        let mut updates: Vec<Vec<f32>> = Vec::new();
        updates.push(vec![-100.0]); // bookend low (trimmed)
        for _ in 0..8 {
            updates.push(honest_update.clone());
        }
        for _ in 0..4 {
            updates.push(attack_update.clone());
        }
        updates.push(vec![100.0]); // bookend high (trimmed)

        let mut weights = vec![0.5]; // bookend
        weights.extend(vec![0.9; 8]); // honest: high rep
        weights.extend(vec![0.5; 4]); // byzantine: default rep
        weights.push(0.5); // bookend

        let result = weighted_trimmed_mean(&updates, 1, &weights);

        // Despite 4 Byzantine vs 8 honest, the honest nodes' higher reputation
        // means the result stays closer to 1.0 than to 1.5.
        // Honest total weight: 8 * 0.9 = 7.2, Byzantine: 4 * 0.5 = 2.0
        // Expected ≈ (7.2 * 1.0 + 2.0 * 1.5) / 9.2 ≈ 1.109
        let drift = (result.weights[0] - 1.0).abs();
        assert!(
            drift < 0.25,
            "4 Sybils at default rep should not cause >0.25 drift from honest mean: got drift={}",
            drift
        );
        // Result should be closer to 1.0 (honest) than to 1.5 (attack)
        assert!(
            result.weights[0] < 1.25,
            "Result should be closer to honest value: got {}",
            result.weights[0]
        );
    }
}
