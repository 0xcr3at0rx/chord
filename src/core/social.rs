use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::core::remote::pb::{SessionStatus, PeerInfo};
use ed25519_dalek::{SigningKey, Verifier, VerifyingKey, Signature};
use rand::Rng;

#[derive(Clone)]
pub struct SocialSession {
    pub session_id: String,
    pub host_id: String,
    pub peers: Arc<RwLock<Vec<PeerInfo>>>,
    pub invite_token: String,
}

impl SocialSession {
    pub fn new(host_id: String, host_name: String, public_key: Vec<u8>) -> Self {
        let session_id = Uuid::new_v4().to_string();
        let invite_token = Uuid::new_v4().to_string(); 
        
        let host_peer = PeerInfo {
            id: host_id.clone(),
            name: host_name,
            is_host: true,
            latency_us: 0,
            public_ip: String::new(),
            public_port: 0,
            public_key,
        };

        Self {
            session_id,
            host_id,
            peers: Arc::new(RwLock::new(vec![host_peer])),
            invite_token,
        }
    }

    pub async fn to_status(&self) -> SessionStatus {
        let peers = self.peers.read().await;
        SessionStatus {
            session_id: self.session_id.clone(),
            host_id: self.host_id.clone(),
            peers: peers.clone(),
            start_timestamp_us: 0, 
        }
    }

    pub async fn add_peer(&self, peer_info: PeerInfo) {
        let mut peers = self.peers.write().await;
        if !peers.iter().any(|p| p.id == peer_info.id) {
            peers.push(peer_info);
        }
    }

    pub async fn remove_peer(&self, peer_id: &str) {
        let mut peers = self.peers.write().await;
        peers.retain(|p| p.id != peer_id);
    }
}

pub struct SocialManager {
    pub active_session: Arc<RwLock<Option<SocialSession>>>,
    pub device_id: String,
    pub device_name: String,
    pub signing_key: SigningKey,
}

impl SocialManager {
    pub fn new(device_id: String, device_name: String) -> Self {
        // Generate identity for this device
        let mut seed = [0u8; 32];
        rand::rng().fill_bytes(&mut seed);
        let signing_key = SigningKey::from_bytes(&seed);

        Self {
            active_session: Arc::new(RwLock::new(None)),
            device_id,
            device_name,
            signing_key,
        }
    }

    pub async fn create_session(&self) -> String {
        let pub_key = self.signing_key.verifying_key().to_bytes().to_vec();
        let session = SocialSession::new(self.device_id.clone(), self.device_name.clone(), pub_key);
        let token = session.invite_token.clone();
        let mut active = self.active_session.write().await;
        *active = Some(session);
        token
    }

    pub async fn leave_session(&self) {
        let mut active = self.active_session.write().await;
        *active = None;
    }

    pub async fn sign_data(&self, data: &[u8]) -> Vec<u8> {
        use ed25519_dalek::Signer;
        self.signing_key.sign(data).to_bytes().to_vec()
    }

    pub fn verify_signature(&self, data: &[u8], sig_bytes: &[u8], pub_key_bytes: &[u8]) -> bool {
        if let Ok(verifying_key) = VerifyingKey::try_from(pub_key_bytes) {
            if let Ok(sig) = Signature::from_slice(sig_bytes) {
                return verifying_key.verify(data, &sig).is_ok();
            }
        }
        false
    }
}
