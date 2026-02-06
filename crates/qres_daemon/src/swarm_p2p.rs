use crate::brain_aggregator::{BrainAggregator, FederatedAverager};
use crate::config::Config;
use crate::living_brain::{LivingBrain, SignedEpiphany};
use crate::peer_keys::PeerKeyStore;
use crate::security::{ReputationManager, SecurityManager, SignedPayload};
use crate::stats::SingularityMetrics;
use axum::{extract::State, routing::get, Json, Router};
use fixed::types::I16F16;
use libp2p::futures::StreamExt; // For select_next_some
use libp2p::gossipsub::IdentTopic; // Added helper
use libp2p::{
    gossipsub, identify, identity, mdns, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, PeerId, SwarmBuilder,
};
use qres_core::adaptive::regime_detector::{Regime, RegimeDetector};
use qres_core::adaptive::SilenceController;
use qres_core::consensus::krum::Bfp16Vec; // v19.0 Bfp16Vec
use qres_core::privacy::PrivacyAccountant;
use qres_core::resource_management::{energy_costs, EnergyPool};
use qres_core::tensor::{FixedTensor, I8F8};
use qres_core::zk_proofs::{ProofBundle, ZkNormProver};
use rand;
use serde::{Deserialize, Serialize}; // Added Deserialize
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{info, warn};

// Topic for brain synchronization
const BRAIN_TOPIC: &str = "qres-hive-v2";

// --- Swarm Configuration Constants ---

/// Buffer size for the FederatedAverager update queue.
const FEDERATION_BUFFER_SIZE: usize = 50;
/// Exponential decay half-life in seconds for federation weights.
const FEDERATION_HALF_LIFE_SECS: f64 = 300.0;

/// Initial differential privacy budget (epsilon) for the accountant.
const INITIAL_PRIVACY_BUDGET: f64 = 10.0;
/// Privacy failure probability delta.
const PRIVACY_DELTA: f64 = 1e-5;
/// Per-epoch privacy budget decay coefficient.
const PRIVACY_DECAY_COEFFICIENT: f64 = 0.995;

/// Window size for regime detection entropy tracking.
const REGIME_WINDOW_SIZE: usize = 100;
/// Entropy threshold for regime transition.
const REGIME_ENTROPY_THRESHOLD: f32 = 0.8;
/// Throughput threshold in bytes/sec for regime detection (1 MB/s).
const REGIME_THROUGHPUT_THRESHOLD: f32 = 1_000_000.0;

/// Total energy capacity for the daemon's energy pool.
const ENERGY_POOL_CAPACITY: u32 = 10_000;

/// Privacy cost charged per published Epiphany.
const EPIPHANY_PRIVACY_COST: f64 = 0.1;

/// Weight given to local confidence when merging federated updates.
const LOCAL_CONFIDENCE_WEIGHT: f32 = 0.9;
/// Weight given to aggregated confidence when merging.
const AGGREGATED_CONFIDENCE_WEIGHT: f32 = 0.1;

/// Brain broadcast interval in seconds.
const BRAIN_BROADCAST_INTERVAL_SECS: u64 = 10;
/// Federation epoch interval in seconds.
const FEDERATION_EPOCH_INTERVAL_SECS: u64 = 5;

/// Gossipsub heartbeat interval in seconds.
const GOSSIPSUB_HEARTBEAT_SECS: u64 = 1;

/// ZK proof L2 norm squared threshold.
const ZK_NORM_THRESHOLD: f32 = 10.0;
/// Reputation score threshold for accepting proofless updates.
const REPUTATION_TRUST_THRESHOLD: f32 = 80.0;

/// Elapsed milliseconds passed to regime detector on update.
const REGIME_UPDATE_INTERVAL_MS: usize = 10_000;

/// Singularity threshold: global error rate below this triggers singularity event.
const SINGULARITY_ERROR_THRESHOLD: f32 = 0.01;

// v19.0: Summary Gene for Fast Onboarding
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SummaryGene {
    pub round_index: u64,
    pub history_hash: [u8; 32],
    pub consensus: Bfp16Vec,
    pub variance: Bfp16Vec,
}

impl SummaryGene {
    pub fn new(round: u64, hash: [u8; 32], consensus: &[f32], variance: &[f32]) -> Self {
        Self {
            round_index: round,
            history_hash: hash,
            consensus: Bfp16Vec::from_f32_slice(consensus),
            variance: Bfp16Vec::from_f32_slice(variance),
        }
    }

    /// Serialize to optimized binary format for packet size verification
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(200);
        bytes.extend_from_slice(&self.round_index.to_be_bytes());
        bytes.extend_from_slice(&self.history_hash);

        // Manual BFP compression using qres_core arithmetic module
        bytes.extend(qres_core::encoding::arithmetic::compress_bfp(
            self.consensus.exponent,
            &self.consensus.mantissas,
        ));
        bytes.extend(qres_core::encoding::arithmetic::compress_bfp(
            self.variance.exponent,
            &self.variance.mantissas,
        ));

        bytes
    }
}

#[derive(Clone, Serialize, Default)]
pub struct SwarmStatus {
    pub peer_id: String,
    pub connected_peers: usize,
    pub known_peers: Vec<String>,
    pub brain_confidence: Vec<f32>,
    pub total_energy_consumed: u64,   // calibration metric
    pub energy_efficiency_ratio: f32, // useful work / total energy
}

pub struct AppState {
    pub local_peer_id: String,
    pub connected_peers: HashSet<String>,
    pub known_peers: HashSet<String>,
    pub brain: LivingBrain,
    pub peer_keys: PeerKeyStore,
    pub security: Option<SecurityManager>,
    pub reputation: ReputationManager,
    pub require_signatures: bool,
    pub aggregator: BrainAggregator,
    pub federated_averager: FederatedAverager,
    pub config: Config,
    pub privacy_accountant: PrivacyAccountant,
    pub zk_prover: ZkNormProver,
    pub regime_detector: RegimeDetector,
    pub silence_controller: SilenceController,
    pub energy_pool: EnergyPool, // Track energy for calibration
}

// Custom Behavior Struct
#[derive(NetworkBehaviour)]
pub struct QresBehavior {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
    pub identify: identify::Behaviour,
}

pub async fn start_p2p_node(
    brain_path: String,
    port: u16,
    key_path_override: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (id_keys, state) = setup_identity_and_state(key_path_override)?;
    spawn_status_api(state.clone(), port);
    let mut swarm = build_swarm(id_keys)?;
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    let mut broadcast_interval =
        tokio::time::interval(Duration::from_secs(BRAIN_BROADCAST_INTERVAL_SECS));
    let mut federation_epoch =
        tokio::time::interval(Duration::from_secs(FEDERATION_EPOCH_INTERVAL_SECS));

    loop {
        tokio::select! {
            _ = broadcast_interval.tick() => {
                handle_broadcast_tick(&state, &mut swarm, &brain_path).await;
            }
            _ = federation_epoch.tick() => {
                handle_federation_tick(&state, &brain_path).await;
            }
            event = swarm.select_next_some() => {
                handle_swarm_event(event, &state, &mut swarm).await;
            }
        }
    }
}

/// Initialize identity, config, security, reputation, and shared state.
#[allow(clippy::type_complexity)]
fn setup_identity_and_state(
    key_path_override: Option<String>,
) -> Result<(identity::Keypair, Arc<RwLock<AppState>>), Box<dyn std::error::Error>> {
    let id_keys = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(id_keys.public());
    info!(peer_id = %peer_id, "Local Peer ID generated");

    let config = Config::load().unwrap_or_default();
    let peer_keys = PeerKeyStore::new(
        &config.security.trusted_peers,
        &config.security.trusted_pubkeys,
    );

    // Priority: 1. CLI Override, 2. Config Key Path, 3. Auto-generate if required
    let security = if let Some(key_path_str) =
        key_path_override.or(config.security.key_path.clone())
    {
        let key_path = PathBuf::from(key_path_str);
        match SecurityManager::new(&key_path, config.security.require_signatures) {
            Ok(mgr) => {
                info!(pubkey = %mgr.public_key_hex(), path = ?key_path, "Security manager initialized");
                Some(mgr)
            }
            Err(e) => {
                warn!(error = %e, "Failed to initialize SecurityManager, running without signatures");
                None
            }
        }
    } else if config.security.require_signatures {
        let key_path = crate::config::qres_data_dir().join("node_key");
        match SecurityManager::new(&key_path, true) {
            Ok(mgr) => {
                info!(pubkey = %mgr.public_key_hex(), key_path = ?key_path, "Security manager auto-initialized");
                Some(mgr)
            }
            Err(e) => {
                warn!(error = %e, "Failed to auto-initialize SecurityManager");
                None
            }
        }
    } else {
        None
    };

    let rep_path = crate::config::qres_data_dir().join("reputation.json");
    let reputation = ReputationManager::new(rep_path);

    let state = Arc::new(RwLock::new(AppState {
        local_peer_id: peer_id.to_string(),
        connected_peers: HashSet::new(),
        known_peers: HashSet::new(),
        brain: LivingBrain::default(),
        peer_keys,
        security,
        reputation,
        require_signatures: config.security.require_signatures,
        aggregator: BrainAggregator::new(config.aggregation.clone()),
        federated_averager: FederatedAverager::new(
            FEDERATION_BUFFER_SIZE,
            FEDERATION_HALF_LIFE_SECS,
        ),
        config,
        privacy_accountant: PrivacyAccountant::new(
            INITIAL_PRIVACY_BUDGET,
            PRIVACY_DELTA,
            PRIVACY_DECAY_COEFFICIENT,
        ),
        zk_prover: ZkNormProver::new(),
        regime_detector: RegimeDetector::new(
            REGIME_WINDOW_SIZE,
            REGIME_ENTROPY_THRESHOLD,
            REGIME_THROUGHPUT_THRESHOLD,
        ),
        silence_controller: SilenceController::new(),
        energy_pool: EnergyPool::new(ENERGY_POOL_CAPACITY),
    }));

    Ok((id_keys, state))
}

/// Spawn the P2P status API on the given port.
fn spawn_status_api(state: Arc<RwLock<AppState>>, port: u16) {
    tokio::spawn(async move {
        let app = Router::new()
            .route("/status", get(get_status))
            .route("/brain", get(get_brain))
            .route("/health", get(get_health))
            .with_state(state);

        let addr_str = if std::env::var("QRES_PUBLIC").is_ok() {
            format!("0.0.0.0:{}", port)
        } else {
            format!("127.0.0.1:{}", port)
        };

        info!(address = addr_str, "API Server listening");
        let listener = match tokio::net::TcpListener::bind(&addr_str).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!(address = %addr_str, error = %e, "Failed to bind API listener");
                return;
            }
        };
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!(error = %e, "API server exited with error");
        }
    });
}

/// Build the libp2p swarm with gossipsub, mDNS, and identify protocols.
fn build_swarm(
    id_keys: identity::Keypair,
) -> Result<libp2p::Swarm<QresBehavior>, Box<dyn std::error::Error>> {
    let swarm = SwarmBuilder::with_existing_identity(id_keys)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|key| {
            let message_id_fn = |message: &gossipsub::Message| {
                let mut s = DefaultHasher::new();
                message.data.hash(&mut s);
                gossipsub::MessageId::from(s.finish().to_string())
            };
            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(GOSSIPSUB_HEARTBEAT_SECS))
                .validation_mode(gossipsub::ValidationMode::Permissive)
                .message_id_fn(message_id_fn)
                .build()
                .map_err(io::Error::other)?;

            let mut gossipsub = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )
            .map_err(io::Error::other)?;

            let topic = gossipsub::IdentTopic::new(BRAIN_TOPIC);
            gossipsub
                .subscribe(&topic)
                .map_err(|e| io::Error::other(format!("{:?}", e)))?;

            let mdns =
                mdns::tokio::Behaviour::new(mdns::Config::default(), PeerId::from(key.public()))?;

            let identify = identify::Behaviour::new(identify::Config::new(
                "qres/1.0.0".to_string(),
                key.public(),
            ));

            Ok(QresBehavior {
                gossipsub,
                mdns,
                identify,
            })
        })?
        .build();

    Ok(swarm)
}

/// Handle the periodic brain broadcast tick (privacy, silence, ZK proofs, signing, publishing).
async fn handle_broadcast_tick(
    state: &Arc<RwLock<AppState>>,
    swarm: &mut libp2p::Swarm<QresBehavior>,
    brain_file: &str,
) {
    // Privacy accounting: decay budget
    {
        state.write().await.privacy_accountant.decay();
    }

    let epiphany_cost = EPIPHANY_PRIVACY_COST;

    let should_publish = {
        let app_state = state.read().await;
        match app_state.privacy_accountant.check_budget(epiphany_cost) {
            Ok(_) => true,
            Err(_) => {
                info!("Privacy budget exhausted. Entering Listen-Only Mode.");
                false
            }
        }
    };

    // Strategic silence gate
    let should_silence = {
        let mut app_state = state.write().await;
        let entropy = calculate_brain_entropy(&app_state.brain);
        let current_regime = app_state.regime_detector.current_regime();
        let variance_stable = app_state.regime_detector.is_stable_enough_for_silence();
        let calm_streak = app_state.regime_detector.calm_streak();

        app_state
            .silence_controller
            .transition(current_regime, variance_stable, calm_streak);

        if matches!(current_regime, Regime::Storm) {
            false
        } else {
            let reputation = app_state.reputation.get_trust(&app_state.local_peer_id);
            let energy_ratio = app_state.energy_pool.ratio();
            !app_state.silence_controller.should_broadcast(
                entropy,
                reputation,
                energy_ratio,
                energy_costs::GOSSIP_SEND,
            )
        }
    };

    if should_silence {
        info!("Strategic Silence: Suppressing broadcast (low utility)");
    }

    if should_publish && !should_silence {
        let can_afford = {
            let mut app_state = state.write().await;
            app_state.energy_pool.spend(energy_costs::GOSSIP_SEND)
        };

        if can_afford {
            if let Ok(content) = fs::read_to_string(brain_file) {
                if let Some(mut brain) = LivingBrain::from_json(&content) {
                    state.write().await.brain = brain.clone();

                    let current_regime = state.read().await.regime_detector.current_regime();
                    let is_storm = matches!(current_regime, Regime::Storm);

                    // Adaptive quantization (Storm: I8F8 to halve bandwidth)
                    if is_storm {
                        if let Some(w_bytes) = &brain.best_engine_weights {
                            let i16f16_weights: Vec<I16F16> = w_bytes
                                .chunks(4)
                                .filter_map(|chunk| {
                                    if chunk.len() == 4 {
                                        let bits = i32::from_le_bytes(chunk.try_into().unwrap());
                                        Some(I16F16::from_bits(bits))
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            let fixed_tensor = FixedTensor::new(i16f16_weights);
                            let i8f8_weights = fixed_tensor.quantize_to_i8f8();
                            let quantized_bytes: Vec<u8> =
                                i8f8_weights.iter().flat_map(|&w| w.to_le_bytes()).collect();
                            brain.best_engine_weights = Some(quantized_bytes);
                        }
                    }

                    // ZK proof generation (Calm mode only)
                    let weights_f32: Vec<f32> = if !is_storm {
                        if let Some(w_bytes) = &brain.best_engine_weights {
                            w_bytes
                                .chunks(4)
                                .filter_map(|chunk| {
                                    if chunk.len() == 4 {
                                        let bits = i32::from_le_bytes(chunk.try_into().unwrap());
                                        let fixed = fixed::types::I16F16::from_bits(bits);
                                        Some(fixed.to_num::<f32>())
                                    } else {
                                        None
                                    }
                                })
                                .collect()
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    };

                    let proof_bundle = if !is_storm {
                        let app_state = state.read().await;
                        if !weights_f32.is_empty() {
                            if let Some((proof, _)) = app_state
                                .zk_prover
                                .generate_proof(&weights_f32, ZK_NORM_THRESHOLD)
                            {
                                Some(ProofBundle {
                                    peer_id: [0u8; 32],
                                    masked_weights: weights_f32,
                                    zk_proof: proof,
                                })
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // Sign and publish
                    let timestamp = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    let nonce = rand::random::<u64>();

                    let mut epiphany = SignedEpiphany {
                        brain: brain.clone(),
                        proof_bundle: proof_bundle.clone(),
                        signature: String::new(),
                        sender_id: state
                            .read()
                            .await
                            .security
                            .as_ref()
                            .map(|s| s.public_key_hex())
                            .unwrap_or_default(),
                        timestamp,
                        nonce,
                        is_storm_mode: is_storm,
                    };

                    let payload_bytes = epiphany.payload_bytes();
                    let signed_payload = {
                        let app_state = state.read().await;
                        if let Some(sec_mgr) = &app_state.security {
                            sec_mgr.sign(&payload_bytes)
                        } else {
                            SignedPayload {
                                data: payload_bytes,
                                signature: String::new(),
                                signer_pubkey: String::new(),
                                timestamp,
                                nonce,
                            }
                        }
                    };
                    epiphany.signature = signed_payload.signature;

                    let msg_bytes = serde_json::to_vec(&epiphany).unwrap();
                    let outgoing_bytes = msg_bytes.len() as u64;
                    let topic = IdentTopic::new(BRAIN_TOPIC);
                    if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic, msg_bytes) {
                        tracing::error!("Publish error: {:?}", e);
                    } else {
                        let mut app_state = state.write().await;
                        let _ = app_state
                            .privacy_accountant
                            .record_consumption(epiphany_cost);
                        let entropy = calculate_brain_entropy(&brain);
                        app_state.regime_detector.update(
                            entropy,
                            REGIME_UPDATE_INTERVAL_MS,
                            outgoing_bytes,
                        );
                        info!(
                            "Published SignedEpiphany (mode: {})",
                            if is_storm { "Storm" } else { "Calm" }
                        );
                    }
                }
            }
        } else {
            info!(
                "Energy Critical: Broadcast inhibited (Ratio: {:.2})",
                state.read().await.energy_pool.ratio()
            );
        }
    }
}

/// Handle the federated learning aggregation epoch.
async fn handle_federation_tick(state: &Arc<RwLock<AppState>>, brain_file: &str) {
    let mut app_state = state.write().await;
    if !app_state.federated_averager.should_aggregate() {
        return;
    }

    let reputation_clone = app_state.reputation.clone();
    if let Some((aggregated_weights, aggregated_confidence)) =
        app_state.federated_averager.aggregate(&reputation_clone)
    {
        if let Ok(local_json) = fs::read_to_string(brain_file) {
            if let Some(mut local_brain) = LivingBrain::from_json(&local_json) {
                local_brain.best_engine_weights = Some(aggregated_weights);

                for (local_conf, &agg_conf) in local_brain
                    .confidence
                    .iter_mut()
                    .zip(aggregated_confidence.iter())
                {
                    *local_conf = *local_conf * LOCAL_CONFIDENCE_WEIGHT
                        + agg_conf * AGGREGATED_CONFIDENCE_WEIGHT;
                }

                let global_error_rate = 1.0
                    - (aggregated_confidence.iter().sum::<f32>()
                        / aggregated_confidence.len() as f32);
                if global_error_rate < SINGULARITY_ERROR_THRESHOLD {
                    info!(
                        "🎯 SINGULARITY ACHIEVED! Global error rate: {:.6}",
                        global_error_rate
                    );
                }

                let local_loss = 1.0
                    - (local_brain.confidence.iter().sum::<f32>()
                        / local_brain.confidence.len() as f32);
                let swarm_variance = aggregated_confidence
                    .iter()
                    .map(|&c| (c - global_error_rate).powi(2))
                    .sum::<f32>()
                    / aggregated_confidence.len() as f32;

                let metrics = SingularityMetrics::new(
                    local_loss,
                    swarm_variance.sqrt(),
                    app_state.connected_peers.len(),
                    app_state.energy_pool.lifetime_consumption(),
                    app_state.energy_pool.ratio(),
                );
                if let Err(e) = metrics.export_csv() {
                    warn!("Failed to export singularity metrics: {}", e);
                }

                let _ = fs::write(brain_file, local_brain.to_json());
                app_state.brain = local_brain;
                info!(
                    "Applied federated aggregation. Global error rate: {:.4}",
                    global_error_rate
                );
            }
        }
    }
}

/// Dispatch and handle swarm events (connections, mDNS, identify, gossipsub messages).
async fn handle_swarm_event(
    event: SwarmEvent<QresBehaviorEvent>,
    state: &Arc<RwLock<AppState>>,
    swarm: &mut libp2p::Swarm<QresBehavior>,
) {
    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            info!(address = %address, "Swarm listening");
        }
        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            info!(peer_id = %peer_id, "Connected to peer");
            state
                .write()
                .await
                .connected_peers
                .insert(peer_id.to_string());

            // v19.0: Serve Summary Gene (Mid-Flight Join)
            {
                let state_read = state.read().await;
                let brain = &state_read.brain;
                let dims = brain.confidence.len().min(8);
                let consensus = &brain.confidence[..dims];
                let variance = vec![0.0; dims];

                let summary = SummaryGene::new(1900, [0xAA; 32], consensus, &variance);
                let bytes = summary.to_bytes();
                info!(
                    peer_id = %peer_id,
                    size_bytes = bytes.len(),
                    "served_summary_gene" = true,
                    "mid_flight_join" = "active",
                    "Serving Summary Gene instead of Event Log"
                );
            }
        }
        SwarmEvent::ConnectionClosed { peer_id, .. } => {
            info!(peer_id = %peer_id, "Disconnected from peer");
            state
                .write()
                .await
                .connected_peers
                .remove(&peer_id.to_string());
        }
        SwarmEvent::Behaviour(QresBehaviorEvent::Mdns(mdns::Event::Discovered(list))) => {
            for (peer_id, multiaddr) in list {
                info!(peer_id = %peer_id, "mDNS Discovered");
                state.write().await.known_peers.insert(peer_id.to_string());
                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                let _ = swarm.dial(multiaddr);
            }
        }
        SwarmEvent::Behaviour(QresBehaviorEvent::Mdns(mdns::Event::Expired(list))) => {
            for (peer_id, _multiaddr) in list {
                info!(peer_id = %peer_id, "mDNS Expired");
                state.write().await.known_peers.remove(&peer_id.to_string());
                swarm
                    .behaviour_mut()
                    .gossipsub
                    .remove_explicit_peer(&peer_id);
            }
        }
        SwarmEvent::Behaviour(QresBehaviorEvent::Identify(identify::Event::Received {
            peer_id,
            info,
        })) => {
            info!(peer_id = %peer_id, agent = %info.agent_version, "Received Identify from peer");
            let mut app_state = state.write().await;
            if app_state.peer_keys.add_peer_key(peer_id, info.public_key) {
                info!(peer_id = %peer_id, known_keys = app_state.peer_keys.peer_count(), "Peer key verified and stored");
            }
        }
        SwarmEvent::Behaviour(QresBehaviorEvent::Identify(identify::Event::Sent { peer_id })) => {
            info!(peer_id = %peer_id, "Sent Identify to peer");
        }
        SwarmEvent::Behaviour(QresBehaviorEvent::Identify(identify::Event::Error {
            peer_id,
            error,
        })) => {
            warn!(peer_id = %peer_id, error = %error, "Identify error");
        }
        SwarmEvent::Behaviour(QresBehaviorEvent::Gossipsub(gossipsub::Event::Message {
            propagation_source: _,
            message_id: _,
            message,
        })) => {
            handle_gossipsub_message(&message, state).await;
        }
        _ => {}
    }
}

/// Process an incoming gossipsub message (verify signature, verify ZK proof, buffer for federation).
async fn handle_gossipsub_message(message: &gossipsub::Message, state: &Arc<RwLock<AppState>>) {
    let signed_epiphany = match serde_json::from_slice::<SignedEpiphany>(&message.data) {
        Ok(e) => e,
        Err(_) => {
            warn!("Failed to deserialize SignedEpiphany");
            return;
        }
    };

    // Reconstruct and verify signature
    let payload_to_verify = SignedPayload {
        data: signed_epiphany.payload_bytes(),
        signature: signed_epiphany.signature.clone(),
        signer_pubkey: signed_epiphany.sender_id.clone(),
        timestamp: signed_epiphany.timestamp,
        nonce: signed_epiphany.nonce,
    };

    let sig_valid = {
        let mut app_state = state.write().await;
        if let Some(security_mgr) = &mut app_state.security {
            match security_mgr.verify(&payload_to_verify) {
                Ok(_) => true,
                Err(e) => {
                    warn!("Verification Failed: {}", e);
                    false
                }
            }
        } else {
            !app_state.require_signatures
        }
    };

    if !sig_valid {
        warn!("Invalid Signature from {}", signed_epiphany.sender_id);
        state
            .write()
            .await
            .reputation
            .punish(&signed_epiphany.sender_id);
        return;
    }

    // Verify ZK proof or trust high-reputation peers
    let proof_valid = {
        let app_state = state.read().await;
        if let Some(bundle) = &signed_epiphany.proof_bundle {
            app_state
                .zk_prover
                .verify_proof(&bundle.zk_proof, ZK_NORM_THRESHOLD)
        } else {
            let reputation_score = app_state.reputation.get_trust(&signed_epiphany.sender_id);
            signed_epiphany.is_storm_mode || reputation_score > REPUTATION_TRUST_THRESHOLD
        }
    };

    if !proof_valid {
        warn!(
            "Rejected SignedEpiphany from {}: missing/invalid proof and low reputation",
            signed_epiphany.sender_id
        );
        state
            .write()
            .await
            .reputation
            .punish(&signed_epiphany.sender_id);
        return;
    }

    // Handle Storm Mode upcasting (I8F8 -> I16F16)
    let mut processed_brain = signed_epiphany.brain.clone();
    if signed_epiphany.is_storm_mode {
        if let Some(w_bytes) = &processed_brain.best_engine_weights {
            let i8f8_weights: Vec<I8F8> = w_bytes
                .chunks(2)
                .filter_map(|chunk| {
                    if chunk.len() == 2 {
                        let bits = i16::from_le_bytes(chunk.try_into().unwrap());
                        Some(I8F8::from_bits(bits))
                    } else {
                        None
                    }
                })
                .collect();
            let fixed_tensor = FixedTensor::from_i8f8(&i8f8_weights);
            let i16f16_bytes: Vec<u8> = fixed_tensor
                .data
                .iter()
                .flat_map(|&w| w.to_le_bytes())
                .collect();
            processed_brain.best_engine_weights = Some(i16f16_bytes);
        }
    }

    // Buffer for federated learning
    let mut app_state = state.write().await;
    app_state
        .federated_averager
        .add_update(signed_epiphany.clone());

    let incoming_bytes = message.data.len() as u64;
    let entropy = calculate_brain_entropy(&processed_brain);
    app_state
        .regime_detector
        .update(entropy, REGIME_UPDATE_INTERVAL_MS, incoming_bytes);

    app_state.reputation.reward(&signed_epiphany.sender_id);
    info!(
        "Buffered SignedEpiphany (mode: {}) for federated averaging",
        if signed_epiphany.is_storm_mode {
            "Storm"
        } else {
            "Calm"
        }
    );
}

// Handlers
async fn get_status(State(state): State<Arc<RwLock<AppState>>>) -> Json<SwarmStatus> {
    let s = state.read().await;
    Json(SwarmStatus {
        peer_id: s.local_peer_id.clone(),
        connected_peers: s.connected_peers.len(),
        known_peers: s.known_peers.iter().cloned().collect(),
        brain_confidence: s.brain.confidence.to_vec(),
        total_energy_consumed: s.energy_pool.lifetime_consumption(),
        energy_efficiency_ratio: s.energy_pool.ratio(), // Re-purposing ratio for now as 'current charge %'
    })
}

async fn get_brain(State(state): State<Arc<RwLock<AppState>>>) -> Json<LivingBrain> {
    let s = state.read().await;
    Json(s.brain.clone())
}

async fn get_health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Calculate entropy from brain confidence (Shannon entropy)
fn calculate_brain_entropy(brain: &LivingBrain) -> f32 {
    let mut entropy = 0.0;
    for &conf in &brain.confidence {
        if conf > 0.0 {
            entropy -= conf * conf.ln(); // Assuming normalized
        }
    }
    entropy
}
