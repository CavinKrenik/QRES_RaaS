use crate::inference::hybrid_predictor::HybridPredictor;
use std::path::Path;

// Re-export MovingAveragePredictor for compatibility if needed,
// but primarily it is used internally relative to this module in benchmarks?
// benchmarks usually import `qres_core::resource_management::ResourceUsagePredictor`.
// If they imported `MovingAveragePredictor`, we might break them.
// Let's re-export it.
// pub use crate::inference::heuristic::MovingAveragePredictor;

// =============================================================================
// EnergyPool: Resource-Aware Agentic Swarm (RaaS) Foundation
// =============================================================================
// Deterministic energy accounting based on SNN analysis (0.9 pJ per accumulate)
// Reference: docs/theory/SNN_ENERGY_ANALYSIS.md (21.9x reduction)

/// Energy costs for swarm operations (in abstract energy units)
/// Derived from SNN energy analysis: predict is cheap (spiking), transmission is expensive
pub mod energy_costs {
    pub const PREDICT: u32 = 1; // Sparse event-driven spiking cost
    pub const GOSSIP_SEND: u32 = 50; // Radio/network transmission (high)
    pub const GOSSIP_RECEIVE: u32 = 20; // Processing incoming packets
    pub const ADAPT: u32 = 25; // Weight update cost
    pub const RECHARGE_RATE: u32 = 5; // Energy recovered per Calm tick
    pub const HEARTBEAT: u32 = 5; // Low-cost proof-of-life packet
}

// =============================================================================
// Hardware-Specific Energy Profiles (Phase 6: Hardware Readiness)
// =============================================================================
// These profiles allow calibration against real hardware without recompiling.
// To calibrate: run daemon with USB power meter and log total_energy_consumed
// then compute: units_to_mWh = actual_mWh / total_energy_consumed

/// Hardware-specific energy profile for calibration
#[derive(Debug, Clone, Copy)]
pub struct EnergyProfile {
    /// Human-readable profile name
    pub name: &'static str,
    /// Cost of one prediction inference (pJ or scaled units)
    pub predict_cost: u32,
    /// Cost of sending a gossip packet (radio TX)
    pub gossip_send_cost: u32,
    /// Cost of receiving a gossip packet (radio RX + processing)
    pub gossip_recv_cost: u32,
    /// Cost of adapting/updating weights
    pub adapt_cost: u32,
    /// Idle power drain per tick (always-on overhead)
    pub idle_leak_rate: u32,
    /// Heartbeat cost (low-power proof-of-life)
    pub heartbeat_cost: u32,
    /// Recharge rate per Calm tick
    pub recharge_rate: u32,
}

impl Default for EnergyProfile {
    fn default() -> Self {
        SNN_THEORETICAL_PROFILE
    }
}

/// Theoretical SNN profile from energy analysis
pub const SNN_THEORETICAL_PROFILE: EnergyProfile = EnergyProfile {
    name: "SNN Theoretical (0.9pJ MAC)",
    predict_cost: 1,
    gossip_send_cost: 50,
    gossip_recv_cost: 20,
    adapt_cost: 25,
    idle_leak_rate: 0,
    heartbeat_cost: 5,
    recharge_rate: 5,
};

/// Raspberry Pi Zero 2 W profile (estimated)
/// Based on: ~100mA idle, ~150mA active WiFi TX
pub const PI_ZERO_PROFILE: EnergyProfile = EnergyProfile {
    name: "Raspberry Pi Zero 2 W",
    predict_cost: 2,       // ARM Cortex-A53 inference
    gossip_send_cost: 120, // WiFi radio TX (high power)
    gossip_recv_cost: 45,  // WiFi radio RX
    adapt_cost: 35,        // Memory-bound weight updates
    idle_leak_rate: 10,    // Always-on OS overhead
    heartbeat_cost: 15,    // Minimal WiFi beacon
    recharge_rate: 8,      // Solar panel assumption
};

/// ESP32-S3 profile (estimated)
/// Based on: ~40mA idle (WiFi modem sleep), ~240mA active TX
pub const ESP32_PROFILE: EnergyProfile = EnergyProfile {
    name: "ESP32-S3 (Low Power)",
    predict_cost: 1,       // Optimized TinyML inference
    gossip_send_cost: 200, // WiFi TX (very high relative to idle)
    gossip_recv_cost: 80,  // WiFi RX
    adapt_cost: 20,        // Flash write cost
    idle_leak_rate: 5,     // Modem sleep mode
    heartbeat_cost: 25,    // BLE beacon alternative
    recharge_rate: 3,      // Battery assumption
};

/// Deterministic energy pool for resource-aware decision making
#[derive(Debug, Clone)]
pub struct EnergyPool {
    /// Current energy units
    current: u32,
    /// Maximum capacity
    max_capacity: u32,
    /// Total energy units consumed (lifetime telemetry)
    lifetime_consumption: u64,
}

impl Default for EnergyPool {
    fn default() -> Self {
        Self::new(1000) // Default 1000 units
    }
}

impl EnergyPool {
    /// Create a new energy pool with specified max capacity
    pub fn new(max_capacity: u32) -> Self {
        Self {
            current: max_capacity,
            max_capacity,
            lifetime_consumption: 0,
        }
    }

    /// Current energy level
    pub fn current(&self) -> u32 {
        self.current
    }

    /// Maximum capacity
    pub fn max_capacity(&self) -> u32 {
        self.max_capacity
    }

    /// Current energy as a ratio (0.0 to 1.0)
    pub fn ratio(&self) -> f32 {
        if self.max_capacity == 0 {
            return 0.0;
        }
        self.current as f32 / self.max_capacity as f32
    }

    /// Total lifetime energy consumed
    pub fn lifetime_consumption(&self) -> u64 {
        self.lifetime_consumption
    }

    /// Check if we can afford a cost
    pub fn can_afford(&self, cost: u32) -> bool {
        self.current >= cost
    }

    /// Spend energy, returns true if successful, false if insufficient
    pub fn spend(&mut self, cost: u32) -> bool {
        if self.can_afford(cost) {
            self.current = self.current.saturating_sub(cost);
            self.lifetime_consumption += cost as u64; // Track for telemetry
            true
        } else {
            false
        }
    }

    /// Recharge energy (capped at max_capacity)
    pub fn recharge(&mut self, amount: u32) {
        self.current = (self.current + amount).min(self.max_capacity);
    }

    /// Check if energy is critical (< 10%)
    pub fn is_critical(&self) -> bool {
        self.ratio() < 0.10
    }

    /// Check if energy is low (< 30%)
    pub fn is_low(&self) -> bool {
        self.ratio() < 0.30
    }

    /// Force set energy (for testing)
    pub fn set_energy(&mut self, amount: u32) {
        self.current = amount.min(self.max_capacity);
    }
}

/// Calculate whether a node should broadcast based on utility vs. cost.
///
/// The decision is: `should_broadcast = (entropy * reputation) > (gossip_cost * efficiency_bias)`
///
/// # Arguments
/// * `local_entropy` - Entropy of local prediction error (higher = more surprising/valuable)
/// * `reputation` - Node's reputation score (0-100 scale, higher = more trusted)
/// * `gossip_cost` - Energy cost of sending a gossip packet (e.g., 50 units)
/// * `efficiency_bias` - Multiplier for cost threshold (higher = more aggressive silence, default: 1.0)
///
/// # Returns
/// `true` if the utility of broadcasting exceeds the energy cost threshold
pub fn calculate_broadcast_utility(
    local_entropy: f32,
    reputation: f32,
    gossip_cost: u32,
    efficiency_bias: f32,
) -> bool {
    let utility = local_entropy * reputation;
    let threshold = gossip_cost as f32 * efficiency_bias;
    utility > threshold
}

pub struct ResourceUsagePredictor {
    inner: HybridPredictor,
}

impl ResourceUsagePredictor {
    pub fn new<P: AsRef<Path>>(onnx_path: Option<P>) -> Self {
        // Default threshold of 0.01 (1% variance)
        // If variance is < 0.01 (very smooth), use Heuristic
        // If variance is > 0.01 (chaotic), use Neural
        Self {
            inner: HybridPredictor::new(onnx_path, 0.01),
        }
    }

    /// Hybrid prediction: Try Neural, fallback to Heuristic
    pub fn predict(&self, window: &[f32]) -> f32 {
        self.inner.predict(window)
    }

    /// Force use of heuristic (good for benchmarking baseline)
    pub fn predict_heuristic(&self, window: &[f32]) -> f32 {
        self.inner.predict_heuristic(window)
    }

    /// Force use of neural (good for benchmarking overhead)
    /// Returns None if neural model not loaded or window size mismatch
    pub fn predict_neural(&self, window: &[f32]) -> Option<f32> {
        self.inner.predict_neural(window)
    }
}

// --- Worker Pool Simulation ---
pub struct WorkerPool {
    pub current_capacity: usize,
}

impl Default for WorkerPool {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerPool {
    pub fn new() -> Self {
        Self {
            current_capacity: 2, // Min threads
        }
    }

    /// Adjust capacity based on predicted load.
    /// Load is 0.0 to 1.0 (or higher if overloaded)
    /// Mapping: 0.0 -> 2 threads, 1.0 -> 16 threads.
    pub fn adjust_capacity(&mut self, predicted_load: f32) -> usize {
        // Linear mapping: y = 2 + (x * 14)
        // Clamp x to [0.0, 1.0] for safety approx
        let load = predicted_load.clamp(0.0, 1.2); // Allow slight overprovision if > 1.0

        // Calculate raw target
        let raw_target = 2.0 + (load * 14.0);

        let target = raw_target.round() as usize;
        let clamped = target.clamp(2, 16);

        self.current_capacity = clamped;
        clamped
    }
}

#[cfg(test)]
mod energy_pool_tests {
    use super::*;

    #[test]
    fn test_energy_pool_basics() {
        let mut pool = EnergyPool::new(100);
        assert_eq!(pool.current(), 100);
        assert_eq!(pool.ratio(), 1.0);

        assert!(pool.spend(30));
        assert_eq!(pool.current(), 70);
        assert!((pool.ratio() - 0.7).abs() < 0.01);

        pool.recharge(50);
        assert_eq!(pool.current(), 100); // Capped at max
    }

    #[test]
    fn test_energy_critical_threshold() {
        let mut pool = EnergyPool::new(100);
        pool.set_energy(9);
        assert!(pool.is_critical());

        pool.set_energy(10);
        assert!(!pool.is_critical());
    }

    #[test]
    fn test_insufficient_energy() {
        let mut pool = EnergyPool::new(100);
        pool.set_energy(10);

        assert!(!pool.spend(50)); // Can't afford
        assert_eq!(pool.current(), 10); // Unchanged
    }
}
