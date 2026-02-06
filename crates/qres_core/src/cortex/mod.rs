/// Aggregation Engine: Active model processing for the mesh
///
/// This module implements the active model aggregator that:
/// 1. Predict the next value (hot path)
/// 2. Detect surprise (anomaly detection via refractory logic)
/// 3. Adapt based on peer signals (collective learning)
/// 4. Export/import model bytecode (behavior propagation)
///
/// Design constraints:
/// - No std: All types use core::* and alloc
/// - No String: All communication is Copy or Vec<u8>
/// - Fixed-point math: I16F16 for determinism across platforms
///
/// # Terminology Migration (v20.2.0)
///
/// | Old Term (Deprecated) | New Term (Preferred) |
/// |-----------------------|----------------------|
/// | Cortex | Aggregation Engine |
/// | GeneStorage | ModelPersistence |
/// | gene | model bytecode |
pub mod linear;
pub mod neuron;
pub mod storage;

pub use linear::LinearNeuron;
pub use neuron::{Regime, SpikeEvent, SwarmNeuron};
#[allow(deprecated)]
pub use storage::GeneStorage;
pub use storage::ModelPersistence;
