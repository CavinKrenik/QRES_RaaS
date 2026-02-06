//! Semantic Middleware: HSTP-Aligned Gene Envelope
//!
//! Wraps raw gene payloads (48–74 bytes) in a semantic envelope compatible with
//! IEEE 7007-2021 (HSTP) and W3C standards. This module provides:
//!
//! - **W3C DID**: Decentralized Identifiers derived from Ed25519 peer keys
//! - **JSON-LD Context**: Semantic vocabulary for gene metadata
//! - **RDF Triples**: Subject–predicate–object provenance for gene lineage
//! - **SemanticEnvelope**: Wire-format wrapper that pairs raw genes with metadata
//!
//! # Design Rationale
//!
//! QRES genes are opaque byte vectors (LinearNeuron: 48B, Summary: ~74B).
//! External systems (HSTP brokers, IEEE 7007 registries, federated swarm bridges)
//! need machine-readable semantics to discover, filter, and compose gene updates
//! without parsing the binary payload. The envelope adds ~200–400 bytes of JSON
//! metadata alongside the raw gene, well within the 1012-byte fragment payload.
//!
//! # Feature Gate
//!
//! This module requires `std` (JSON serialization via `serde_json`).

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// W3C Decentralized Identifier (DID)
// ---------------------------------------------------------------------------

/// A W3C DID derived from an Ed25519 public key.
///
/// Format: `did:qres:<hex-encoded-ed25519-pubkey>`
///
/// This follows the DID Core specification (W3C Recommendation 2022-07-19).
/// The `qres` method namespace identifies QRES swarm nodes. Resolution is
/// local: any node holding the corresponding Ed25519 keypair can prove
/// ownership via a standard Schnorr signature.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeDid {
    /// Full DID string, e.g. `did:qres:a1b2c3...`
    pub id: String,
    /// Raw Ed25519 public key bytes (32 bytes)
    #[serde(skip)]
    pub pubkey: [u8; 32],
}

impl NodeDid {
    /// Create a DID from a 32-byte Ed25519 public key.
    pub fn from_pubkey(pubkey: &[u8; 32]) -> Self {
        let hex = bytes_to_hex(pubkey);
        Self {
            id: alloc::format!("did:qres:{}", hex),
            pubkey: *pubkey,
        }
    }

    /// Parse a DID string back to pubkey bytes. Returns `None` if malformed.
    pub fn parse(did: &str) -> Option<Self> {
        let hex_part = did.strip_prefix("did:qres:")?;
        if hex_part.len() != 64 {
            return None;
        }
        let pubkey = hex_to_bytes32(hex_part)?;
        Some(Self {
            id: String::from(did),
            pubkey,
        })
    }
}

// ---------------------------------------------------------------------------
// JSON-LD Context for HSTP / IEEE 7007-2021 alignment
// ---------------------------------------------------------------------------

/// JSON-LD `@context` entries for QRES gene metadata.
///
/// Maps short property names to IRIs from established vocabularies:
/// - `schema.org` for general metadata (name, description, dateCreated)
/// - `w3id.org/security` for cryptographic provenance
/// - `ieee.org/7007` for autonomous-agent interoperability
/// - `qres.io` for QRES-specific terms (gene, regime, modality)
pub const JSONLD_CONTEXT: &str = r#"{
  "@context": {
    "@vocab": "https://qres.io/vocab/",
    "schema": "https://schema.org/",
    "sec": "https://w3id.org/security#",
    "ieee7007": "https://standards.ieee.org/ieee/7007/",
    "did": "https://www.w3.org/ns/did/v1#",
    "gene": "https://qres.io/vocab/gene",
    "regime": "https://qres.io/vocab/regime",
    "modality": "https://qres.io/vocab/modality",
    "fitness": "https://qres.io/vocab/fitness",
    "epoch": "https://qres.io/vocab/epoch",
    "swarmSize": "https://qres.io/vocab/swarmSize",
    "reputation": "https://qres.io/vocab/reputation",
    "creator": "did:id",
    "created": "schema:dateCreated",
    "proofMethod": "sec:proof"
  }
}"#;

// ---------------------------------------------------------------------------
// RDF Triple
// ---------------------------------------------------------------------------

/// A minimal RDF triple for gene provenance.
///
/// Triples describe relationships in subject–predicate–object form:
/// ```text
/// <did:qres:abc123>  <qres:exported>  <gene:linear:48B>
/// <gene:linear:48B>  <qres:fitness>   "0.0351"
/// <gene:linear:48B>  <qres:regime>    "Storm"
/// ```
///
/// These are embedded in the `SemanticEnvelope` and can be extracted by
/// HSTP brokers or SPARQL endpoints for swarm-wide gene discovery.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RdfTriple {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

impl RdfTriple {
    pub fn new(subject: &str, predicate: &str, object: &str) -> Self {
        Self {
            subject: String::from(subject),
            predicate: String::from(predicate),
            object: String::from(object),
        }
    }
}

// ---------------------------------------------------------------------------
// Gene Metadata
// ---------------------------------------------------------------------------

/// Regime at time of gene export.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Regime {
    Calm,
    PreStorm,
    Storm,
}

impl Regime {
    pub fn as_str(&self) -> &'static str {
        match self {
            Regime::Calm => "Calm",
            Regime::PreStorm => "PreStorm",
            Regime::Storm => "Storm",
        }
    }
}

/// Sensor modality associated with a gene.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Modality {
    Temperature,
    AirQuality,
    Vibration,
    Power,
    Custom(String),
}

impl Modality {
    pub fn as_str(&self) -> &str {
        match self {
            Modality::Temperature => "temperature",
            Modality::AirQuality => "air-quality",
            Modality::Vibration => "vibration",
            Modality::Power => "power",
            Modality::Custom(s) => s.as_str(),
        }
    }
}

/// Structured metadata for a gene payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneMetadata {
    /// Sensor modality this gene is trained on
    pub modality: Modality,
    /// Regime at time of gene export
    pub regime: Regime,
    /// Training epoch count
    pub epoch: u32,
    /// Fitness score (RMSE or equivalent, lower is better)
    pub fitness: f32,
    /// Reputation of the exporting node at export time
    pub reputation: f32,
    /// Swarm size at export time (for adaptive exponent context)
    pub swarm_size: u16,
    /// Gene format identifier (e.g. "linear-48", "summary-74")
    pub gene_format: String,
}

// ---------------------------------------------------------------------------
// Semantic Envelope
// ---------------------------------------------------------------------------

/// HSTP-aligned semantic envelope wrapping a raw gene payload.
///
/// This is the wire format for semantically-enriched gene gossip. It pairs
/// the opaque gene bytes with JSON-LD–compatible metadata and RDF provenance
/// triples, enabling HSTP brokers and IEEE 7007 registries to index, filter,
/// and route gene updates without parsing the binary payload.
///
/// # Size Budget
///
/// | Component        | Typical Size |
/// |------------------|-------------|
/// | Raw gene         | 48–74 B     |
/// | Metadata JSON    | 150–250 B   |
/// | RDF triples      | 100–200 B   |
/// | DID + overhead   | 80 B        |
/// | **Total**        | **~400–600 B** |
///
/// This fits within a single 1012-byte fragment payload, preserving the
/// existing MTU fragmentation strategy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SemanticEnvelope {
    /// JSON-LD @context reference (compact IRI)
    #[serde(rename = "@context")]
    pub context: String,
    /// JSON-LD @type
    #[serde(rename = "@type")]
    pub type_: String,
    /// W3C DID of the exporting node
    pub creator: NodeDid,
    /// Gene metadata (modality, regime, fitness, etc.)
    pub metadata: GeneMetadata,
    /// RDF provenance triples
    pub provenance: Vec<RdfTriple>,
    /// Raw gene payload (base64-like hex encoding for JSON safety)
    pub gene_payload_hex: String,
    /// Byte length of the raw gene payload
    pub gene_payload_len: usize,
}

impl SemanticEnvelope {
    /// Wrap a raw gene payload in a semantic envelope.
    ///
    /// # Arguments
    /// * `peer_id` - 32-byte Ed25519 public key of the exporting node
    /// * `gene_bytes` - Raw gene payload (48 or 74 bytes typically)
    /// * `metadata` - Structured gene metadata
    pub fn wrap(peer_id: &[u8; 32], gene_bytes: &[u8], metadata: GeneMetadata) -> Self {
        let creator = NodeDid::from_pubkey(peer_id);
        let gene_hex = bytes_to_hex(gene_bytes);
        let gene_uri = alloc::format!("gene:{}:{}B", metadata.gene_format, gene_bytes.len());

        // Build provenance triples
        let mut provenance = Vec::with_capacity(4);
        provenance.push(RdfTriple::new(&creator.id, "qres:exported", &gene_uri));
        provenance.push(RdfTriple::new(
            &gene_uri,
            "qres:fitness",
            &alloc::format!("{:.4}", metadata.fitness),
        ));
        provenance.push(RdfTriple::new(
            &gene_uri,
            "qres:regime",
            metadata.regime.as_str(),
        ));
        provenance.push(RdfTriple::new(
            &gene_uri,
            "qres:modality",
            metadata.modality.as_str(),
        ));

        Self {
            context: String::from("https://qres.io/vocab/v20"),
            type_: String::from("GeneUpdate"),
            creator,
            metadata,
            provenance,
            gene_payload_hex: gene_hex,
            gene_payload_len: gene_bytes.len(),
        }
    }

    /// Extract the raw gene bytes from the envelope.
    pub fn unwrap_gene(&self) -> Option<Vec<u8>> {
        hex_to_bytes(&self.gene_payload_hex)
    }

    /// Serialize the envelope to JSON (JSON-LD compatible).
    #[cfg(feature = "std")]
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serialize the envelope to pretty-printed JSON.
    #[cfg(feature = "std")]
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize an envelope from JSON.
    #[cfg(feature = "std")]
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Total estimated wire size in bytes.
    pub fn estimated_wire_bytes(&self) -> usize {
        // Rough estimate: JSON overhead + payload hex (2x raw size)
        // Actual size depends on serde_json output
        200 + self.gene_payload_hex.len() + self.provenance.len() * 60
    }

    /// Check if the envelope fits within a single fragment payload (1012 bytes).
    pub fn fits_single_fragment(&self) -> bool {
        self.estimated_wire_bytes() <= 1012
    }

    /// Validate envelope integrity.
    pub fn validate(&self) -> Result<(), &'static str> {
        // DID must be well-formed
        if !self.creator.id.starts_with("did:qres:") {
            return Err("invalid DID format");
        }
        // Gene payload length must match hex
        if self.gene_payload_hex.len() != self.gene_payload_len * 2 {
            return Err("gene payload length mismatch");
        }
        // Fitness must be non-negative
        if self.metadata.fitness < 0.0 {
            return Err("negative fitness");
        }
        // Must have at least the export provenance triple
        if self.provenance.is_empty() {
            return Err("missing provenance");
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Semantic-aware GhostUpdate extension
// ---------------------------------------------------------------------------

/// A GhostUpdate with optional HSTP semantic metadata.
///
/// This extends the base `GhostUpdate` with a semantic envelope for
/// interoperability with IEEE 7007-2021 systems. The envelope is optional:
/// intra-swarm gossip can omit it to save bandwidth, while cross-swarm
/// or HSTP-bridged communication includes it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SemanticGhostUpdate {
    /// The base GhostUpdate (weights, ZK proof, privacy budget)
    pub update: crate::packet::GhostUpdate,
    /// Optional semantic envelope (included for HSTP bridging)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub envelope: Option<SemanticEnvelope>,
}

impl SemanticGhostUpdate {
    /// Create a semantic update from a base GhostUpdate.
    pub fn new(update: crate::packet::GhostUpdate) -> Self {
        Self {
            update,
            envelope: None,
        }
    }

    /// Attach a semantic envelope to this update.
    pub fn with_envelope(mut self, envelope: SemanticEnvelope) -> Self {
        self.envelope = Some(envelope);
        self
    }

    /// Check if this update carries semantic metadata.
    pub fn has_semantics(&self) -> bool {
        self.envelope.is_some()
    }

    /// Strip the semantic envelope (for intra-swarm bandwidth savings).
    pub fn strip_semantics(&mut self) {
        self.envelope = None;
    }
}

// ---------------------------------------------------------------------------
// HSTP Discovery Descriptor
// ---------------------------------------------------------------------------

/// An HSTP service descriptor for gene discovery.
///
/// This follows IEEE 7007-2021 Section 6 service description format,
/// enabling HSTP brokers to advertise available gene types from a swarm node.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HstpDescriptor {
    /// JSON-LD @context
    #[serde(rename = "@context")]
    pub context: String,
    /// JSON-LD @type
    #[serde(rename = "@type")]
    pub type_: String,
    /// Node DID
    pub provider: NodeDid,
    /// Available gene formats
    pub gene_formats: Vec<String>,
    /// Supported modalities
    pub modalities: Vec<String>,
    /// Current regime
    pub regime: Regime,
    /// Node reputation (for trust filtering)
    pub reputation: f32,
    /// Swarm size
    pub swarm_size: u16,
}

impl HstpDescriptor {
    /// Create a discovery descriptor for this node.
    pub fn new(
        peer_id: &[u8; 32],
        gene_formats: Vec<String>,
        modalities: Vec<String>,
        regime: Regime,
        reputation: f32,
        swarm_size: u16,
    ) -> Self {
        Self {
            context: String::from("https://qres.io/vocab/v20"),
            type_: String::from("GeneService"),
            provider: NodeDid::from_pubkey(peer_id),
            gene_formats,
            modalities,
            regime,
            reputation,
            swarm_size,
        }
    }

    /// Serialize to JSON for HSTP broker registration.
    #[cfg(feature = "std")]
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

// ---------------------------------------------------------------------------
// Hex encoding utilities (no_std compatible, no extra dependencies)
// ---------------------------------------------------------------------------

const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        hex.push(HEX_CHARS[(b >> 4) as usize] as char);
        hex.push(HEX_CHARS[(b & 0x0f) as usize] as char);
    }
    hex
}

fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    let hex_bytes = hex.as_bytes();
    for i in (0..hex_bytes.len()).step_by(2) {
        let hi = hex_nibble(hex_bytes[i])?;
        let lo = hex_nibble(hex_bytes[i + 1])?;
        bytes.push((hi << 4) | lo);
    }
    Some(bytes)
}

fn hex_to_bytes32(hex: &str) -> Option<[u8; 32]> {
    let bytes = hex_to_bytes(hex)?;
    if bytes.len() != 32 {
        return None;
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Some(arr)
}

fn hex_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_peer_id() -> [u8; 32] {
        let mut id = [0u8; 32];
        for (i, b) in id.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7).wrapping_add(0x42);
        }
        id
    }

    fn sample_gene() -> Vec<u8> {
        // Simulate a 48-byte LinearNeuron gene
        (0..48).map(|i| (i as u8).wrapping_mul(3)).collect()
    }

    fn sample_metadata() -> GeneMetadata {
        GeneMetadata {
            modality: Modality::Temperature,
            regime: Regime::Calm,
            epoch: 150,
            fitness: 0.0351,
            reputation: 0.92,
            swarm_size: 25,
            gene_format: String::from("linear-48"),
        }
    }

    #[test]
    fn test_did_roundtrip() {
        let peer_id = sample_peer_id();
        let did = NodeDid::from_pubkey(&peer_id);
        assert!(did.id.starts_with("did:qres:"));
        assert_eq!(did.id.len(), 9 + 64); // "did:qres:" + 64 hex chars

        let parsed = NodeDid::parse(&did.id).unwrap();
        assert_eq!(parsed.pubkey, peer_id);
        assert_eq!(parsed.id, did.id);
    }

    #[test]
    fn test_did_parse_invalid() {
        assert!(NodeDid::parse("did:eth:abc123").is_none());
        assert!(NodeDid::parse("did:qres:short").is_none());
        assert!(NodeDid::parse("").is_none());
    }

    #[test]
    fn test_envelope_wrap_unwrap() {
        let peer_id = sample_peer_id();
        let gene = sample_gene();
        let meta = sample_metadata();

        let envelope = SemanticEnvelope::wrap(&peer_id, &gene, meta);

        // Validate structure
        assert_eq!(envelope.type_, "GeneUpdate");
        assert_eq!(envelope.gene_payload_len, 48);
        assert_eq!(envelope.provenance.len(), 4);
        assert!(envelope.validate().is_ok());

        // Round-trip gene bytes
        let recovered = envelope.unwrap_gene().unwrap();
        assert_eq!(recovered, gene);
    }

    #[test]
    fn test_envelope_fits_fragment() {
        let peer_id = sample_peer_id();
        let gene = sample_gene();
        let meta = sample_metadata();

        let envelope = SemanticEnvelope::wrap(&peer_id, &gene, meta);
        assert!(envelope.fits_single_fragment());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_envelope_json_roundtrip() {
        let peer_id = sample_peer_id();
        let gene = sample_gene();
        let meta = sample_metadata();

        let envelope = SemanticEnvelope::wrap(&peer_id, &gene, meta);
        let json = envelope.to_json().unwrap();

        // Verify JSON-LD fields present
        assert!(json.contains("@context"));
        assert!(json.contains("@type"));
        assert!(json.contains("GeneUpdate"));
        assert!(json.contains("did:qres:"));

        // Deserialize and verify
        let restored = SemanticEnvelope::from_json(&json).unwrap();
        assert_eq!(restored.gene_payload_len, 48);
        assert_eq!(restored.unwrap_gene().unwrap(), gene);
        assert_eq!(restored.metadata.fitness, 0.0351);
    }

    #[test]
    fn test_envelope_validation_rejects_bad_did() {
        let peer_id = sample_peer_id();
        let gene = sample_gene();
        let meta = sample_metadata();

        let mut envelope = SemanticEnvelope::wrap(&peer_id, &gene, meta);
        envelope.creator.id = String::from("invalid");
        assert!(envelope.validate().is_err());
    }

    #[test]
    fn test_envelope_validation_rejects_length_mismatch() {
        let peer_id = sample_peer_id();
        let gene = sample_gene();
        let meta = sample_metadata();

        let mut envelope = SemanticEnvelope::wrap(&peer_id, &gene, meta);
        envelope.gene_payload_len = 99; // Wrong
        assert!(envelope.validate().is_err());
    }

    #[test]
    fn test_provenance_triples() {
        let peer_id = sample_peer_id();
        let gene = sample_gene();
        let meta = sample_metadata();

        let envelope = SemanticEnvelope::wrap(&peer_id, &gene, meta);

        // Verify provenance triples
        let predicates: Vec<&str> = envelope
            .provenance
            .iter()
            .map(|t| t.predicate.as_str())
            .collect();
        assert!(predicates.contains(&"qres:exported"));
        assert!(predicates.contains(&"qres:fitness"));
        assert!(predicates.contains(&"qres:regime"));
        assert!(predicates.contains(&"qres:modality"));

        // Verify fitness value
        let fitness_triple = envelope
            .provenance
            .iter()
            .find(|t| t.predicate == "qres:fitness")
            .unwrap();
        assert_eq!(fitness_triple.object, "0.0351");
    }

    fn dummy_norm_proof() -> crate::zk_proofs::NormProof {
        use curve25519_dalek::edwards::CompressedEdwardsY;
        use curve25519_dalek::scalar::Scalar;
        crate::zk_proofs::NormProof {
            commitment: CompressedEdwardsY::default(),
            response: Scalar::ZERO,
        }
    }

    #[test]
    fn test_semantic_ghost_update() {
        let update = crate::packet::GhostUpdate {
            peer_id: sample_peer_id(),
            masked_weights: alloc::vec![1, 2, 3],
            zk_proof: dummy_norm_proof(),
            dp_epsilon: 0.1,
            residual_error: 0.01,
            accuracy_delta: 0.08,
        };

        let gene = sample_gene();
        let meta = sample_metadata();
        let envelope = SemanticEnvelope::wrap(&update.peer_id, &gene, meta);

        let semantic = SemanticGhostUpdate::new(update).with_envelope(envelope);

        assert!(semantic.has_semantics());
        assert_eq!(semantic.envelope.as_ref().unwrap().gene_payload_len, 48);
    }

    #[test]
    fn test_semantic_ghost_update_strip() {
        let update = crate::packet::GhostUpdate {
            peer_id: sample_peer_id(),
            masked_weights: alloc::vec![],
            zk_proof: dummy_norm_proof(),
            dp_epsilon: 0.0,
            residual_error: 0.0,
            accuracy_delta: 0.0,
        };

        let gene = sample_gene();
        let meta = sample_metadata();
        let envelope = SemanticEnvelope::wrap(&update.peer_id, &gene, meta);

        let mut semantic = SemanticGhostUpdate::new(update).with_envelope(envelope);

        assert!(semantic.has_semantics());
        semantic.strip_semantics();
        assert!(!semantic.has_semantics());
    }

    #[test]
    fn test_hstp_descriptor() {
        let peer_id = sample_peer_id();
        let desc = HstpDescriptor::new(
            &peer_id,
            alloc::vec![String::from("linear-48"), String::from("summary-74")],
            alloc::vec![String::from("temperature"), String::from("air-quality")],
            Regime::Calm,
            0.92,
            25,
        );

        assert_eq!(desc.type_, "GeneService");
        assert_eq!(desc.gene_formats.len(), 2);
        assert_eq!(desc.modalities.len(), 2);
    }

    #[test]
    fn test_hex_roundtrip() {
        let data = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0xFF];
        let hex = bytes_to_hex(&data);
        assert_eq!(hex, "deadbeef00ff");
        let recovered = hex_to_bytes(&hex).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn test_hex_invalid() {
        assert!(hex_to_bytes("0").is_none()); // Odd length
        assert!(hex_to_bytes("zz").is_none()); // Invalid chars
    }

    #[test]
    fn test_74_byte_summary_gene() {
        // Verify envelope works with 74-byte summary genes too
        let peer_id = sample_peer_id();
        let gene: Vec<u8> = (0..74).map(|i| (i as u8).wrapping_mul(5)).collect();
        let meta = GeneMetadata {
            modality: Modality::AirQuality,
            regime: Regime::Storm,
            epoch: 300,
            fitness: 0.0280,
            reputation: 0.85,
            swarm_size: 60,
            gene_format: String::from("summary-74"),
        };

        let envelope = SemanticEnvelope::wrap(&peer_id, &gene, meta);
        assert!(envelope.validate().is_ok());
        assert!(envelope.fits_single_fragment());
        assert_eq!(envelope.unwrap_gene().unwrap().len(), 74);
    }
}
