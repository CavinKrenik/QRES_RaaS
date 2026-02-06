#!/usr/bin/env python3
"""
Example 03: Swarm Node Setup & P2P Participation
================================================

Demonstrates how to initialize a QRES node and participate in the P2P swarm.

Features:
- P2P node initialization with libp2p
- Gossip protocol participation (viral epidemic AD-SGD)
- Model bytecode gossip (not data gossip)
- Reputation tracking
- W3C DID identity generation

Requirements:
    pip install qres-raas

References:
    - P2P Implementation: docs/guides/P2P_IMPLEMENTATION.md
    - Swarm Architecture: docs/reference/ARCHITECTURE.md (Section 2)
    - Viral Protocol: CHANGELOG.md (v20.0.0 epidemic AD-SGD)
"""

import sys
import time
try:
    from qres import QRES_API
    from qres.swarm_cli import SwarmNode, SwarmConfig
except ImportError as e:
    print(f"✗ Error: {e}")
    print("\nInstall dependencies:")
    print("  cd bindings/python && maturin develop --release")
    sys.exit(1)


def main():
    print("=" * 70)
    print("QRES v21.0 - Swarm Node Participation Example")
    print("=" * 70)
    print("\nFeature: Viral Epidemic AD-SGD Gossip Protocol")
    print("Paper: 'RaaS: Resource-Aware Agentic Swarm', Section IV\n")
    
    # Step 1: Initialize swarm configuration
    print("Step 1: Configuring Swarm Node")
    print("-" * 70)
    
    try:
        config = SwarmConfig(
            node_id="demo_node_001",
            listen_address="/ip4/0.0.0.0/tcp/0",  # Auto-assign port
            bootstrap_peers=[],  # Empty for standalone demo
            reputation_initial=0.8,  # Start with moderate reputation
            regime="Calm"  # Initial regime (Calm/PreStorm/Storm)
        )
        print(f"✓ Node ID: {config.node_id}")
        print(f"✓ Listen: {config.listen_address}")
        print(f"✓ Initial Reputation: {config.reputation_initial}")
        print(f"✓ Regime: {config.regime}\n")
    except Exception as e:
        print(f"⚠️ SwarmConfig not available: {e}")
        print("   This example requires full P2P implementation")
        print("   See docs/guides/P2P_IMPLEMENTATION.md for setup")
        return
    
    # Step 2: Launch swarm node
    print("Step 2: Launching Swarm Node")
    print("-" * 70)
    
    try:
        node = SwarmNode(config)
        print(f"✓ Swarm node initialized")
        print(f"  PeerID: {node.peer_id if hasattr(node, 'peer_id') else 'N/A'}")
        print(f"  DID: did:qres:{node.did_suffix if hasattr(node, 'did_suffix') else 'N/A'}")
        print()
    except Exception as e:
        print(f"⚠️ SwarmNode not available: {e}")
        print("   Running in simulation mode...\n")
        node = None
    
    # Step 3: Gossip participation simulation
    print("Step 3: Gossip Protocol Simulation")
    print("-" * 70)
    
    # Simulate model updates
    updates = [
        {"residual_error": 0.05, "accuracy_delta": 0.02, "epoch": 1},
        {"residual_error": 0.03, "accuracy_delta": 0.01, "epoch": 2},
        {"residual_error": 0.02, "accuracy_delta": 0.015, "epoch": 3},
    ]
    
    for i, update in enumerate(updates, 1):
        print(f"Gossip Round {i}:")
        print(f"  Residual Error: {update['residual_error']:.4f}")
        print(f"  Accuracy Delta: {update['accuracy_delta']:.4f}")
        
        # Calculate epidemic priority
        reputation = config.reputation_initial
        priority = (update['residual_error'] * 
                   update['accuracy_delta'] * 
                   reputation)
        
        print(f"  Epidemic Priority: {priority:.6f}")
        print(f"    (= {update['residual_error']} × {update['accuracy_delta']} × {reputation})")
        
        # Infection criteria (from v20.0 viral protocol)
        cure_threshold = 0.01
        can_infect = update['accuracy_delta'] > cure_threshold
        
        if can_infect:
            print(f"  ✓ Can infect peers (Δaccuracy > {cure_threshold})")
        else:
            print(f"  ✗ Cannot infect (Δaccuracy ≤ {cure_threshold})")
        
        print()
        
        # Simulate network delay
        time.sleep(0.5)
    
    # Step 4: Reputation tracking
    print("Step 4: Reputation System")
    print("-" * 70)
    
    # Simulate reputation updates based on contribution quality
    contributions = [
        {"quality": 0.9, "result": "good"},
        {"quality": 0.85, "result": "good"},
        {"quality": 0.4, "result": "suspicious"},
        {"quality": 0.92, "result": "excellent"},
    ]
    
    current_rep = config.reputation_initial
    print(f"Initial Reputation: {current_rep:.3f}\n")
    
    for i, contrib in enumerate(contributions, 1):
        # Simple reputation update (actual algorithm is more complex)
        if contrib['quality'] > 0.7:
            current_rep = min(1.0, current_rep + 0.02)
        else:
            current_rep = max(0.0, current_rep - 0.1)
        
        print(f"Contribution {i}: quality={contrib['quality']:.2f} → {contrib['result']}")
        print(f"  Updated Reputation: {current_rep:.3f}")
        
        # Calculate adaptive exponent (v20.0 Rule 4)
        swarm_size = 100  # Assume 100-node swarm
        if swarm_size < 20:
            rep_exponent = 2.0
        elif swarm_size < 50:
            rep_exponent = 3.0
        else:
            rep_exponent = 3.5
        
        # Influence cap: rep^exponent × 0.8
        influence = min(current_rep ** rep_exponent * 0.8, 1.0)
        print(f"  Influence: {influence:.4f} (rep^{rep_exponent} × 0.8)\n")
    
    # Summary
    print("-" * 70)
    print("Summary:")
    print(f"  Final Reputation: {current_rep:.3f}")
    print(f"  Swarm Size:       {swarm_size} nodes")
    print(f"  Rep Exponent:     {rep_exponent} (adaptive scaling)")
    print(f"  Final Influence:  {influence:.4f}")
    
    # Cleanup
    if node:
        try:
            node.shutdown()
            print("\n✓ Swarm node shut down gracefully")
        except:
            pass
    
    print("\n" + "=" * 70)
    print("Next Steps:")
    print("  - See 04_byzantine_defense.py for cartel detection")
    print("  - See examples/virtual_iot_network/ for full 100-node demo")
    print("  - Read docs/guides/P2P_IMPLEMENTATION.md for libp2p details")
    print("=" * 70)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n\n✗ Interrupted by user")
        sys.exit(130)
    except Exception as e:
        print(f"\n✗ Unexpected error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)
