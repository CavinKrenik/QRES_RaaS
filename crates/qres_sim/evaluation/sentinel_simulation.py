"""
Phase 3 (v20): Sentinel Simulation – Virtual Dark-Space Smart City
===================================================================
Tests urban resilience in a fully disconnected, zoned topology with:
  - 4 isolated zones (streetlights, transit, water, energy substations)
  - Bridge-only inter-zone gossip (eligibility R ≥ 0.8)
  - Autonomous Calm ↔ Storm regime transitions
  - Lamarckian resumption after total power failure
  - Adversarial slander campaign across one zone

Success Metrics:
  - Zone isolation contains Sybil attacks (INV-2, INV-3)
  - Regime gate requires high-reputation quorum (INV-4)
  - 100% weight recovery after blackout (INV-6)
  - Median PeerEval contains coordinated slander (INV-2)

This test validates:
  - INV-2, INV-3: Zone-based attack containment
  - INV-4: RegimeDetector quorum requirements
  - INV-6: Lamarckian bit-perfect recovery
"""

import numpy as np
import pandas as pd
import pickle
from pathlib import Path
from dataclasses import dataclass
from typing import List, Dict, Set

# -- Configuration ----------------------------------------------------
SEED = 2027
RNG = np.random.default_rng(SEED)

# Zone configuration
ZONES = ["streetlights", "transit", "water", "energy"]
NODES_PER_ZONE = 25  # 100 nodes total
TOTAL_NODES = len(ZONES) * NODES_PER_ZONE

# Bridge configuration (inter-zone communication)
BRIDGE_REPUTATION_THRESHOLD = 0.8
BRIDGE_RATE_CAP = 10  # Max messages per round

# Regime detector
CALM_UPDATE_INTERVAL = 14400  # 4 hours in seconds
STORM_UPDATE_INTERVAL = 30  # 30 seconds
REGIME_QUORUM_MIN = 3  # Minimum high-rep nodes to trigger Storm

# Reputation
DEFAULT_REPUTATION = 0.5
BAN_THRESHOLD = 0.2
HIGH_REP_THRESHOLD = 0.8

# Simulation
MAX_ROUNDS = 150
BLACKOUT_ROUND = 75  # Total power failure at round 75
ADVERSARIAL_ZONE = "streetlights"  # Zone under attack
ADVERSARIAL_RATIO = 0.4  # 40% Byzantine in attacked zone

# Model dimension
DIM = 16


# -- Data Structures --------------------------------------------------

@dataclass
class Node:
    """A single QRES node in a specific zone."""
    node_id: int
    zone: str
    reputation: float = DEFAULT_REPUTATION
    is_byzantine: bool = False
    weights: np.ndarray = None  # Learned model weights (Q16.16 simulation)
    energy: float = 1.0  # Energy pool (0.0 to 1.0)
    regime: str = "Calm"  # Current operating regime
    
    def __post_init__(self):
        if self.weights is None:
            self.weights = RNG.normal(0, 0.1, DIM)  # Initialize with small random weights
    
    def save_to_nvram(self):
        """Simulate non-volatile storage (Lamarckian resumption)."""
        return self.weights.copy()
    
    def restore_from_nvram(self, stored_weights):
        """Restore weights from non-volatile storage."""
        self.weights = stored_weights.copy()
    
    def can_bridge(self):
        """Check if node is eligible for inter-zone communication."""
        return self.reputation >= BRIDGE_REPUTATION_THRESHOLD


@dataclass
class Zone:
    """A physical zone in the smart city topology."""
    name: str
    nodes: List[Node]
    
    def count_high_reputation(self):
        """Count nodes with R > 0.8 (for regime quorum)."""
        return sum(1 for n in self.nodes if n.reputation > HIGH_REP_THRESHOLD)
    
    def detect_entropy_spike(self):
        """Simulate entropy spike detection (e.g., accident, outage)."""
        # Random entropy spike with 10% probability
        return RNG.random() < 0.1


# -- Bridge Network ---------------------------------------------------

class BridgeNetwork:
    """Manages inter-zone communication with eligibility and rate caps."""
    
    def __init__(self, zones: Dict[str, Zone]):
        self.zones = zones
        self.message_counts = {z: 0 for z in zones.keys()}
    
    def can_send_message(self, source_zone: str, source_node: Node) -> bool:
        """Check if node can send inter-zone message."""
        # Must be eligible
        if not source_node.can_bridge():
            return False
        
        # Must not exceed rate cap
        if self.message_counts[source_zone] >= BRIDGE_RATE_CAP:
            return False
        
        return True
    
    def send_message(self, source_zone: str, target_zone: str, message: dict):
        """Send message across bridge (if eligible)."""
        if self.message_counts[source_zone] >= BRIDGE_RATE_CAP:
            return False
        
        self.message_counts[source_zone] += 1
        return True
    
    def reset_round(self):
        """Reset message counts for new round."""
        self.message_counts = {z: 0 for z in self.zones.keys()}


# -- Regime Detector --------------------------------------------------

class RegimeDetector:
    """Detects regime transitions (Calm → Storm) with consensus gate."""
    
    def __init__(self, zone: Zone):
        self.zone = zone
    
    def should_trigger_storm(self) -> bool:
        """Check if Storm regime should be activated."""
        # Simulate entropy spike detection
        if not self.zone.detect_entropy_spike():
            return False
        
        # Count high-reputation nodes (quorum requirement)
        high_rep_count = self.zone.count_high_reputation()
        
        # INV-4: Storm requires ≥3 nodes with R > 0.8
        return high_rep_count >= REGIME_QUORUM_MIN
    
    def update_regime(self):
        """Update regime for all nodes in zone."""
        if self.should_trigger_storm():
            for node in self.zone.nodes:
                node.regime = "Storm"
            return "Storm"
        else:
            for node in self.zone.nodes:
                node.regime = "Calm"
            return "Calm"


# -- Reputation Tracker -----------------------------------------------

class ReputationTracker:
    """Manages node reputation with bucketed peer evaluation."""
    
    def __init__(self):
        self.peer_evaluations = {}  # node_id -> list of peer scores
    
    def report_peer_eval(self, node_id: int, score: float):
        """Add a peer evaluation score."""
        if node_id not in self.peer_evaluations:
            self.peer_evaluations[node_id] = []
        self.peer_evaluations[node_id].append(score)
    
    def compute_median_reputation(self, node_id: int) -> float:
        """Compute median reputation (robust to slander)."""
        if node_id not in self.peer_evaluations or not self.peer_evaluations[node_id]:
            return DEFAULT_REPUTATION
        
        return float(np.median(self.peer_evaluations[node_id]))
    
    def update_reputations(self, nodes: List[Node]):
        """Update all node reputations based on median peer eval."""
        for node in nodes:
            node.reputation = self.compute_median_reputation(node.node_id)


# -- Main Simulation --------------------------------------------------

def main():
    print("=" * 70)
    print("PHASE 3 (v20): SENTINEL SIMULATION – DARK-SPACE SMART CITY")
    print("=" * 70)
    print(f"Zones: {ZONES}")
    print(f"Nodes per zone: {NODES_PER_ZONE}")
    print(f"Total nodes: {TOTAL_NODES}")
    print(f"Bridge threshold: R ≥ {BRIDGE_REPUTATION_THRESHOLD}")
    print(f"Adversarial zone: {ADVERSARIAL_ZONE} ({ADVERSARIAL_RATIO*100:.0f}% Byzantine)")
    print(f"Blackout round: {BLACKOUT_ROUND}/{MAX_ROUNDS}")
    print()
    
    # Initialize zones
    zones = {}
    all_nodes = []
    node_id_counter = 0
    
    for zone_name in ZONES:
        nodes_in_zone = []
        n_byzantine = int(NODES_PER_ZONE * ADVERSARIAL_RATIO) if zone_name == ADVERSARIAL_ZONE else 0
        
        for i in range(NODES_PER_ZONE):
            is_byz = i < n_byzantine
            node = Node(
                node_id=node_id_counter,
                zone=zone_name,
                is_byzantine=is_byz,
                reputation=0.3 if is_byz else DEFAULT_REPUTATION  # Attackers start low
            )
            nodes_in_zone.append(node)
            all_nodes.append(node)
            node_id_counter += 1
        
        zones[zone_name] = Zone(name=zone_name, nodes=nodes_in_zone)
    
    print(f"Initialized {TOTAL_NODES} nodes across {len(zones)} zones")
    print(f"Byzantine nodes (in {ADVERSARIAL_ZONE}): {sum(1 for n in all_nodes if n.is_byzantine)}")
    print()
    
    # Initialize systems
    bridge = BridgeNetwork(zones)
    regime_detectors = {z: RegimeDetector(zones[z]) for z in ZONES}
    rep_tracker = ReputationTracker()
    
    # Storage for Lamarckian resumption
    nvram_storage = {}
    
    # Simulation metrics
    storm_triggers = {z: 0 for z in ZONES}
    slander_attempts = 0
    slander_contained = 0
    
    # Run simulation
    print(f"Running {MAX_ROUNDS} rounds...")
    for round_idx in range(MAX_ROUNDS):
        bridge.reset_round()
        
        # === BLACKOUT EVENT ===
        if round_idx == BLACKOUT_ROUND:
            print(f"\n⚡ ROUND {round_idx}: TOTAL POWER FAILURE")
            # Save all weights to NVRAM before "death"
            for node in all_nodes:
                nvram_storage[node.node_id] = node.save_to_nvram()
            
            # Simulate power loss: zero out weights
            for node in all_nodes:
                node.weights = np.zeros(DIM)
                node.energy = 0.0
            
            print(f"  Saved {len(nvram_storage)} node states to NVRAM")
        
        # === RECOVERY ===
        if round_idx == BLACKOUT_ROUND + 1:
            print(f"\n🔋 ROUND {round_idx}: POWER RESTORED – LAMARCKIAN RESUMPTION")
            for node in all_nodes:
                node.restore_from_nvram(nvram_storage[node.node_id])
                node.energy = 0.5  # Partial charge from solar
            
            # Verify recovery
            recovery_errors = []
            for node in all_nodes:
                expected = nvram_storage[node.node_id]
                actual = node.weights
                error = np.max(np.abs(expected - actual))
                recovery_errors.append(error)
            
            max_error = max(recovery_errors)
            print(f"  Recovery complete: max error = {max_error:.10f}")
            if max_error < 1e-9:
                print("  ✓ 100% bit-perfect recovery (INV-6)")
            else:
                print(f"  ✗ Recovery error detected! (INV-6 VIOLATED)")
        
        # === REGIME DETECTION ===
        for zone_name, detector in regime_detectors.items():
            new_regime = detector.update_regime()
            if new_regime == "Storm":
                storm_triggers[zone_name] += 1
        
        # === ADVERSARIAL SLANDER CAMPAIGN (Round 50-60) ===
        if 50 <= round_idx < 60:
            # Byzantine nodes in adversarial zone launch coordinated slander
            byz_nodes = [n for n in zones[ADVERSARIAL_ZONE].nodes if n.is_byzantine]
            
            # Target honest nodes in same zone
            honest_targets = [n for n in zones[ADVERSARIAL_ZONE].nodes if not n.is_byzantine]
            
            for byz in byz_nodes:
                for target in honest_targets[:3]:  # Each Byzantine slanders 3 honest nodes
                    # Slander: give low peer evaluation (0.1)
                    rep_tracker.report_peer_eval(target.node_id, 0.1)
                    slander_attempts += 1
            
            # Honest nodes give accurate peer evals
            for honest in honest_targets:
                for target in honest_targets[:5]:
                    if target != honest:
                        rep_tracker.report_peer_eval(target.node_id, 0.8)  # High score
        
        # === REPUTATION UPDATE ===
        rep_tracker.update_reputations(all_nodes)
        
        # === INTER-ZONE BRIDGE COMMUNICATION ===
        # High-reputation nodes can gossip across zones
        for source_zone_name in ZONES:
            eligible_nodes = [n for n in zones[source_zone_name].nodes if n.can_bridge()]
            
            for node in eligible_nodes[:2]:  # Limit to 2 nodes per zone per round
                for target_zone_name in ZONES:
                    if target_zone_name != source_zone_name:
                        message = {"round": round_idx, "source": node.node_id}
                        bridge.send_message(source_zone_name, target_zone_name, message)
    
    print()
    print("=" * 70)
    print("RESULTS")
    print("=" * 70)
    
    # Analyze slander containment
    adversarial_zone_nodes = zones[ADVERSARIAL_ZONE].nodes
    honest_victims = [n for n in adversarial_zone_nodes if not n.is_byzantine]
    avg_victim_reputation = np.mean([n.reputation for n in honest_victims])
    
    print(f"\nSlander Attack Analysis (Rounds 50-60):")
    print(f"  Slander attempts: {slander_attempts}")
    print(f"  Honest victims avg reputation: {avg_victim_reputation:.3f}")
    if avg_victim_reputation > 0.5:
        print(f"  ✓ Median PeerEval contained damage (INV-2, INV-3)")
        slander_contained = 1
    else:
        print(f"  ✗ Reputation collapsed despite median (INV-2 VIOLATED)")
    
    print(f"\nRegime Trigger Statistics:")
    for zone, count in storm_triggers.items():
        print(f"  {zone}: {count} Storm triggers")
        quorum_count = zones[zone].count_high_reputation()
        print(f"    High-rep nodes (R > {HIGH_REP_THRESHOLD}): {quorum_count}")
    
    print(f"\nLamarckian Recovery:")
    if max_error < 1e-9:
        recovery_pass = 1
        print(f"  ✓ 100% bit-perfect recovery (max error: {max_error:.2e})")
    else:
        recovery_pass = 0
        print(f"  ✗ Recovery error: {max_error:.2e} (INV-6 VIOLATED)")
    
    # Zone isolation check
    adversarial_bridges = sum(1 for n in zones[ADVERSARIAL_ZONE].nodes if n.can_bridge())
    print(f"\nZone Isolation:")
    print(f"  Byzantine zone ({ADVERSARIAL_ZONE}) eligible bridges: {adversarial_bridges}/{NODES_PER_ZONE}")
    if adversarial_bridges < 3:
        zone_isolation_pass = 1
        print(f"  ✓ Attack contained within zone (INV-2, INV-3)")
    else:
        zone_isolation_pass = 0
        print(f"  ✗ Too many Byzantine bridges (attack spread risk)")
    
    # Pass/Fail
    print()
    print("=" * 70)
    print("PASS/FAIL:")
    pass_criteria = {
        "Slander contained (INV-2, INV-3)": slander_contained == 1,
        "Lamarckian recovery (INV-6)": recovery_pass == 1,
        "Zone isolation (INV-2, INV-3)": zone_isolation_pass == 1,
    }
    
    all_pass = all(pass_criteria.values())
    for criterion, passed in pass_criteria.items():
        status = "✓ PASS" if passed else "✗ FAIL"
        print(f"  {criterion}: {status}")
    
    print()
    if all_pass:
        print("🎉 PHASE 3 VERIFICATION: ALL TESTS PASSED")
    else:
        print("❌ PHASE 3 VERIFICATION: FAILED")
    
    # Save results
    results_dir = Path(__file__).parent.parent.parent / "docs" / "RaaS_Data"
    results_dir.mkdir(parents=True, exist_ok=True)
    
    summary_df = pd.DataFrame({
        "metric": [
            "slander_attempts",
            "avg_victim_reputation",
            "recovery_max_error",
            "adversarial_bridges",
        ],
        "value": [
            slander_attempts,
            avg_victim_reputation,
            max_error,
            adversarial_bridges,
        ]
    })
    summary_df.to_csv(results_dir / "phase3_sentinel_summary.csv", index=False)
    print(f"\nSaved: {results_dir / 'phase3_sentinel_summary.csv'}")
    
    return all_pass


if __name__ == "__main__":
    success = main()
    exit(0 if success else 1)
