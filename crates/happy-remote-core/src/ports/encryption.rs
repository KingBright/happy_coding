//! Encryption port trait

use happy_types::{DataKey, EncryptedMessage, KeyExchange, KeyPair, Nonce, PublicKey, SecretKey};
use crate::Result;

/// Port for encryption operations (wrapper around EncryptionEngine)
pub trait EncryptionPort: Send + Sync {
    fn generate_keypair(&self) -> KeyPair;
    fn generate_data_key(&self) -> DataKey;
    fn generate_nonce(&self) -> Nonce;
    fn encrypt(&self, plaintext: &[u8], recipient_pk: &PublicKey, sender_sk: &SecretKey) -> Result<EncryptedMessage>;
    fn decrypt(&self, encrypted: &EncryptedMessage, sender_pk: &PublicKey, recipient_sk: &SecretKey) -> Result<Vec<u8>>;
    fn encrypt_data_key(&self, data_key: &DataKey, recipient_pk: &PublicKey, sender_sk: &SecretKey) -> Result<KeyExchange>;
    fn decrypt_data_key(&self, key_exchange: &KeyExchange, sender_pk: &PublicKey, recipient_sk: &SecretKey) -> Result<DataKey>;
    fn encrypt_symmetric(&self, plaintext: &[u8], key: &DataKey, nonce: &Nonce) -> Result<Vec<u8>>;
    fn decrypt_symmetric(&self, ciphertext: &[u8], key: &DataKey, nonce: &Nonce) -> Result<Vec<u8>>;
}
