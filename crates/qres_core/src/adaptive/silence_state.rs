//! Strategic Silence State Machine
//!
//! Implements the Dynamic Utility-Gated Silence mechanism for RaaS.
//! Nodes transition between Active, Alert, and DeepSilence states
//! based on regime stability and energy levels.

use super::regime_detector::Regime;

/// Silence states for the node
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SilenceState {
    /// Normal gossiping - broadcasting updates regularly
    #[default]
    Active,
    /// PreStorm detected - ready to wake from silence, pre-charging energy
    Alert,
    /// Low entropy, stable regime - conserving energy via minimal communication
    DeepSilence,
}

/// Controller for Strategic Silence state machine
#[derive(Debug, Clone)]
pub struct SilenceController {
    /// Current silence state
    state: SilenceState,
    /// Tick count since last heartbeat
    ticks_since_heartbeat: u64,
    /// Heartbeat interval (ticks) - send proof-of-life every N ticks in DeepSilence
    heartbeat_interval: u64,
    /// Efficiency bias: higher = more aggressive silence (default 1.0)
    efficiency_bias: f32,
}

impl Default for SilenceController {
    fn default() -> Self {
        Self::new()
    }
}

impl SilenceController {
    /// Create a new SilenceController with default settings
    pub fn new() -> Self {
        Self {
            state: SilenceState::Active,
            ticks_since_heartbeat: 0,
            heartbeat_interval: 50, // Default: heartbeat every 50 ticks in DeepSilence
            efficiency_bias: 1.0,
        }
    }

    /// Create with custom heartbeat interval
    pub fn with_heartbeat_interval(mut self, interval: u64) -> Self {
        self.heartbeat_interval = interval;
        self
    }

    /// Set efficiency bias (higher = more aggressive silence)
    pub fn set_efficiency_bias(&mut self, bias: f32) {
        self.efficiency_bias = bias;
    }

    /// Get current silence state
    pub fn state(&self) -> SilenceState {
        self.state
    }

    /// Determine if the node should broadcast based on utility calculation.
    ///
    /// The decision is: `should_broadcast = (entropy * reputation) > (gossip_cost * efficiency_bias)`
    ///
    /// Additionally, in DeepSilence, we only send heartbeats at intervals.
    ///
    /// # Arguments
    /// * `local_entropy` - Entropy of local prediction error (0.0 = perfectly accurate)
    /// * `reputation` - Node's reputation score (0-100 scale)
    /// * `energy_ratio` - Current energy as ratio (0.0 to 1.0)
    /// * `gossip_cost` - Cost of sending a full gossip packet (e.g., 50 units)
    pub fn should_broadcast(
        &mut self,
        local_entropy: f32,
        reputation: f32,
        energy_ratio: f32,
        gossip_cost: u32,
    ) -> bool {
        // Increment tick counter
        self.ticks_since_heartbeat += 1;

        // If energy is critical (<10%), never broadcast
        if energy_ratio < 0.10 {
            return false;
        }

        match self.state {
            SilenceState::Active => {
                // Utility gate: (entropy * reputation) > (cost * efficiency_bias)
                let utility = local_entropy * reputation;
                let threshold = gossip_cost as f32 * self.efficiency_bias;

                // Also broadcast if energy is high (>70%) to contribute to swarm
                utility > threshold || energy_ratio > 0.70
            }
            SilenceState::Alert => {
                // In Alert mode, broadcast if we have moderate energy
                // to help swarm prepare for storm
                energy_ratio > 0.30
            }
            SilenceState::DeepSilence => {
                // Only send heartbeat at intervals
                if self.ticks_since_heartbeat >= self.heartbeat_interval {
                    self.ticks_since_heartbeat = 0;
                    true // Send heartbeat
                } else {
                    false
                }
            }
        }
    }

    /// Check if we should send a heartbeat (low-cost proof-of-life)
    pub fn should_send_heartbeat(&self) -> bool {
        self.state == SilenceState::DeepSilence
            && self.ticks_since_heartbeat >= self.heartbeat_interval
    }

    /// Transition silence state based on regime and stability
    ///
    /// # Arguments
    /// * `regime` - Current regime from RegimeDetector
    /// * `variance_stable` - True if variance is below silence threshold (from is_stable_enough_for_silence())
    /// * `calm_streak` - Number of consecutive Calm observations
    pub fn transition(&mut self, regime: Regime, variance_stable: bool, calm_streak: usize) {
        let new_state = match regime {
            Regime::Storm => {
                // Wake up immediately on Storm
                self.ticks_since_heartbeat = 0; // Reset for fresh start
                SilenceState::Active
            }
            Regime::PreStorm => {
                // Preemptive wake-up: transition to Alert to start foraging energy
                SilenceState::Alert
            }
            Regime::Calm => {
                if variance_stable && calm_streak >= 100 {
                    // Stable enough for deep silence
                    SilenceState::DeepSilence
                } else if calm_streak >= 50 {
                    // Approaching stability, but stay active with reduced output
                    SilenceState::Active
                } else {
                    SilenceState::Active
                }
            }
        };

        // Log state transition if changed
        if new_state != self.state {
            self.state = new_state;
            // Reset heartbeat counter on state change
            self.ticks_since_heartbeat = 0;
        }
    }

    /// Force transition to a specific state (for testing or manual override)
    pub fn set_state(&mut self, state: SilenceState) {
        self.state = state;
        self.ticks_since_heartbeat = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silence_controller_utility_gate() {
        let mut controller = SilenceController::new();

        // High entropy, high reputation -> should broadcast
        assert!(controller.should_broadcast(0.8, 80.0, 0.5, 50));

        // Low entropy, low reputation -> should NOT broadcast (below threshold)
        let mut controller2 = SilenceController::new();
        assert!(!controller2.should_broadcast(0.1, 10.0, 0.5, 50));
    }

    #[test]
    fn test_silence_controller_energy_critical() {
        let mut controller = SilenceController::new();

        // Even with high utility, critical energy prevents broadcast
        assert!(!controller.should_broadcast(1.0, 100.0, 0.05, 50));
    }

    #[test]
    fn test_silence_controller_transitions() {
        let mut controller = SilenceController::new();
        assert_eq!(controller.state(), SilenceState::Active);

        // Transition to DeepSilence when stable
        controller.transition(Regime::Calm, true, 150);
        assert_eq!(controller.state(), SilenceState::DeepSilence);

        // Wake on PreStorm
        controller.transition(Regime::PreStorm, false, 0);
        assert_eq!(controller.state(), SilenceState::Alert);

        // Full wake on Storm
        controller.transition(Regime::Storm, false, 0);
        assert_eq!(controller.state(), SilenceState::Active);
    }

    #[test]
    fn test_silence_controller_heartbeat() {
        let mut controller = SilenceController::new().with_heartbeat_interval(10);
        controller.set_state(SilenceState::DeepSilence);

        // Should not broadcast initially
        for _ in 0..9 {
            assert!(!controller.should_broadcast(0.1, 50.0, 0.5, 50));
        }

        // 10th tick: should send heartbeat
        assert!(controller.should_broadcast(0.1, 50.0, 0.5, 50));
    }
}
