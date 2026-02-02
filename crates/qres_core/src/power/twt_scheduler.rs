//! Target Wake Time (TWT) Scheduler for QRES
//!
//! Coordinates Wi-Fi radio sleep with QRES's regime-aware silence protocol.
//! Provides simulation mode for testing on Linux/Windows without Wi-Fi 6 hardware.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐     regime change      ┌──────────────────┐
//! │ RegimeDetector   │ ──────────────────────► │  TWTScheduler    │
//! │ (Calm/Pre/Storm) │                         │  (sleep/wake)    │
//! └─────────────────┘                         └────────┬─────────┘
//!                                                      │
//!                                      ┌───────────────┼───────────────┐
//!                                      ▼               ▼               ▼
//!                                 ┌─────────┐   ┌───────────┐   ┌──────────┐
//!                                 │ Sentinel │   │ OnDemand  │   │Scheduled │
//!                                 │(always on)│  │(wake-bcast)│  │(periodic)│
//!                                 └─────────┘   └───────────┘   └──────────┘
//! ```
//!
//! # Node Roles
//!
//! - **Sentinel**: Always awake. Monitors for emergencies and broadcasts wake signals.
//! - **OnDemand**: Sleeps until a Sentinel broadcasts an emergency wake.
//! - **Scheduled**: Wakes at periodic TWT intervals determined by the current regime.
//!
//! # Regime-Aware Intervals
//!
//! | Regime    | Interval  | Rationale                                    |
//! |-----------|-----------|----------------------------------------------|
//! | Calm      | 4 hours   | Low entropy, minimal coordination needed     |
//! | PreStorm  | 10 min    | Rising entropy, prepare for convergence      |
//! | Storm     | 30 sec    | Active learning, frequent synchronization    |

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::adaptive::regime_detector::Regime;
use crate::packet::GhostUpdate;

// =============================================================================
// TWT Interval Constants (in milliseconds)
// =============================================================================

/// 4 hours in milliseconds — deep conservation during Calm
const CALM_INTERVAL_MS: u64 = 4 * 60 * 60 * 1000;

/// 10 minutes in milliseconds — elevated readiness during PreStorm
const PRESTORM_INTERVAL_MS: u64 = 10 * 60 * 1000;

/// 30 seconds in milliseconds — rapid coordination during Storm
const STORM_INTERVAL_MS: u64 = 30 * 1000;

/// Maximum jitter as a fraction of the interval (±10%)
const JITTER_FRACTION: f32 = 0.10;

/// Low-reputation nodes wake at most this many times more often than the base
/// interval. A floor factor of 5 means rep=0.0 → interval = base / 5.
const REPUTATION_FLOOR_DIVISOR: f32 = 5.0;

/// Default reputation for new nodes (full trust = full sleep allowance)
const DEFAULT_REPUTATION: f32 = 1.0;

// =============================================================================
// Power Consumption Estimates (milliwatts)
// =============================================================================

/// Wi-Fi radio active TX power (typical Wi-Fi 6 STA)
const RADIO_ACTIVE_MW: f32 = 220.0;

/// Wi-Fi radio idle/listen power
const RADIO_IDLE_MW: f32 = 80.0;

/// Wi-Fi radio TWT sleep power (MAC layer sleep, not deep sleep)
const RADIO_SLEEP_MW: f32 = 5.0;

/// CPU active power estimate (ARM Cortex-A53 class)
const CPU_ACTIVE_MW: f32 = 150.0;

/// CPU idle power estimate
const CPU_IDLE_MW: f32 = 30.0;

// =============================================================================
// Core Types
// =============================================================================

/// TWT configuration for Scheduled nodes
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TWTConfig {
    /// Base wake interval in milliseconds (overridden by regime)
    pub base_interval_ms: u64,
    /// Whether to apply jitter to wake times (±10%)
    pub jitter_enabled: bool,
    /// Maximum number of messages to batch during sleep
    pub max_batch_size: usize,
}

impl Default for TWTConfig {
    fn default() -> Self {
        Self {
            base_interval_ms: CALM_INTERVAL_MS,
            jitter_enabled: true,
            max_batch_size: 64,
        }
    }
}

/// Node role in the TWT hierarchy
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeRole {
    /// Always-on monitor. Responsible for emergency wake broadcasts.
    Sentinel,
    /// Sleeps until a Sentinel issues an emergency wake broadcast.
    OnDemand,
    /// Periodic TWT wake/sleep cycle governed by regime intervals.
    Scheduled(TWTConfig),
}

/// Simulated radio state for testing without Wi-Fi 6 hardware
#[derive(Debug, Clone)]
pub struct MockRadio {
    /// Whether the radio is currently "awake"
    is_awake: bool,
    /// Timestamp (ms) when the radio last went to sleep
    sleep_start_ms: u64,
    /// Total accumulated sleep time in milliseconds
    total_sleep_ms: u64,
    /// Total accumulated awake time in milliseconds
    total_awake_ms: u64,
    /// Number of wake/sleep transitions
    transition_count: u64,
    /// Energy consumed in milliwatt-hours (simulated)
    energy_consumed_mwh: f64,
    /// Last state-change timestamp for energy accounting
    last_transition_ms: u64,
}

impl MockRadio {
    pub fn new() -> Self {
        Self {
            is_awake: true,
            sleep_start_ms: 0,
            total_sleep_ms: 0,
            total_awake_ms: 0,
            transition_count: 0,
            energy_consumed_mwh: 0.0,
            last_transition_ms: 0,
        }
    }

    /// Put the radio to sleep at the given simulated timestamp
    pub fn sleep(&mut self, now_ms: u64) {
        if self.is_awake {
            self.account_energy(now_ms);
            self.is_awake = false;
            self.sleep_start_ms = now_ms;
            self.transition_count += 1;
        }
    }

    /// Wake the radio at the given simulated timestamp
    pub fn wake(&mut self, now_ms: u64) {
        if !self.is_awake {
            self.account_energy(now_ms);
            self.is_awake = true;
            self.transition_count += 1;
        }
    }

    /// Account energy consumption since last transition
    fn account_energy(&mut self, now_ms: u64) {
        let elapsed_ms = now_ms.saturating_sub(self.last_transition_ms);
        if elapsed_ms == 0 {
            return;
        }

        let elapsed_hours = elapsed_ms as f64 / 3_600_000.0;

        if self.is_awake {
            self.total_awake_ms += elapsed_ms;
            // Active: radio idle + CPU active (not TX, just listening)
            self.energy_consumed_mwh +=
                (RADIO_IDLE_MW as f64 + CPU_ACTIVE_MW as f64) * elapsed_hours;
        } else {
            self.total_sleep_ms += elapsed_ms;
            // Sleep: radio TWT sleep + CPU idle
            self.energy_consumed_mwh +=
                (RADIO_SLEEP_MW as f64 + CPU_IDLE_MW as f64) * elapsed_hours;
        }

        self.last_transition_ms = now_ms;
    }

    /// Simulate a burst transmission (adds TX energy cost)
    pub fn account_transmission(&mut self, message_count: usize) {
        // Each message TX takes ~2ms at active power
        let tx_time_hours = (message_count as f64 * 0.002) / 3600.0;
        self.energy_consumed_mwh += RADIO_ACTIVE_MW as f64 * tx_time_hours;
    }

    pub fn is_awake(&self) -> bool {
        self.is_awake
    }

    pub fn total_sleep_ms(&self) -> u64 {
        self.total_sleep_ms
    }

    pub fn total_awake_ms(&self) -> u64 {
        self.total_awake_ms
    }

    pub fn transition_count(&self) -> u64 {
        self.transition_count
    }

    pub fn energy_consumed_mwh(&self) -> f64 {
        self.energy_consumed_mwh
    }

    /// Finalize energy accounting up to the given timestamp.
    /// Call this before reading metrics to ensure accuracy.
    pub fn finalize(&mut self, now_ms: u64) {
        self.account_energy(now_ms);
    }
}

impl Default for MockRadio {
    fn default() -> Self {
        Self::new()
    }
}

/// Aggregated power metrics for reporting
#[derive(Debug, Clone)]
pub struct PowerMetrics {
    /// Fraction of time the radio was asleep (0.0 to 1.0)
    pub radio_sleep_ratio: f32,
    /// Total energy consumed (mWh)
    pub energy_consumed_mwh: f64,
    /// Number of wake/sleep transitions
    pub transition_count: u64,
    /// Messages batched during sleep periods
    pub messages_batched: u64,
    /// Messages sent in bursts on wake
    pub messages_burst_sent: u64,
    /// Estimated energy without TWT (always-on baseline)
    pub baseline_energy_mwh: f64,
    /// Estimated savings as percentage
    pub savings_percent: f32,
}

// =============================================================================
// Gossip Batch Queue
// =============================================================================

/// Queue for batching gossip messages during radio sleep.
///
/// When the radio is asleep, outgoing GhostUpdates are buffered here.
/// On wake, the queue is drained and all messages are burst-transmitted.
#[derive(Debug, Clone)]
pub struct GossipBatchQueue {
    /// Outgoing messages waiting to be sent
    outgoing: VecDeque<GhostUpdate>,
    /// Maximum queue capacity (drop oldest if exceeded)
    max_size: usize,
    /// Total messages ever enqueued
    total_enqueued: u64,
    /// Total messages burst-sent on wake
    total_burst_sent: u64,
}

impl GossipBatchQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            outgoing: VecDeque::with_capacity(max_size.min(256)),
            max_size,
            total_enqueued: 0,
            total_burst_sent: 0,
        }
    }

    /// Enqueue a message for later transmission. Drops oldest if at capacity.
    pub fn enqueue(&mut self, update: GhostUpdate) {
        if self.outgoing.len() >= self.max_size {
            self.outgoing.pop_front(); // Drop oldest
        }
        self.outgoing.push_back(update);
        self.total_enqueued += 1;
    }

    /// Drain all queued messages for burst transmission
    pub fn drain_batch(&mut self) -> Vec<GhostUpdate> {
        let batch: Vec<GhostUpdate> = self.outgoing.drain(..).collect();
        self.total_burst_sent += batch.len() as u64;
        batch
    }

    /// Number of messages currently queued
    pub fn pending_count(&self) -> usize {
        self.outgoing.len()
    }

    pub fn total_enqueued(&self) -> u64 {
        self.total_enqueued
    }

    pub fn total_burst_sent(&self) -> u64 {
        self.total_burst_sent
    }

    pub fn is_empty(&self) -> bool {
        self.outgoing.is_empty()
    }
}

// =============================================================================
// TWT Scheduler
// =============================================================================

/// Target Wake Time Scheduler
///
/// Coordinates radio sleep/wake cycles with QRES regime detection.
/// In simulation mode, uses `MockRadio` to track power without real hardware.
///
/// # Reputation-Weighted Intervals
///
/// Sleep intervals scale with the node's reputation score (0.0–1.0):
/// - High reputation (1.0): full base interval (maximum sleep)
/// - Low reputation (0.0): base / 5 (must stay awake more to contribute)
///
/// Formula: `effective_interval = base * lerp(1/FLOOR, 1.0, reputation)`
///
/// This naturally staggers wake times across a swarm, preventing blind spots.
#[derive(Debug, Clone)]
pub struct TWTScheduler {
    /// Node's role in the TWT hierarchy
    role: NodeRole,
    /// Current regime (drives interval selection)
    current_regime: Regime,
    /// Current wake interval in milliseconds (after reputation weighting)
    current_interval_ms: u64,
    /// Node's reputation score (0.0 = untrusted, 1.0 = fully trusted)
    reputation: f32,
    /// Timestamp of last wake event (ms)
    last_wake_ms: u64,
    /// Timestamp of next scheduled wake (ms), None if always-on
    next_wake_ms: Option<u64>,
    /// Simulated radio for testing
    mock_radio: MockRadio,
    /// Gossip message batch queue
    batch_queue: GossipBatchQueue,
    /// Whether an emergency wake has been received
    emergency_wake_pending: bool,
    /// Simple deterministic PRNG state for jitter (xorshift32)
    jitter_seed: u32,
}

impl TWTScheduler {
    /// Create a new TWT scheduler for the given node role.
    ///
    /// Scheduled nodes start awake and will enter their first sleep on the
    /// next `tick()` after the initial burst window (500ms) elapses.
    /// Default reputation is 1.0 (fully trusted, maximum sleep allowance).
    pub fn new(role: NodeRole) -> Self {
        Self::with_reputation(role, DEFAULT_REPUTATION)
    }

    /// Create a TWT scheduler with a specific initial reputation.
    ///
    /// Reputation (0.0–1.0) scales the sleep interval:
    /// - 1.0 → full base interval (max sleep)
    /// - 0.0 → base / 5 (must contribute more)
    pub fn with_reputation(role: NodeRole, reputation: f32) -> Self {
        let rep = reputation.clamp(0.0, 1.0);
        let interval = match &role {
            NodeRole::Sentinel => 0, // Never sleeps
            NodeRole::OnDemand => 0, // Sleeps indefinitely until woken
            NodeRole::Scheduled(cfg) => {
                calculate_weighted_interval(cfg.base_interval_ms, rep)
            }
        };

        let max_batch = match &role {
            NodeRole::Scheduled(cfg) => cfg.max_batch_size,
            _ => 64,
        };

        // Scheduled nodes record initial wake so tick() can track awake duration
        let initial_wake = match &role {
            NodeRole::Scheduled(_) => 1, // Non-zero so tick() can measure duration
            _ => 0,
        };

        Self {
            role,
            current_regime: Regime::Calm,
            current_interval_ms: interval,
            reputation: rep,
            last_wake_ms: initial_wake,
            next_wake_ms: None,
            mock_radio: MockRadio::new(),
            batch_queue: GossipBatchQueue::new(max_batch),
            emergency_wake_pending: false,
            jitter_seed: 0xDEAD_BEEF, // Deterministic seed for reproducibility
        }
    }

    /// Create a Scheduled node with default TWT config
    pub fn new_scheduled() -> Self {
        Self::new(NodeRole::Scheduled(TWTConfig::default()))
    }

    /// Create a Sentinel node (always awake)
    pub fn new_sentinel() -> Self {
        Self::new(NodeRole::Sentinel)
    }

    /// Create an OnDemand node (wake on broadcast)
    pub fn new_on_demand() -> Self {
        Self::new(NodeRole::OnDemand)
    }

    // -------------------------------------------------------------------------
    // Regime Integration
    // -------------------------------------------------------------------------

    /// Update the scheduler when the regime changes.
    ///
    /// Automatically adjusts wake intervals:
    /// - Calm: 4 hour intervals (deep conservation)
    /// - PreStorm: 10 minute intervals (elevated readiness)
    /// - Storm: 30 second intervals (rapid coordination)
    ///
    /// For Sentinel nodes, this is a no-op (always awake).
    /// For OnDemand nodes, Storm triggers an immediate wake.
    pub fn update_regime(&mut self, new_regime: Regime, now_ms: u64) {
        let old_regime = self.current_regime;
        self.current_regime = new_regime;

        match self.role {
            NodeRole::Sentinel => {
                // Sentinels are always awake, no interval changes
            }
            NodeRole::OnDemand => {
                // OnDemand nodes wake immediately on Storm or PreStorm
                if new_regime != Regime::Calm && !self.mock_radio.is_awake() {
                    self.force_wake(now_ms);
                }
                // Go back to sleep when Calm returns
                if new_regime == Regime::Calm && old_regime != Regime::Calm {
                    self.enter_sleep(now_ms);
                }
            }
            NodeRole::Scheduled(_) => {
                let base = regime_to_interval_ms(new_regime);
                self.current_interval_ms = calculate_weighted_interval(base, self.reputation);

                // If transitioning to a more urgent regime, wake immediately
                if regime_urgency(new_regime) > regime_urgency(old_regime)
                    && !self.mock_radio.is_awake()
                {
                    self.force_wake(now_ms);
                }

                // Recalculate next wake time based on new interval
                self.schedule_next_wake(now_ms);
            }
        }
    }

    /// Get the current regime
    pub fn current_regime(&self) -> Regime {
        self.current_regime
    }

    /// Get the current wake interval in milliseconds (reputation-weighted)
    pub fn current_interval_ms(&self) -> u64 {
        self.current_interval_ms
    }

    /// Get the node's current reputation
    pub fn reputation(&self) -> f32 {
        self.reputation
    }

    /// Update the node's reputation score and recalculate the sleep interval.
    ///
    /// Higher reputation → longer sleep (node has proven reliable).
    /// Lower reputation → shorter sleep (node must contribute more).
    ///
    /// The interval is immediately recalculated but the current sleep/wake
    /// cycle is not interrupted — the new interval takes effect at the next
    /// `schedule_next_wake` call.
    pub fn set_reputation(&mut self, reputation: f32, now_ms: u64) {
        self.reputation = reputation.clamp(0.0, 1.0);
        if let NodeRole::Scheduled(_) = self.role {
            let base = regime_to_interval_ms(self.current_regime);
            self.current_interval_ms = calculate_weighted_interval(base, self.reputation);
            self.schedule_next_wake(now_ms);
        }
    }

    // -------------------------------------------------------------------------
    // Wake/Sleep Logic
    // -------------------------------------------------------------------------

    /// Check whether the node should transmit at the given timestamp.
    ///
    /// Returns `false` if the radio is "asleep" (simulated).
    /// Sentinels always return `true`.
    pub fn should_transmit(&self, _now_ms: u64) -> bool {
        match self.role {
            NodeRole::Sentinel => true,
            _ => self.mock_radio.is_awake(),
        }
    }

    /// Advance the scheduler to the given timestamp.
    ///
    /// For Scheduled nodes, this checks whether it's time to wake up,
    /// performs the wake, drains the batch queue, and schedules the next sleep.
    ///
    /// Returns the number of batched messages ready for burst transmission.
    pub fn tick(&mut self, now_ms: u64) -> usize {
        match self.role {
            NodeRole::Sentinel => 0, // Always awake, no batching
            NodeRole::OnDemand => {
                if self.emergency_wake_pending {
                    self.emergency_wake_pending = false;
                    self.force_wake(now_ms);
                    self.batch_queue.pending_count()
                } else {
                    0
                }
            }
            NodeRole::Scheduled(_) => {
                if let Some(next) = self.next_wake_ms {
                    if now_ms >= next && !self.mock_radio.is_awake() {
                        // Time to wake
                        self.mock_radio.wake(now_ms);
                        self.last_wake_ms = now_ms;
                        let pending = self.batch_queue.pending_count();
                        // Schedule next sleep after burst window (100ms for TX)
                        self.schedule_next_wake(now_ms);
                        return pending;
                    }
                }

                // Check if we should go back to sleep after burst window
                if self.mock_radio.is_awake() && self.last_wake_ms > 0 {
                    let awake_duration = now_ms.saturating_sub(self.last_wake_ms);
                    // Stay awake for a brief burst window (500ms) then sleep
                    let burst_window = if self.current_regime == Regime::Storm {
                        5_000 // 5s during Storm for sustained coordination
                    } else {
                        500 // 500ms normally
                    };
                    if awake_duration > burst_window {
                        self.enter_sleep(now_ms);
                    }
                }

                0
            }
        }
    }

    /// Queue an outgoing GhostUpdate. If the radio is asleep, the message
    /// is batched. If awake, it's still queued for immediate drain.
    pub fn enqueue_gossip(&mut self, update: GhostUpdate) {
        self.batch_queue.enqueue(update);
    }

    /// Drain all batched messages for transmission.
    /// Call this after `tick()` returns > 0, or when the radio is awake.
    pub fn drain_batch(&mut self) -> Vec<GhostUpdate> {
        let batch = self.batch_queue.drain_batch();
        self.mock_radio.account_transmission(batch.len());
        batch
    }

    /// Signal an emergency wake (Sentinel → broadcast to OnDemand nodes)
    pub fn emergency_wake(&mut self, now_ms: u64) {
        match self.role {
            NodeRole::Sentinel => {
                // Sentinels are already awake; this is a no-op for self
            }
            _ => {
                self.emergency_wake_pending = true;
                self.force_wake(now_ms);
            }
        }
    }

    /// Check if the radio is currently awake
    pub fn is_awake(&self) -> bool {
        match self.role {
            NodeRole::Sentinel => true,
            _ => self.mock_radio.is_awake(),
        }
    }

    /// Get the next scheduled wake time (if any)
    pub fn next_wake_ms(&self) -> Option<u64> {
        self.next_wake_ms
    }

    /// Generate a wake schedule: list of wake timestamps from `start_ms` for `count` intervals
    pub fn get_wake_schedule(&self, start_ms: u64, count: usize) -> Vec<u64> {
        if self.current_interval_ms == 0 {
            return Vec::new(); // Sentinel/OnDemand have no fixed schedule
        }
        let mut schedule = Vec::with_capacity(count);
        let mut t = start_ms;
        for _ in 0..count {
            t += self.current_interval_ms;
            schedule.push(t);
        }
        schedule
    }

    // -------------------------------------------------------------------------
    // Metrics
    // -------------------------------------------------------------------------

    /// Get power metrics snapshot. Call with current timestamp for accurate accounting.
    pub fn get_metrics(&mut self, now_ms: u64) -> PowerMetrics {
        self.mock_radio.finalize(now_ms);

        let total_time = self.mock_radio.total_awake_ms() + self.mock_radio.total_sleep_ms();
        let sleep_ratio = if total_time > 0 {
            self.mock_radio.total_sleep_ms() as f32 / total_time as f32
        } else {
            0.0
        };

        // Baseline: always-on energy (radio idle + CPU active) for the same duration
        let total_hours = total_time as f64 / 3_600_000.0;
        let baseline = (RADIO_IDLE_MW as f64 + CPU_ACTIVE_MW as f64) * total_hours;

        let actual = self.mock_radio.energy_consumed_mwh();
        let savings = if baseline > 0.0 {
            ((baseline - actual) / baseline * 100.0) as f32
        } else {
            0.0
        };

        PowerMetrics {
            radio_sleep_ratio: sleep_ratio,
            energy_consumed_mwh: actual,
            transition_count: self.mock_radio.transition_count(),
            messages_batched: self.batch_queue.total_enqueued(),
            messages_burst_sent: self.batch_queue.total_burst_sent(),
            baseline_energy_mwh: baseline,
            savings_percent: savings,
        }
    }

    /// Get a reference to the mock radio (for inspection)
    pub fn mock_radio(&self) -> &MockRadio {
        &self.mock_radio
    }

    /// Get the node role
    pub fn role(&self) -> &NodeRole {
        &self.role
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    fn force_wake(&mut self, now_ms: u64) {
        self.mock_radio.wake(now_ms);
        self.last_wake_ms = now_ms;
    }

    fn enter_sleep(&mut self, now_ms: u64) {
        self.mock_radio.sleep(now_ms);
        self.schedule_next_wake(now_ms);
    }

    fn schedule_next_wake(&mut self, now_ms: u64) {
        if self.current_interval_ms == 0 {
            self.next_wake_ms = None;
            return;
        }

        let jitter = if self.has_jitter_enabled() {
            self.deterministic_jitter()
        } else {
            0
        };

        let interval = self.current_interval_ms;
        // Jitter: ±10% of interval
        let jitter_range = (interval as f32 * JITTER_FRACTION) as i64;
        let jittered = (interval as i64 + (jitter as i64 % (2 * jitter_range + 1)) - jitter_range)
            .max(1) as u64;

        self.next_wake_ms = Some(now_ms + jittered);
    }

    fn has_jitter_enabled(&self) -> bool {
        match &self.role {
            NodeRole::Scheduled(cfg) => cfg.jitter_enabled,
            _ => false,
        }
    }

    /// Xorshift32 PRNG for deterministic jitter
    fn deterministic_jitter(&mut self) -> u32 {
        let mut x = self.jitter_seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.jitter_seed = x;
        x
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Map a regime to its TWT wake interval in milliseconds
pub fn regime_to_interval_ms(regime: Regime) -> u64 {
    match regime {
        Regime::Calm => CALM_INTERVAL_MS,
        Regime::PreStorm => PRESTORM_INTERVAL_MS,
        Regime::Storm => STORM_INTERVAL_MS,
    }
}

/// Calculate a reputation-weighted sleep interval.
///
/// The interval scales linearly from `base / FLOOR` (at reputation 0.0) to
/// `base` (at reputation 1.0). This ensures:
///
/// - **High-reputation nodes** earn longer sleep (they've proven reliable)
/// - **Low-reputation nodes** must wake more frequently to contribute
/// - **Floor guarantee**: even reputation=0.0 gets `base / 5`, never zero
///
/// # Formula
///
/// ```text
/// floor_fraction = 1.0 / REPUTATION_FLOOR_DIVISOR   (= 0.2)
/// weight = floor_fraction + reputation * (1.0 - floor_fraction)
/// interval = base_ms * weight
/// ```
///
/// # Examples
///
/// With Calm base = 4h:
/// - rep=1.0 → 4h (full sleep)
/// - rep=0.5 → 2.4h
/// - rep=0.0 → 48min (base/5)
pub fn calculate_weighted_interval(base_ms: u64, reputation: f32) -> u64 {
    let rep = reputation.clamp(0.0, 1.0);
    let floor_fraction = 1.0 / REPUTATION_FLOOR_DIVISOR; // 0.2
    let weight = floor_fraction + rep * (1.0 - floor_fraction); // 0.2..1.0
    let weighted = base_ms as f64 * weight as f64;
    weighted.round().max(1.0) as u64
}

/// Numeric urgency for regime comparison (higher = more urgent)
fn regime_urgency(regime: Regime) -> u8 {
    match regime {
        Regime::Calm => 0,
        Regime::PreStorm => 1,
        Regime::Storm => 2,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk_proofs::NormProof;
    use curve25519_dalek::edwards::CompressedEdwardsY;
    use curve25519_dalek::scalar::Scalar;
    use curve25519_dalek::traits::Identity;

    fn make_dummy_ghost_update() -> GhostUpdate {
        GhostUpdate {
            peer_id: [0u8; 32],
            masked_weights: alloc::vec![0i32; 4],
            zk_proof: NormProof {
                commitment: CompressedEdwardsY::identity(),
                response: Scalar::ZERO,
            },
            dp_epsilon: 1.0,
        }
    }

    // ---- Interval Calculation Tests ----

    #[test]
    fn test_regime_intervals() {
        assert_eq!(regime_to_interval_ms(Regime::Calm), 4 * 60 * 60 * 1000);
        assert_eq!(regime_to_interval_ms(Regime::PreStorm), 10 * 60 * 1000);
        assert_eq!(regime_to_interval_ms(Regime::Storm), 30 * 1000);
    }

    #[test]
    fn test_sentinel_always_awake() {
        let sched = TWTScheduler::new_sentinel();
        assert!(sched.is_awake());
        assert!(sched.should_transmit(0));
        assert!(sched.should_transmit(999_999_999));
    }

    #[test]
    fn test_scheduled_default_interval() {
        let sched = TWTScheduler::new_scheduled();
        assert_eq!(sched.current_interval_ms(), CALM_INTERVAL_MS);
        assert_eq!(sched.current_regime(), Regime::Calm);
    }

    #[test]
    fn test_regime_change_updates_interval() {
        let mut sched = TWTScheduler::new_scheduled();

        sched.update_regime(Regime::PreStorm, 1000);
        assert_eq!(sched.current_interval_ms(), PRESTORM_INTERVAL_MS);

        sched.update_regime(Regime::Storm, 2000);
        assert_eq!(sched.current_interval_ms(), STORM_INTERVAL_MS);

        sched.update_regime(Regime::Calm, 3000);
        assert_eq!(sched.current_interval_ms(), CALM_INTERVAL_MS);
    }

    // ---- Sleep/Wake Cycle Tests ----

    #[test]
    fn test_scheduled_sleep_wake_cycle() {
        let cfg = TWTConfig {
            base_interval_ms: 1000, // 1 second for testing
            jitter_enabled: false,
            max_batch_size: 16,
        };
        let mut sched = TWTScheduler::new(NodeRole::Scheduled(cfg));

        // Start awake (Scheduled nodes start with last_wake_ms=1)
        assert!(sched.is_awake());

        // After burst window (500ms from wake at ms=1), should go to sleep
        sched.tick(602); // 601ms past wake -> past 500ms burst window
        assert!(!sched.is_awake());

        // Next wake should be scheduled ~1000ms from sleep
        assert!(sched.next_wake_ms().is_some());
    }

    #[test]
    fn test_on_demand_wakes_on_emergency() {
        let mut sched = TWTScheduler::new_on_demand();

        // Manually put to sleep
        sched.mock_radio.sleep(0);
        assert!(!sched.is_awake());

        // Emergency wake
        sched.emergency_wake(100);
        assert!(sched.is_awake());
    }

    #[test]
    fn test_storm_forces_immediate_wake() {
        let cfg = TWTConfig {
            base_interval_ms: 100_000,
            jitter_enabled: false,
            max_batch_size: 16,
        };
        let mut sched = TWTScheduler::new(NodeRole::Scheduled(cfg));

        // Put to sleep
        sched.mock_radio.sleep(0);
        assert!(!sched.is_awake());

        // Storm regime change should force wake
        sched.update_regime(Regime::Storm, 500);
        assert!(sched.is_awake());
        assert_eq!(sched.current_interval_ms(), STORM_INTERVAL_MS);
    }

    // ---- Gossip Batching Tests ----

    #[test]
    fn test_gossip_batching_during_sleep() {
        let mut sched = TWTScheduler::new_scheduled();

        // Put to sleep
        sched.mock_radio.sleep(0);

        // Queue messages while asleep
        for _ in 0..5 {
            sched.enqueue_gossip(make_dummy_ghost_update());
        }

        assert_eq!(sched.batch_queue.pending_count(), 5);

        // Wake and drain
        sched.mock_radio.wake(1000);
        let batch = sched.drain_batch();
        assert_eq!(batch.len(), 5);
        assert_eq!(sched.batch_queue.pending_count(), 0);
    }

    #[test]
    fn test_batch_queue_overflow_drops_oldest() {
        let mut queue = GossipBatchQueue::new(3);
        for i in 0..5 {
            let mut update = make_dummy_ghost_update();
            update.dp_epsilon = i as f32; // Tag each message
            queue.enqueue(update);
        }

        assert_eq!(queue.pending_count(), 3);
        let batch = queue.drain_batch();
        // Should have messages 2, 3, 4 (oldest 0, 1 dropped)
        assert_eq!(batch[0].dp_epsilon, 2.0);
        assert_eq!(batch[1].dp_epsilon, 3.0);
        assert_eq!(batch[2].dp_epsilon, 4.0);
    }

    // ---- Power Metrics Tests ----

    #[test]
    fn test_mock_radio_energy_tracking() {
        let mut radio = MockRadio::new();

        // Awake for 1 hour
        radio.wake(0);
        radio.finalize(3_600_000);

        let energy = radio.energy_consumed_mwh();
        // Should be close to (80 + 150) mW * 1h = 230 mWh
        assert!(energy > 220.0 && energy < 240.0, "energy = {}", energy);
    }

    #[test]
    fn test_sleep_saves_energy_vs_baseline() {
        let mut radio = MockRadio::new();
        // Awake 1 hour, sleep 3 hours
        radio.wake(0);
        radio.sleep(3_600_000);
        radio.finalize(4 * 3_600_000);

        let total_hours = 4.0;
        let baseline = (RADIO_IDLE_MW as f64 + CPU_ACTIVE_MW as f64) * total_hours; // 920 mWh

        let actual = radio.energy_consumed_mwh();
        // 1h awake (230mWh) + 3h sleep (35mW * 3 = 105mWh) = ~335mWh
        assert!(actual < baseline * 0.5, "actual={}, baseline={}", actual, baseline);
    }

    #[test]
    fn test_scheduler_metrics_show_savings() {
        let cfg = TWTConfig {
            base_interval_ms: 1000,
            jitter_enabled: false,
            max_batch_size: 16,
        };
        let mut sched = TWTScheduler::new(NodeRole::Scheduled(cfg));

        // Simulate: awake briefly, then sleep for most of the time
        sched.mock_radio.wake(0);
        sched.mock_radio.sleep(100);  // Awake for 100ms
        // Sleep for ~10 seconds

        let metrics = sched.get_metrics(10_000);
        assert!(metrics.radio_sleep_ratio > 0.9, "sleep_ratio = {}", metrics.radio_sleep_ratio);
        assert!(metrics.savings_percent > 0.0, "savings = {}", metrics.savings_percent);
    }

    // ---- Regime Transition Integration Test ----

    #[test]
    fn test_calm_to_storm_transition_sequence() {
        let cfg = TWTConfig {
            base_interval_ms: CALM_INTERVAL_MS,
            jitter_enabled: false,
            max_batch_size: 32,
        };
        let mut sched = TWTScheduler::new(NodeRole::Scheduled(cfg));

        // Start in Calm — sleeping after initial burst window (last_wake_ms=1 by default)
        sched.tick(602); // Past burst window, enters sleep
        assert!(!sched.is_awake());
        assert_eq!(sched.current_interval_ms(), CALM_INTERVAL_MS);

        // Queue messages while sleeping in Calm
        for _ in 0..3 {
            sched.enqueue_gossip(make_dummy_ghost_update());
        }

        // PreStorm detected — should wake (more urgent than Calm)
        sched.update_regime(Regime::PreStorm, 1000);
        assert!(sched.is_awake());
        assert_eq!(sched.current_interval_ms(), PRESTORM_INTERVAL_MS);

        // Drain batched messages
        let batch = sched.drain_batch();
        assert_eq!(batch.len(), 3);

        // Storm arrives
        sched.update_regime(Regime::Storm, 2000);
        assert!(sched.is_awake());
        assert_eq!(sched.current_interval_ms(), STORM_INTERVAL_MS);

        // Return to Calm
        sched.update_regime(Regime::Calm, 60_000);
        assert_eq!(sched.current_interval_ms(), CALM_INTERVAL_MS);
    }

    // ---- Wake Schedule Tests ----

    #[test]
    fn test_get_wake_schedule() {
        let cfg = TWTConfig {
            base_interval_ms: 1000,
            jitter_enabled: false,
            max_batch_size: 16,
        };
        let sched = TWTScheduler::new(NodeRole::Scheduled(cfg));
        let schedule = sched.get_wake_schedule(0, 5);
        assert_eq!(schedule, vec![1000, 2000, 3000, 4000, 5000]);
    }

    #[test]
    fn test_sentinel_has_no_schedule() {
        let sched = TWTScheduler::new_sentinel();
        let schedule = sched.get_wake_schedule(0, 5);
        assert!(schedule.is_empty());
    }

    // ---- Deterministic Jitter Tests ----

    #[test]
    fn test_jitter_is_deterministic() {
        let mut s1 = TWTScheduler::new_scheduled();
        let mut s2 = TWTScheduler::new_scheduled();

        let j1: Vec<u32> = (0..10).map(|_| s1.deterministic_jitter()).collect();
        let j2: Vec<u32> = (0..10).map(|_| s2.deterministic_jitter()).collect();

        assert_eq!(j1, j2, "Jitter should be deterministic with same seed");
    }

    #[test]
    fn test_jitter_produces_variation() {
        let mut sched = TWTScheduler::new_scheduled();
        let values: Vec<u32> = (0..10).map(|_| sched.deterministic_jitter()).collect();

        // Not all the same
        let first = values[0];
        assert!(values.iter().any(|&v| v != first), "Jitter should produce variation");
    }

    // ---- Reputation-Weighted Interval Tests ----

    #[test]
    fn test_weighted_interval_full_reputation() {
        // rep=1.0 → full base interval
        assert_eq!(calculate_weighted_interval(CALM_INTERVAL_MS, 1.0), CALM_INTERVAL_MS);
        assert_eq!(calculate_weighted_interval(STORM_INTERVAL_MS, 1.0), STORM_INTERVAL_MS);
    }

    #[test]
    fn test_weighted_interval_zero_reputation() {
        // rep=0.0 → base / 5 (floor)
        let expected = CALM_INTERVAL_MS / 5;
        let actual = calculate_weighted_interval(CALM_INTERVAL_MS, 0.0);
        // Allow ±1ms for rounding
        assert!((actual as i64 - expected as i64).unsigned_abs() <= 1,
            "rep=0.0: expected ~{}, got {}", expected, actual);
    }

    #[test]
    fn test_weighted_interval_mid_reputation() {
        // rep=0.5 → base * (0.2 + 0.5 * 0.8) = base * 0.6
        let base = 10_000u64;
        let expected = 6_000u64;
        let actual = calculate_weighted_interval(base, 0.5);
        assert_eq!(actual, expected, "rep=0.5 with base={}", base);
    }

    #[test]
    fn test_weighted_interval_monotonic() {
        // Higher reputation → longer interval (more sleep)
        let base = CALM_INTERVAL_MS;
        let reps = [0.0, 0.1, 0.2, 0.3, 0.5, 0.7, 0.9, 1.0];
        let intervals: Vec<u64> = reps.iter()
            .map(|&r| calculate_weighted_interval(base, r))
            .collect();

        for i in 1..intervals.len() {
            assert!(intervals[i] >= intervals[i - 1],
                "Intervals should be monotonically increasing with reputation: {:?}", intervals);
        }
    }

    #[test]
    fn test_weighted_interval_clamps_out_of_range() {
        let base = 10_000u64;
        // Reputation > 1.0 should clamp to 1.0
        assert_eq!(calculate_weighted_interval(base, 5.0), calculate_weighted_interval(base, 1.0));
        // Reputation < 0.0 should clamp to 0.0
        assert_eq!(calculate_weighted_interval(base, -1.0), calculate_weighted_interval(base, 0.0));
    }

    #[test]
    fn test_scheduler_with_reputation() {
        let cfg = TWTConfig {
            base_interval_ms: CALM_INTERVAL_MS,
            jitter_enabled: false,
            max_batch_size: 32,
        };

        // High reputation → full interval
        let high = TWTScheduler::with_reputation(NodeRole::Scheduled(cfg), 1.0);
        assert_eq!(high.current_interval_ms(), CALM_INTERVAL_MS);
        assert_eq!(high.reputation(), 1.0);

        // Low reputation → shorter interval
        let low = TWTScheduler::with_reputation(NodeRole::Scheduled(cfg), 0.0);
        assert!(low.current_interval_ms() < CALM_INTERVAL_MS);
        assert_eq!(low.reputation(), 0.0);

        // Default constructor → reputation 1.0
        let default = TWTScheduler::new(NodeRole::Scheduled(cfg));
        assert_eq!(default.reputation(), 1.0);
        assert_eq!(default.current_interval_ms(), CALM_INTERVAL_MS);
    }

    #[test]
    fn test_set_reputation_recalculates_interval() {
        let cfg = TWTConfig {
            base_interval_ms: CALM_INTERVAL_MS,
            jitter_enabled: false,
            max_batch_size: 32,
        };
        let mut sched = TWTScheduler::new(NodeRole::Scheduled(cfg));
        assert_eq!(sched.current_interval_ms(), CALM_INTERVAL_MS);

        // Drop reputation → interval shrinks
        sched.set_reputation(0.5, 1000);
        let expected = calculate_weighted_interval(CALM_INTERVAL_MS, 0.5);
        assert_eq!(sched.current_interval_ms(), expected);

        // Regime change with reputation still applied
        sched.update_regime(Regime::Storm, 2000);
        let storm_weighted = calculate_weighted_interval(STORM_INTERVAL_MS, 0.5);
        assert_eq!(sched.current_interval_ms(), storm_weighted);
    }

    #[test]
    fn test_reputation_staggering_different_nodes() {
        // Three nodes with different reputations in Calm → different intervals
        let cfg = TWTConfig {
            base_interval_ms: CALM_INTERVAL_MS,
            jitter_enabled: false,
            max_batch_size: 32,
        };

        let low = TWTScheduler::with_reputation(NodeRole::Scheduled(cfg), 0.1);
        let mid = TWTScheduler::with_reputation(NodeRole::Scheduled(cfg), 0.5);
        let high = TWTScheduler::with_reputation(NodeRole::Scheduled(cfg), 0.9);

        assert!(low.current_interval_ms() < mid.current_interval_ms());
        assert!(mid.current_interval_ms() < high.current_interval_ms());

        // All three should be different
        assert_ne!(low.current_interval_ms(), mid.current_interval_ms());
        assert_ne!(mid.current_interval_ms(), high.current_interval_ms());
    }
}
