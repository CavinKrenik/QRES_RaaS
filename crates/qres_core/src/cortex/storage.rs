//! Persistent Storage Layer
//!
//! Defines traits for persisting model bytecode across sessions.
//! This enables learned strategies to survive reboots via trait-based persistence.

use alloc::vec::Vec;

/// Abstract interface for storing and retrieving model bytecode.
///
/// This trait allows mesh nodes to persist learned bytecode strategies
/// even after the swarm restarts. Implementations can use disk, cloud, or
/// other persistence backends.
///
/// Constraint: Must be `no_std` compatible; implementations may use `std`.
///
/// # Deprecation Notice (v20.2.0)
///
/// This trait is deprecated in favor of [`ModelPersistence`]. The functionality
/// remains unchanged; only the name has been updated as part of the systems
/// engineering terminology migration (biological metaphors → systems terms).
/// See [`docs/TECHNICAL_DEBT.md`](../../../docs/TECHNICAL_DEBT.md) for details.
///
/// **Migration Path:** Use the `ModelPersistence` trait bound instead. All types
/// implementing `GeneStorage` automatically implement `ModelPersistence`.
#[deprecated(
    since = "20.2.0",
    note = "Use the ModelPersistence trait bound instead. See docs/TECHNICAL_DEBT.md"
)]
pub trait GeneStorage {
    /// Save an evolved gene for a specific node.
    ///
    /// # Arguments
    /// * `id` - The unique identifier of the node
    /// * `gene` - The bytecode/genome to persist
    ///
    /// # Returns
    /// `true` if save succeeded, `false` otherwise
    fn save_gene(&mut self, id: u32, gene: &[u8]) -> bool;

    /// Load a previously saved gene for a node.
    ///
    /// # Arguments
    /// * `id` - The unique identifier of the node
    ///
    /// # Returns
    /// `Some(gene)` if a saved gene exists, `None` otherwise
    fn load_gene(&self, id: u32) -> Option<Vec<u8>>;
}

// =============================================================================
// ModelPersistence: Systems Engineering Terminology Bridge (v20.2.0)
// =============================================================================

/// Abstract interface for storing and retrieving model bytecode.
///
/// This is the preferred trait for persistent storage operations as of v20.2.0.
/// It replaces the deprecated [`GeneStorage`] trait as part of the systems
/// engineering terminology migration.
///
/// # Automatic Implementation
///
/// All types implementing [`GeneStorage`] automatically implement `ModelPersistence`.
/// This provides a non-breaking migration path:
///
/// ```ignore
/// // Old code (still works, but emits deprecation warning):
/// fn persist<S: GeneStorage>(storage: &mut S) { ... }
///
/// // New code (preferred):
/// fn persist<S: ModelPersistence>(storage: &mut S) { ... }
/// ```
///
/// # Methods
///
/// This trait inherits all methods from [`GeneStorage`]:
/// - [`save_gene`](GeneStorage::save_gene) - Save model bytecode for a node
/// - [`load_gene`](GeneStorage::load_gene) - Load previously saved model bytecode
///
/// # Timeline
///
/// - **v20.2.0**: `ModelPersistence` introduced as subtrait bridge
/// - **v21.0.0**: `GeneStorage` removed, `ModelPersistence` becomes standalone
#[allow(deprecated)]
pub trait ModelPersistence: GeneStorage {}

/// Blanket implementation: all `GeneStorage` implementors are `ModelPersistence`.
#[allow(deprecated)]
impl<T: GeneStorage> ModelPersistence for T {}
