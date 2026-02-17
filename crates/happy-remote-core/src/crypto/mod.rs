//! Encryption module using NaCl (libsodium)
//!
//! Provides X25519 key exchange and XSalsa20-Poly1305 authenticated encryption

mod nacl;

pub use nacl::NaClEngine;

use happy_types::{DataKey, EncryptedMessage, KeyExchange, KeyPair, Nonce, PublicKey, SecretKey};
use crate::{HappyError, Result};

/// Encryption engine trait for E2E encryption
pub trait EncryptionEngine: Send + Sync {
    /// Generate a new X25519 key pair
    fn generate_keypair(&self) -> KeyPair;

    /// Generate a random data key for session encryption
    fn generate_data_key(&self) -> DataKey;

    /// Generate a random nonce
    fn generate_nonce(&self) -> Nonce;

    /// Encrypt plaintext using XSalsa20-Poly1305 with the given keys
    fn encrypt(
        &self,
        plaintext: &[u8],
        recipient_pk: &PublicKey,
        sender_sk: &SecretKey,
    ) -> Result<EncryptedMessage>;

    /// Decrypt ciphertext using XSalsa20-Poly1305 with the given keys
    fn decrypt(
        &self,
        encrypted: &EncryptedMessage,
        sender_pk: &PublicKey,
        recipient_sk: &SecretKey,
    ) -> Result<Vec<u8>>;

    /// Perform X25519 key exchange to derive a shared secret
    fn key_exchange(&self, public_key: &PublicKey, secret_key: &SecretKey) -> Result<[u8; 32]>;

    /// Encrypt a data key using the shared secret from key exchange
    fn encrypt_data_key(
        &self,
        data_key: &DataKey,
        recipient_pk: &PublicKey,
        sender_sk: &SecretKey,
    ) -> Result<KeyExchange>;

    /// Decrypt a data key using the shared secret from key exchange
    fn decrypt_data_key(
        &self,
        key_exchange: &KeyExchange,
        sender_pk: &PublicKey,
        recipient_sk: &SecretKey,
    ) -> Result<DataKey>;

    /// Encrypt with a symmetric data key (for session data)
    fn encrypt_symmetric(&self, plaintext: &[u8], key: &DataKey, nonce: &Nonce) -> Result<Vec<u8>>;

    /// Decrypt with a symmetric data key (for session data)
    fn decrypt_symmetric(&self, ciphertext: &[u8], key: &DataKey, nonce: &Nonce) -> Result<Vec<u8>>;
}

/// Initialize libsodium
pub fn init() -> Result<()> {
    sodiumoxide::init().map_err(|_| HappyError::Encryption("Failed to initialize sodium".to_string()))
}
