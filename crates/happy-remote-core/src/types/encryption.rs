//! Encryption types

use serde::{Deserialize, Serialize};

/// X25519 public key (32 bytes)
pub type PublicKey = [u8; 32];

/// X25519 secret key (32 bytes)
pub type SecretKey = [u8; 32];

/// XSalsa20 nonce (24 bytes)
pub type Nonce = [u8; 24];

/// Encrypted message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedMessage {
    /// XSalsa20 nonce (24 bytes)
    pub nonce: Nonce,
    /// Encrypted payload
    pub ciphertext: Vec<u8>,
    /// Sender's X25519 public key (32 bytes)
    pub sender_pubkey: PublicKey,
}

/// Key exchange structure for establishing session keys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyExchange {
    /// Ephemeral X25519 public key
    pub ephemeral_pubkey: PublicKey,
    /// Data key encrypted with shared secret
    pub encrypted_data_key: Vec<u8>,
}

/// Key pair for E2E encryption
#[derive(Debug, Clone)]
pub struct KeyPair {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
}

/// Session data key (32 bytes for XSalsa20)
pub type DataKey = [u8; 32];

impl EncryptedMessage {
    pub fn new(nonce: Nonce, ciphertext: Vec<u8>, sender_pubkey: PublicKey) -> Self {
        Self {
            nonce,
            ciphertext,
            sender_pubkey,
        }
    }
}

impl KeyExchange {
    pub fn new(ephemeral_pubkey: PublicKey, encrypted_data_key: Vec<u8>) -> Self {
        Self {
            ephemeral_pubkey,
            encrypted_data_key,
        }
    }
}
