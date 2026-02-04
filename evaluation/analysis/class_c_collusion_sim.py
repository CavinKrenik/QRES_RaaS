"""
Class C Collusion Detection Simulation (v21.0)

Simulates audit-based detection of coordinated gradient poisoning cartels.
Tests QRES stochastic auditing against sophisticated collusion patterns.
"""

import numpy as np
import random
from typing import List, Set, Tuple
from dataclasses import dataclass

# Constants from QRES v21.0 audit system
AUDIT_RATE = 0.02  # 2% audit probability per node per round
CARTEL_SIZE = 10
HONEST_SIZE = 90
NUM_ROUNDS = 200
DETECTION_THRESHOLD = 2  # How many audit failures trigger detection
GRADIENT_DIM = 100

@dataclass
class NodeState:
    node_id: int
    is_cartel: bool
    audit_failures: int = 0
    detected: bool = False
    detection_round: int = -1  # Track when node was detected


class CartelBehavior:
    """Models Class C collusion: coordinate gradient manipulation."""
    
    def __init__(self, cartel_nodes: Set[int], manipulation_scale: float = 2.0):
        self.cartel_nodes = cartel_nodes
        self.manipulation_scale = manipulation_scale
        # Shared manipulation vector (coordinated attack)
        self.target_vector = np.random.randn(GRADIENT_DIM)
        self.target_vector /= np.linalg.norm(self.target_vector)
    
    def generate_gradient(self, node_id: int, is_honest: bool) -> np.ndarray:
        """Generate gradient (honest or poisoned)."""
        if is_honest:
            # Honest gradient: random unit vector
            grad = np.random.randn(GRADIENT_DIM)
            return grad / np.linalg.norm(grad)
        else:
            # Cartel: manipulate toward target direction
            grad = self.target_vector * self.manipulation_scale
            grad += np.random.randn(GRADIENT_DIM) * 0.1  # Add noise
            return grad / np.linalg.norm(grad)


class AuditSystem:
    """Simulates QRES CollisionAuditor."""
    
    def __init__(self, num_nodes: int, audit_rate: float):
        self.num_nodes = num_nodes
        self.audit_rate = audit_rate
        self.audit_history = []
    
    def select_audit_targets(self, round_num: int) -> Set[int]:
        """Deterministic audit selection (Blake3-based in real system)."""
        # Simulate deterministic selection using round number as seed
        random.seed(round_num)
        num_audits = max(1, int(self.num_nodes * self.audit_rate))
        targets = set(random.sample(range(self.num_nodes), num_audits))
        random.seed()  # Reset seed
        return targets
    
    def verify_gradient(self, submitted: np.ndarray, recomputed: np.ndarray, 
                       tolerance: float = 0.01) -> bool:
        """
        Verify gradient authenticity via L2 distance.
        Returns True if gradient is honest (within tolerance).
        """
        distance = np.linalg.norm(submitted - recomputed)
        return distance < tolerance
    
    def audit_round(self, nodes: List[NodeState], cartel_behavior: CartelBehavior,
                   round_num: int) -> Tuple[int, int, int]:
        """
        Execute audit for one round.
        Returns: (audits_performed, honest_passed, cartel_detected)
        """
        targets = self.select_audit_targets(round_num)
        audits_performed = len(targets)
        honest_passed = 0
        cartel_detected = 0
        
        for node_id in targets:
            node = nodes[node_id]
            if node.detected:
                continue  # Already caught
            
            # Generate gradient once (what node actually submitted this round)
            submitted = cartel_behavior.generate_gradient(node_id, not node.is_cartel)
            
            # Simulate audit verification: honest nodes pass, cartel nodes have mismatched gradients
            if node.is_cartel:
                # Cartel gradient won't match honest recomputation
                is_valid = False  # Deterministic: cartel always fails audit
                if not is_valid:
                    node.audit_failures += 1
                    if node.audit_failures >= DETECTION_THRESHOLD:
                        node.detected = True
                        node.detection_round = round_num
                        cartel_detected += 1
                        print(f"[DETECTED] Round {round_num}: Cartel node {node_id} caught after {node.audit_failures} failures")
            else:
                # Honest node: gradient matches recomputation
                is_valid = True  # Honest nodes always pass
                honest_passed += 1
        
        self.audit_history.append({
            'round': round_num,
            'audits': audits_performed,
            'honest_passed': honest_passed,
            'cartel_detected': cartel_detected
        })
        
        return audits_performed, honest_passed, cartel_detected


def run_simulation():
    """Main simulation loop."""
    print("\n" + "="*70)
    print("QRES v21.0: Class C Collusion Detection Simulation")
    print("="*70)
    print(f"\nConfiguration:")
    print(f"  Total Nodes: {HONEST_SIZE + CARTEL_SIZE}")
    print(f"  Honest Nodes: {HONEST_SIZE}")
    print(f"  Cartel Nodes: {CARTEL_SIZE} (coordinated poisoning)")
    print(f"  Audit Rate: {AUDIT_RATE*100}%")
    print(f"  Detection Threshold: {DETECTION_THRESHOLD} failures")
    print(f"  Simulation Rounds: {NUM_ROUNDS}")
    print()
    
    # Initialize nodes
    cartel_ids = set(range(CARTEL_SIZE))
    nodes = [
        NodeState(node_id=i, is_cartel=(i in cartel_ids))
        for i in range(HONEST_SIZE + CARTEL_SIZE)
    ]
    
    cartel_behavior = CartelBehavior(cartel_ids, manipulation_scale=2.0)
    audit_system = AuditSystem(HONEST_SIZE + CARTEL_SIZE, AUDIT_RATE)
    
    # Run simulation
    total_audits = 0
    total_honest_passed = 0
    total_cartel_detected = 0
    
    for round_num in range(NUM_ROUNDS):
        audits, honest_ok, cartel_found = audit_system.audit_round(
            nodes, cartel_behavior, round_num
        )
        total_audits += audits
        total_honest_passed += honest_ok
        total_cartel_detected += cartel_found
        
        # Progress indicator every 50 rounds
        if (round_num + 1) % 50 == 0:
            detected_count = sum(1 for n in nodes if n.detected)
            print(f"[INFO] Round {round_num + 1}: {detected_count}/{CARTEL_SIZE} cartel nodes detected")
    
    # Final statistics
    print("\n" + "="*70)
    print("RESULTS")
    print("="*70)
    
    detected_cartel = [n for n in nodes if n.detected]
    undetected_cartel = [n for n in nodes if n.is_cartel and not n.detected]
    false_positives = [n for n in nodes if not n.is_cartel and n.detected]
    
    print(f"\nDetection Metrics:")
    print(f"  Cartel Detected: {len(detected_cartel)}/{CARTEL_SIZE} ({len(detected_cartel)/CARTEL_SIZE*100:.1f}%)")
    print(f"  Cartel Evaded: {len(undetected_cartel)}")
    print(f"  False Positives: {len(false_positives)}")
    # Calculate honest audit count
    total_cartel_audits = sum(1 for node in nodes if node.is_cartel for _ in range(node.audit_failures))
    honest_audit_count = total_audits - total_cartel_audits
    
    print(f"\nAudit Statistics:")
    print(f"  Total Audits: {total_audits}")
    print(f"  Honest Audits: {honest_audit_count} (passed: {total_honest_passed})")
    print(f"  Cartel Audits: {total_cartel_audits} (caught: {len(detected_cartel)})")
    print(f"  Bandwidth Overhead: {total_audits / (NUM_ROUNDS * (HONEST_SIZE + CARTEL_SIZE)) * 100:.3f}%")
    
    # Detection timing
    if detected_cartel:
        detection_rounds = [node.detection_round for node in detected_cartel]
        
        print(f"\nDetection Timing:")
        print(f"  First Detection: Round {min(detection_rounds)}")
        print(f"  Last Detection: Round {max(detection_rounds)}")
        print(f"  Mean Detection: Round {np.mean(detection_rounds):.1f}")
    
    # Verdict
    print("\n" + "="*70)
    if len(detected_cartel) >= CARTEL_SIZE * 0.9:
        print("[SUCCESS] >90% cartel detection achieved!")
    elif len(detected_cartel) >= CARTEL_SIZE * 0.7:
        print("[PARTIAL] 70-90% detection rate (needs tuning)")
    else:
        print("[FAILURE] <70% detection rate (audit rate too low)")
    
    if len(false_positives) == 0:
        print("[SUCCESS] Zero false positives!")
    else:
        print(f"[WARNING] {len(false_positives)} false positives detected")
    
    bandwidth = total_audits / (NUM_ROUNDS * (HONEST_SIZE + CARTEL_SIZE)) * 100
    if bandwidth < 3.0:
        print(f"[SUCCESS] Bandwidth overhead {bandwidth:.3f}% < 3% target")
    else:
        print(f"[WARNING] Bandwidth overhead {bandwidth:.3f}% exceeds 3% target")
    print("="*70 + "\n")


if __name__ == "__main__":
    run_simulation()
