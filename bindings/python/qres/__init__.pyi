"""
Type stubs for qres_rust Rust extension module.

This module provides Python type hints for the Rust-implemented QRES API.
Generated for qres v21.0.0.
"""

from typing import List, Dict, Any, Optional, Tuple
import numpy as np
import numpy.typing as npt

class QRES_API:
    """
    Main API for QRES compression and decompression.
    
    The QRES_API provides deterministic compression using Q16.16 fixed-point
    quantization with adaptive regime detection.
    """
    
    def __init__(
        self,
        mode: str = "hybrid",
        seed: Optional[int] = None
    ) -> None:
        """
        Initialize QRES API.
        
        Args:
            mode: Operating mode - "hybrid", "fixed", or "multimodal"
            seed: Random seed for deterministic behavior (optional)
        """
        ...
    
    def compress(
        self,
        data: npt.ArrayLike,
        usage_hint: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        Compress data using Q16.16 fixed-point quantization.
        
        Args:
            data: Input data (list, numpy array, or scalar)
            usage_hint: Compression hint - "Integer", "Signal", "Sparse", or None
        
        Returns:
            Dictionary with keys:
                - 'compressed': np.ndarray of compressed bytes
                - 'ratio': float compression ratio
                - 'original_size': int original size in bytes
                - 'compressed_size': int compressed size in bytes
        """
        ...
    
    def decompress(
        self,
        compressed: npt.ArrayLike
    ) -> npt.NDArray[np.float32]:
        """
        Decompress QRES-compressed data.
        
        Args:
            compressed: Compressed byte array from compress()
        
        Returns:
            Decompressed numpy array (float32)
        """
        ...
    
    def estimate_entropy(
        self,
        data: npt.ArrayLike
    ) -> float:
        """
        Estimate Shannon entropy of data.
        
        Args:
            data: Input data
        
        Returns:
            Entropy value (bits per symbol)
        """
        ...
    
    def set_predictor(
        self,
        predictor: Any
    ) -> None:
        """
        Set custom predictor for residual encoding.
        
        Args:
            predictor: Custom predictor implementing BasePredictor interface
        """
        ...

class TAAFPredictor:
    """
    Temporal Adaptive Attention Fusion (TAAF) predictor.
    
    Fuses multiple modalities using online variance-based attention weighting.
    """
    
    def __init__(
        self,
        num_modalities: int
    ) -> None:
        """
        Initialize TAAF predictor.
        
        Args:
            num_modalities: Number of sensor modalities to fuse
        """
        ...
    
    def predict(
        self,
        values: List[float]
    ) -> Dict[str, Any]:
        """
        Predict fused value from multiple modalities.
        
        Args:
            values: List of sensor readings (length = num_modalities)
        
        Returns:
            Dictionary with keys:
                - 'prediction': float fused prediction
                - 'attention': List[float] attention weights
                - 'variances': List[float] per-modality variances
        """
        ...
    
    def set_weights(
        self,
        weights: List[float]
    ) -> None:
        """
        Set manual attention weights (overrides learned weights).
        
        Args:
            weights: List of weights (must sum to 1.0)
        """
        ...
    
    def reset(self) -> None:
        """Reset predictor state (clears history)."""
        ...

class SwarmNode:
    """
    P2P swarm node for decentralized model aggregation.
    
    Uses libp2p gossipsub with viral epidemic protocol for peer discovery.
    """
    
    def __init__(
        self,
        listen_addr: str,
        bootstrap: Optional[List[str]] = None
    ) -> None:
        """
        Initialize swarm node.
        
        Args:
            listen_addr: Multiaddr to listen on (e.g., "/ip4/0.0.0.0/tcp/9000")
            bootstrap: List of bootstrap peer multiaddrs (optional)
        """
        ...
    
    def start(self) -> None:
        """Start listening for connections."""
        ...
    
    def stop(self) -> None:
        """Stop node and close connections."""
        ...
    
    def peer_id(self) -> str:
        """Get local peer ID (base58 encoded)."""
        ...
    
    def listen_addrs(self) -> List[str]:
        """Get list of listening multiaddrs."""
        ...
    
    def connected_peers(self) -> List[str]:
        """Get list of connected peer IDs."""
        ...
    
    def broadcast_model(
        self,
        compressed: npt.ArrayLike,
        metadata: Dict[str, Any]
    ) -> None:
        """
        Broadcast compressed model update to swarm.
        
        Args:
            compressed: Compressed model data
            metadata: Update metadata (epoch, loss, sender, etc.)
        """
        ...

class RegimeDetector:
    """
    Adaptive regime detector using entropy thresholds with hysteresis.
    """
    
    def __init__(
        self,
        calm_threshold: float = 0.5,
        storm_threshold: float = 1.5,
        hysteresis: float = 0.2
    ) -> None:
        """
        Initialize regime detector.
        
        Args:
            calm_threshold: Entropy threshold for Calm regime
            storm_threshold: Entropy threshold for Storm regime
            hysteresis: Hysteresis band to prevent flapping
        """
        ...
    
    def update_entropy(
        self,
        entropy: float
    ) -> None:
        """
        Update detector with new entropy measurement.
        
        Args:
            entropy: Current entropy value
        """
        ...
    
    def current_regime(self) -> str:
        """
        Get current regime.
        
        Returns:
            "Calm" or "Storm"
        """
        ...
    
    def time_in_regime(
        self,
        regime: str
    ) -> int:
        """
        Get number of updates spent in regime.
        
        Args:
            regime: Regime name ("Calm" or "Storm")
        
        Returns:
            Number of consecutive updates in regime
        """
        ...

class ModelPersistence:
    """
    Persistent storage manager for compressed models.
    """
    
    def __init__(
        self,
        storage_path: str
    ) -> None:
        """
        Initialize persistence manager.
        
        Args:
            storage_path: Directory path for checkpoint storage
        """
        ...
    
    def save(
        self,
        compressed: npt.ArrayLike,
        metadata: Dict[str, Any]
    ) -> str:
        """
        Save compressed model checkpoint.
        
        Args:
            compressed: Compressed model data
            metadata: Checkpoint metadata
        
        Returns:
            Checkpoint ID
        """
        ...
    
    def load(
        self,
        checkpoint_id: str = "latest"
    ) -> Dict[str, Any]:
        """
        Load compressed model checkpoint.
        
        Args:
            checkpoint_id: Checkpoint identifier or "latest"
        
        Returns:
            Dictionary with 'compressed' and 'metadata' keys
        """
        ...
    
    def save_raw(
        self,
        model: npt.ArrayLike,
        checkpoint_id: str
    ) -> None:
        """
        Save raw (uncompressed) model for testing.
        
        Args:
            model: Raw model data
            checkpoint_id: Checkpoint identifier
        """
        ...
    
    def load_raw(
        self,
        checkpoint_id: str
    ) -> npt.NDArray[np.float32]:
        """
        Load raw model checkpoint.
        
        Args:
            checkpoint_id: Checkpoint identifier
        
        Returns:
            Raw model data
        """
        ...

class AdaptiveAggregator:
    """
    Byzantine-tolerant aggregator with adaptive trimming.
    """
    
    def __init__(
        self,
        regime: str = "calm"
    ) -> None:
        """
        Initialize adaptive aggregator.
        
        Args:
            regime: Initial regime ("calm" or "storm")
        """
        ...
    
    def aggregate(
        self,
        updates: npt.ArrayLike
    ) -> Dict[str, Any]:
        """
        Aggregate updates with Byzantine filtering.
        
        Args:
            updates: Array of shape (num_nodes, model_size)
        
        Returns:
            Dictionary with keys:
                - 'aggregated': Aggregated model
                - 'num_filtered': Number of filtered updates
        """
        ...

class CartelDetector:
    """
    Cartel detection using Grubbs' test for outliers.
    """
    
    def __init__(
        self,
        threshold: float = 0.05
    ) -> None:
        """
        Initialize cartel detector.
        
        Args:
            threshold: p-value threshold for outlier detection
        """
        ...
    
    def detect_cartel(
        self,
        updates: npt.ArrayLike
    ) -> npt.NDArray[np.bool_]:
        """
        Detect cartel members in update set.
        
        Args:
            updates: Array of shape (num_nodes, model_size)
        
        Returns:
            Boolean array (True = cartel member)
        """
        ...

class TWTScheduler:
    """
    Target Wake Time (TWT) scheduler for energy management.
    """
    
    def __init__(self) -> None:
        """Initialize TWT scheduler."""
        ...
    
    def set_interval(
        self,
        regime: str,
        wake_ms: int,
        sleep_ms: int
    ) -> None:
        """
        Configure TWT interval for regime.
        
        Args:
            regime: Regime name ("Calm" or "Storm")
            wake_ms: Wake duration in milliseconds
            sleep_ms: Sleep duration in milliseconds
        """
        ...
    
    def get_interval(
        self,
        regime: str
    ) -> Dict[str, int]:
        """
        Get TWT interval for regime.
        
        Args:
            regime: Regime name
        
        Returns:
            Dictionary with 'wake_ms' and 'sleep_ms' keys
        """
        ...
    
    def should_wake(self) -> bool:
        """
        Check if node should be awake.
        
        Returns:
            True if node should be active
        """
        ...

class EnergyMonitor:
    """
    Energy consumption monitor for profiling.
    """
    
    def __init__(self) -> None:
        """Initialize energy monitor."""
        ...
    
    def start(self) -> None:
        """Start monitoring."""
        ...
    
    def record_active(self) -> None:
        """Record active state (high power)."""
        ...
    
    def record_sleep(self) -> None:
        """Record sleep state (low power)."""
        ...
    
    def total_joules(self) -> float:
        """Get total energy consumed in joules."""
        ...
    
    def average_watts(self) -> float:
        """Get average power in watts."""
        ...

class BasePredictor:
    """
    Base class for custom predictors.
    
    Subclass this to implement custom prediction strategies.
    """
    
    def __init__(self) -> None:
        """Initialize predictor."""
        self.history: List[float] = []
    
    def predict(
        self,
        value: float
    ) -> Dict[str, Any]:
        """
        Predict next value (must be implemented by subclass).
        
        Args:
            value: Current value
        
        Returns:
            Dictionary with 'prediction' and 'confidence' keys
        """
        raise NotImplementedError
    
    def reset(self) -> None:
        """Reset predictor state."""
        self.history.clear()

# Version info
__version__: str = "21.0.0"
