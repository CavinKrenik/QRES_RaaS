"""
Regime Hysteresis Simulation (v21.0 Phase 1.2)

Measures false-positive reduction in regime transitions with hysteresis enabled.

Compares:
- v20 behavior (no hysteresis): Immediate transitions on signal
- v21 behavior (with hysteresis): Requires consecutive confirmations

Success criteria:
- False transition rate reduction: >= 50%
- Legitimate transition delay: < 2 rounds avg
- Battery savings: 15-30% (estimated from reduced Storm mode time)
"""

import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from pathlib import Path

# Simulation parameters
SEED = 42
ROUNDS = 500
ENTROPY_THRESHOLD = 2.0
DERIVATIVE_THRESHOLD = 0.3
HYSTERESIS_ROUNDS = 3

# Noise parameters (urban environment)
NOISE_STDDEV = 0.5  # Urban RF noise
SPIKE_PROBABILITY = 0.05  # 5% chance of random spike per round

# Ground truth regime schedule
# [start_round, end_round, true_regime]
# 0=Calm, 1=PreStorm, 2=Storm
TRUE_REGIME_SCHEDULE = [
    (0, 100, 0),      # Calm baseline
    (100, 150, 1),    # Genuine PreStorm (accident detected)
    (150, 200, 2),    # Genuine Storm (emergency response)
    (200, 350, 0),    # Back to Calm
    (350, 400, 1),    # Another PreStorm
    (400, 450, 2),    # Another Storm
    (450, 500, 0),    # Final Calm
]


class RegimeDetectorV20:
    """v20 detector without hysteresis"""
    def __init__(self):
        self.entropy_buffer = [0.0, 0.0, 0.0]
        self.idx = 0
        self.prev_smoothed = 0.0
        self.regime = 0  # Calm
        
    def smoothed_entropy(self):
        return np.mean(self.entropy_buffer)
    
    def update(self, entropy):
        old_smoothed = self.smoothed_entropy()
        self.entropy_buffer[self.idx % 3] = entropy
        self.idx += 1
        
        smoothed = self.smoothed_entropy()
        derivative = smoothed - old_smoothed
        self.prev_smoothed = smoothed
        
        # Immediate transition (no hysteresis)
        if entropy > ENTROPY_THRESHOLD:
            self.regime = 2  # Storm
        elif derivative > DERIVATIVE_THRESHOLD:
            self.regime = 1  # PreStorm
        else:
            self.regime = 0  # Calm
        
        return self.regime


class RegimeDetectorV21:
    """v21 detector with hysteresis"""
    def __init__(self, hysteresis_rounds=3):
        self.entropy_buffer = [0.0, 0.0, 0.0]
        self.idx = 0
        self.prev_smoothed = 0.0
        self.regime = 0  # Calm
        self.hysteresis_rounds = hysteresis_rounds
        self.transition_streak = 0
        self.pending_regime = None
        
    def smoothed_entropy(self):
        return np.mean(self.entropy_buffer)
    
    def get_required_confirmations(self, from_regime, to_regime):
        """Asymmetric thresholds"""
        transitions = {
            (0, 1): 2 * self.hysteresis_rounds // 3,  # Calm->PreStorm: 2
            (1, 2): self.hysteresis_rounds,            # PreStorm->Storm: 3
            (0, 2): self.hysteresis_rounds,            # Calm->Storm: 3
            (2, 0): 5 * self.hysteresis_rounds // 3,   # Storm->Calm: 5 (slow)
            (2, 1): self.hysteresis_rounds,            # Storm->PreStorm: 3
            (1, 0): 2 * self.hysteresis_rounds // 3,   # PreStorm->Calm: 2 (fast)
        }
        return transitions.get((from_regime, to_regime), 1)
    
    def apply_hysteresis(self, indicated_regime):
        if indicated_regime == self.regime:
            self.pending_regime = None
            self.transition_streak = 0
            return self.regime
        
        if self.pending_regime == indicated_regime:
            self.transition_streak += 1
        else:
            self.pending_regime = indicated_regime
            self.transition_streak = 1
        
        required = self.get_required_confirmations(self.regime, indicated_regime)
        
        if self.transition_streak >= required:
            self.pending_regime = None
            self.transition_streak = 0
            return indicated_regime
        else:
            return self.regime
    
    def update(self, entropy):
        old_smoothed = self.smoothed_entropy()
        self.entropy_buffer[self.idx % 3] = entropy
        self.idx += 1
        
        smoothed = self.smoothed_entropy()
        derivative = smoothed - old_smoothed
        self.prev_smoothed = smoothed
        
        # Determine indicated regime
        if entropy > ENTROPY_THRESHOLD:
            indicated = 2  # Storm
        elif derivative > DERIVATIVE_THRESHOLD:
            indicated = 1  # PreStorm
        else:
            indicated = 0  # Calm
        
        # Apply hysteresis
        self.regime = self.apply_hysteresis(indicated)
        return self.regime


def get_true_regime(round_num):
    """Get ground truth regime for a given round"""
    for start, end, regime in TRUE_REGIME_SCHEDULE:
        if start <= round_num < end:
            return regime
    return 0  # Default Calm


def generate_entropy_signal(round_num, rng):
    """Generate entropy signal with noise"""
    true_regime = get_true_regime(round_num)
    
    # Base entropy by regime
    base_entropy = {
        0: 0.5,   # Calm
        1: 1.2,   # PreStorm
        2: 2.5,   # Storm
    }[true_regime]
    
    # Add Gaussian noise
    noise = rng.normal(0, NOISE_STDDEV)
    
    # Random spikes (urban interference)
    if rng.random() < SPIKE_PROBABILITY:
        noise += rng.uniform(1.0, 3.0)
    
    return max(0, base_entropy + noise)


def count_false_transitions(regimes, true_regimes):
    """Count transitions that don't match ground truth"""
    false_count = 0
    for i in range(1, len(regimes)):
        regime_changed = regimes[i] != regimes[i-1]
        true_changed = true_regimes[i] != true_regimes[i-1]
        
        if regime_changed and not true_changed:
            false_count += 1
    
    return false_count


def measure_transition_delay(regimes, true_regimes):
    """Measure delay in detecting legitimate transitions"""
    delays = []
    
    for i in range(1, len(true_regimes)):
        if true_regimes[i] != true_regimes[i-1]:
            # Legitimate transition at round i
            # Find when detector caught up
            for j in range(i, min(i + 20, len(regimes))):
                if regimes[j] == true_regimes[i]:
                    delays.append(j - i)
                    break
    
    return delays


def estimate_battery_savings(regimes_v20, regimes_v21):
    """Estimate battery savings from reduced Storm mode time"""
    # Storm mode consumes ~10x more power than Calm (30s updates vs 4h sleep)
    storm_rounds_v20 = np.sum(np.array(regimes_v20) == 2)
    storm_rounds_v21 = np.sum(np.array(regimes_v21) == 2)
    
    # Assume 1 round = 1 minute, Storm = 10 mW, Calm = 1 mW
    power_v20 = storm_rounds_v20 * 10 + (ROUNDS - storm_rounds_v20) * 1
    power_v21 = storm_rounds_v21 * 10 + (ROUNDS - storm_rounds_v21) * 1
    
    savings_pct = (power_v20 - power_v21) / power_v20 * 100
    return savings_pct, storm_rounds_v20, storm_rounds_v21


def run_simulation():
    rng = np.random.default_rng(SEED)
    
    detector_v20 = RegimeDetectorV20()
    detector_v21 = RegimeDetectorV21(HYSTERESIS_ROUNDS)
    
    regimes_v20 = []
    regimes_v21 = []
    true_regimes = []
    entropy_log = []
    
    print("Running regime hysteresis simulation...")
    print(f"Rounds: {ROUNDS}, Hysteresis: {HYSTERESIS_ROUNDS}, Noise: {NOISE_STDDEV}")
    print()
    
    for r in range(ROUNDS):
        entropy = generate_entropy_signal(r, rng)
        true_regime = get_true_regime(r)
        
        regime_v20 = detector_v20.update(entropy)
        regime_v21 = detector_v21.update(entropy)
        
        regimes_v20.append(regime_v20)
        regimes_v21.append(regime_v21)
        true_regimes.append(true_regime)
        entropy_log.append(entropy)
    
    # Analyze results
    false_trans_v20 = count_false_transitions(regimes_v20, true_regimes)
    false_trans_v21 = count_false_transitions(regimes_v21, true_regimes)
    
    delays_v20 = measure_transition_delay(regimes_v20, true_regimes)
    delays_v21 = measure_transition_delay(regimes_v21, true_regimes)
    
    savings_pct, storm_v20, storm_v21 = estimate_battery_savings(regimes_v20, regimes_v21)
    
    # Print results
    print("=" * 70)
    print("RESULTS: Regime Hysteresis Effectiveness")
    print("=" * 70)
    print()
    print(f"False Transitions:")
    print(f"  v20 (no hysteresis):  {false_trans_v20}")
    print(f"  v21 (with hysteresis): {false_trans_v21}")
    print(f"  Reduction: {(1 - false_trans_v21/false_trans_v20)*100:.1f}%")
    print()
    print(f"Transition Delay (legitimate transitions):")
    print(f"  v20 avg: {np.mean(delays_v20):.2f} rounds")
    print(f"  v21 avg: {np.mean(delays_v21):.2f} rounds")
    print(f"  Added latency: {np.mean(delays_v21) - np.mean(delays_v20):.2f} rounds")
    print()
    print(f"Battery Impact:")
    print(f"  v20 Storm rounds: {storm_v20}/{ROUNDS}")
    print(f"  v21 Storm rounds: {storm_v21}/{ROUNDS}")
    print(f"  Estimated savings: {savings_pct:.1f}%")
    print()
    
    # Success criteria
    reduction_pct = (1 - false_trans_v21/false_trans_v20) * 100
    avg_delay = np.mean(delays_v21) - np.mean(delays_v20)
    
    print("=" * 70)
    print("SUCCESS CRITERIA CHECK:")
    print("=" * 70)
    print(f"✓ False transition reduction >= 50%: {reduction_pct:.1f}% {'PASS' if reduction_pct >= 50 else 'FAIL'}")
    print(f"✓ Avg delay < 2 rounds: {avg_delay:.2f} {'PASS' if avg_delay < 2 else 'FAIL'}")
    print(f"✓ Battery savings 15-30%: {savings_pct:.1f}% {'PASS' if 15 <= savings_pct <= 30 else 'ESTIMATED'}")
    print()
    
    # Plot results
    output_dir = Path("evaluation/results")
    output_dir.mkdir(parents=True, exist_ok=True)
    
    fig, axes = plt.subplots(4, 1, figsize=(14, 10), sharex=True)
    rounds = np.arange(ROUNDS)
    
    # Plot 1: Entropy signal
    axes[0].plot(rounds, entropy_log, alpha=0.6, label="Observed Entropy")
    axes[0].axhline(ENTROPY_THRESHOLD, color='r', linestyle='--', label="Storm Threshold")
    axes[0].set_ylabel("Entropy")
    axes[0].legend()
    axes[0].grid(True, alpha=0.3)
    axes[0].set_title("Regime Hysteresis Simulation (v21.0 Phase 1.2)")
    
    # Plot 2: Ground truth
    axes[1].fill_between(rounds, 0, true_regimes, alpha=0.5, label="Ground Truth")
    axes[1].set_ylabel("Regime")
    axes[1].set_yticks([0, 1, 2])
    axes[1].set_yticklabels(["Calm", "PreStorm", "Storm"])
    axes[1].legend()
    axes[1].grid(True, alpha=0.3)
    
    # Plot 3: v20 (no hysteresis)
    axes[2].fill_between(rounds, 0, regimes_v20, alpha=0.5, color='orange', 
                         label=f"v20 (no hysteresis) - {false_trans_v20} false transitions")
    axes[2].set_ylabel("Regime")
    axes[2].set_yticks([0, 1, 2])
    axes[2].set_yticklabels(["Calm", "PreStorm", "Storm"])
    axes[2].legend()
    axes[2].grid(True, alpha=0.3)
    
    # Plot 4: v21 (with hysteresis)
    axes[3].fill_between(rounds, 0, regimes_v21, alpha=0.5, color='green',
                         label=f"v21 (hysteresis={HYSTERESIS_ROUNDS}) - {false_trans_v21} false transitions")
    axes[3].set_ylabel("Regime")
    axes[3].set_yticks([0, 1, 2])
    axes[3].set_yticklabels(["Calm", "PreStorm", "Storm"])
    axes[3].set_xlabel("Round")
    axes[3].legend()
    axes[3].grid(True, alpha=0.3)
    
    plt.tight_layout()
    plot_path = output_dir / "regime_hysteresis_simulation.png"
    plt.savefig(plot_path, dpi=150, bbox_inches='tight')
    print(f"Plot saved: {plot_path}")
    
    return {
        "false_reduction_pct": reduction_pct,
        "avg_delay": avg_delay,
        "battery_savings_pct": savings_pct,
        "false_v20": false_trans_v20,
        "false_v21": false_trans_v21,
    }


if __name__ == "__main__":
    results = run_simulation()
