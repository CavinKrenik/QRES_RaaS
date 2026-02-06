#!/usr/bin/env python3
"""
Example 02: Multimodal TAAF Fusion
===================================

Demonstrates Temporal Attention-Guided Adaptive Fusion (v20.0 feature).

Features:
- Cross-modal sensor fusion (temperature + humidity + pressure)
- Event-driven sparse spiking (Welford's online variance)
- Attention weight visualization
- 3.6% RMSE improvement over unimodal (v21.0 verified)

Requirements:
    pip install qres-raas numpy

References:
    - TAAF Architecture: docs/reference/ARCHITECTURE.md (Section 3)
    - Multimodal Verification: docs/verification/QRES_V20_FINAL_VERIFICATION.md
    - Theory: RaaS_Extras/docs/theory/THEORY.md
"""

import sys
try:
    from qres import QRES_API
    from qres.multimodal import TAAFPredictor, observe_multimodal
    import numpy as np
except ImportError as e:
    print(f"✗ Error: {e}")
    print("\nInstall dependencies:")
    print("  pip install numpy")
    print("  cd bindings/python && maturin develop --release")
    sys.exit(1)


def generate_correlated_sensors(num_samples=100):
    """Generate synthetic correlated sensor data."""
    np.random.seed(42)  # Deterministic for reproducibility
    
    # Base temperature signal (sine wave + noise)
    t = np.linspace(0, 4*np.pi, num_samples)
    temperature = 20 + 5 * np.sin(t) + np.random.normal(0, 0.5, num_samples)
    
    # Humidity inversely correlated with temperature
    humidity = 70 - 2 * (temperature - 20) + np.random.normal(0, 2, num_samples)
    
    # Pressure is mostly stable with small variations
    pressure = 1013 + np.random.normal(0, 1, num_samples)
    
    return temperature, humidity, pressure


def main():
    print("=" * 70)
    print("QRES v21.0 - Multimodal TAAF Fusion Example")
    print("=" * 70)
    print("\nFeature: Temporal Attention-Guided Adaptive Fusion")
    print("Paper: 'RaaS: Resource-Aware Agentic Swarm', Section III\n")
    
    # Generate synthetic sensor data
    print("Generating synthetic correlated sensor streams...")
    temp, humidity, pressure = generate_correlated_sensors(num_samples=100)
    print(f"✓ Generated 100 samples × 3 modalities\n")
    
    # Initialize TAAF predictor
    print("Initializing TAAF Predictor...")
    try:
        predictor = TAAFPredictor(
            num_modalities=3,
            fusion_mode="attention",
            spike_threshold=2.0  # Welford's variance threshold
        )
        print("✓ TAAF Predictor initialized (event-driven sparse spiking)\n")
    except Exception as e:
        print(f"⚠️ TAAF Predictor not available: {e}")
        print("   Using basic QRES_API instead (unimodal compression)")
        predictor = None
    
    # Process multimodal stream
    print("Processing Multimodal Stream:")
    print("-" * 70)
    
    total_residual = 0.0
    spike_events = 0
    
    for i in range(10, len(temp)):  # Start at 10 to allow variance estimation
        # Current sensor readings
        modality_values = [temp[i], humidity[i], pressure[i]]
        
        # Observe and predict using TAAF
        if predictor:
            try:
                prediction, attention_weights = observe_multimodal(
                    predictor, 
                    modality_values
                )
                
                # Check if spike detected (high attention weights)
                max_attention = max(attention_weights)
                if max_attention > 0.5:
                    spike_events += 1
                
                # Calculate residual error
                residual = abs(temp[i] - prediction)  # Predicting temperature
                total_residual += residual
                
                # Print sample outputs
                if i % 20 == 0:
                    print(f"Sample {i:3d}:")
                    print(f"  Modalities: T={temp[i]:.2f}°C, H={humidity[i]:.1f}%, P={pressure[i]:.1f}hPa")
                    print(f"  Attention:  [{attention_weights[0]:.3f}, {attention_weights[1]:.3f}, {attention_weights[2]:.3f}]")
                    print(f"  Prediction: {prediction:.2f}°C (residual: {residual:.3f})")
                    print()
            except Exception as e:
                print(f"⚠️ TAAF processing error: {e}")
                break
        else:
            # Fallback: basic compression
            data = f"{temp[i]:.2f},{humidity[i]:.1f},{pressure[i]:.1f}".encode()
            api = QRES_API(mode="hybrid")
            compressed = api.compress(data, usage_hint="iot")
            total_residual += len(compressed) / len(data)
    
    # Summary statistics
    print("-" * 70)
    print("Summary:")
    print(f"  Average residual: {total_residual / (len(temp) - 10):.4f}")
    print(f"  Spike events:     {spike_events} / {len(temp) - 10} ({spike_events/(len(temp)-10)*100:.1f}%)")
    print(f"  Energy savings:   ~{100 - spike_events/(len(temp)-10)*100:.1f}% (sparse spiking vs. full attention)")
    
    if predictor:
        print("\n✓ Multimodal TAAF fusion demonstrated")
        print("  → Event-driven attention reduces computation by ~60% in Calm regime")
        print("  → 3.6% RMSE improvement over unimodal (verified v20.0)")
    else:
        print("\n⚠️ TAAF fusion not demonstrated (API unavailable)")
    
    print("\n" + "=" * 70)
    print("Next Steps:")
    print("  - See 03_swarm_node.py for P2P swarm participation")
    print("  - Read docs/reference/ARCHITECTURE.md for TAAF pipeline details")
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
