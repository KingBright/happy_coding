//! NaCl/libsodium encryption implementation using crypto_box (XSalsa20Poly1305)

use super::{
    DataKey, EncryptedMessage, EncryptionEngine, KeyExchange, KeyPair, Nonce, PublicKey, SecretKey,
};
use crate::{HappyError, Result};
use crypto_box::{
    aead::{Aead, OsRng},
    SalsaBox,
};
use xsalsa20poly1305::XSalsa20Poly1305;

/// NaCl encryption engine
pub struct NaClEngine;

impl NaClEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NaClEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl EncryptionEngine for NaClEngine {
    fn generate_keypair(&self) -> KeyPair {
        let secret_key = crypto_box::SecretKey::generate(&mut OsRng);
        let public_key = secret_key.public_key();

        KeyPair {
            public_key: (*public_key.as_bytes()).into(),
            secret_key: secret_key.to_bytes().into(),
        }
    }

    fn generate_data_key(&self) -> DataKey {
        let mut key = [0u8; 32];
        use rand::RngCore;
        OsRng.fill_bytes(&mut key);
        key
    }

    fn generate_nonce(&self) -> Nonce {
        // crypto_box uses 24-byte nonces (XSalsa20)
        let mut nonce = [0u8; 24];
        use rand::RngCore;
        OsRng.fill_bytes(&mut nonce);
        nonce
    }

    fn encrypt(
        &self,
        plaintext: &[u8],
        recipient_pk: &PublicKey,
        sender_sk: &SecretKey,
    ) -> Result<EncryptedMessage> {
        let recipient_pk_obj = crypto_box::PublicKey::from(*recipient_pk);
        let sender_sk_obj = crypto_box::SecretKey::from(*sender_sk);
        let sender_pk_obj = sender_sk_obj.public_key();

        // Box::new creates a precomputed shared key for faster operations if reused,
        // but here we just do one-shot.
        let parsed_box = SalsaBox::new(&recipient_pk_obj, &sender_sk_obj);
        let nonce_bytes = self.generate_nonce();
        let nonce = xsalsa20poly1305::Nonce::from_slice(&nonce_bytes);

        let ciphertext = parsed_box
            .encrypt(nonce, plaintext)
            .map_err(|_| HappyError::Encryption("Encryption failed".to_string()))?;

        Ok(EncryptedMessage::new(
            nonce_bytes,
            ciphertext,
            (*sender_pk_obj.as_bytes()).into(),
        ))
    }

    fn decrypt(
        &self,
        encrypted: &EncryptedMessage,
        sender_pk: &PublicKey,
        recipient_sk: &SecretKey,
    ) -> Result<Vec<u8>> {
        let sender_pk = crypto_box::PublicKey::from(*sender_pk);
        let recipient_sk = crypto_box::SecretKey::from(*recipient_sk);

        let parsed_box = SalsaBox::new(&sender_pk, &recipient_sk);
        let nonce = xsalsa20poly1305::Nonce::from_slice(&encrypted.nonce);

        let plaintext = parsed_box
            .decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|_| HappyError::Decryption("Decryption failed".to_string()))?;

        Ok(plaintext)
    }

    fn key_exchange(&self, _public_key: &PublicKey, _secret_key: &SecretKey) -> Result<[u8; 32]> {
        // crypto_box doesn't expose raw Diffie-Hellman easily in the high-level API,
        // but we can usually derive it. However, typically we use Box directly.
        // If we need raw shared secret, we might need x25519-dalek.
        // For now, let's keep this stubbed or return error if not strictly needed by high-level flows.
        // Or implement using lower-level crate if required.
        Err(HappyError::Encryption(
            "Raw key exchange not supported by this engine yet".to_string(),
        ))
    }

    fn encrypt_data_key(
        &self,
        data_key: &DataKey,
        recipient_pk: &PublicKey,
        sender_sk: &SecretKey,
    ) -> Result<KeyExchange> {
        // Encrypt the data key using public key auth encryption (Box)
        // We generate an ephemeral keypair for forward secrecy if we wanted,
        // but the interface implies we use sender_sk.
        // So we just use `encrypt` logic but wrap in KeyExchange struct.

        // Note: KeyExchange struct usually stores ephemeral PK.
        // If we use static sender_sk, we might not need ephemeral.
        // BUT, usually `KeyExchange` implies Ephemeral-Static DH.
        // Let's generate an ephemeral keypair here.

        // Unused sender_sk in this path if we generate ephemeral
        let _ = sender_sk;

        let ephemeral_sk = crypto_box::SecretKey::generate(&mut OsRng);
        let ephemeral_pk = ephemeral_sk.public_key();

        let recipient_pk_obj = crypto_box::PublicKey::from(*recipient_pk);
        let parsed_box = SalsaBox::new(&recipient_pk_obj, &ephemeral_sk);

        let nonce_bytes = self.generate_nonce();
        let nonce = xsalsa20poly1305::Nonce::from_slice(&nonce_bytes);

        // We encrypt the data_key
        // Standard practice: Prepend Nonce to ciphertext.

        let ciphertext = parsed_box
            .encrypt(nonce, data_key.as_slice())
            .map_err(|_| HappyError::Encryption("Data key encryption failed".to_string()))?;

        let mut final_ciphertext = nonce_bytes.to_vec();
        final_ciphertext.extend(ciphertext);

        Ok(KeyExchange::new(
            (*ephemeral_pk.as_bytes()).into(),
            final_ciphertext,
        ))
    }

    fn decrypt_data_key(
        &self,
        key_exchange: &KeyExchange,
        _sender_pk: &PublicKey, // Not used if using ephemeral?
        recipient_sk: &SecretKey,
    ) -> Result<DataKey> {
        // Sender PK here is the Ephemeral PK from the message
        let ephemeral_pk_bytes = key_exchange.ephemeral_pubkey;
        let ephemeral_pk = crypto_box::PublicKey::from(ephemeral_pk_bytes);

        let recipient_sk_obj = crypto_box::SecretKey::from(*recipient_sk);
        let parsed_box = SalsaBox::new(&ephemeral_pk, &recipient_sk_obj);

        // Extract nonce (first 24 bytes)
        if key_exchange.encrypted_data_key.len() < 24 {
            return Err(HappyError::Decryption(
                "Invalid encrypted data key length".to_string(),
            ));
        }

        let (nonce_bytes, ciphertext) = key_exchange.encrypted_data_key.split_at(24);
        let nonce = xsalsa20poly1305::Nonce::from_slice(nonce_bytes);

        let plaintext = parsed_box
            .decrypt(nonce, ciphertext)
            .map_err(|_| HappyError::Decryption("Data key decryption failed".to_string()))?;

        if plaintext.len() != 32 {
            return Err(HappyError::Decryption(
                "Decrypted data key has wrong length".to_string(),
            ));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&plaintext);
        Ok(key)
    }

    // Symmetric encryption (XSalsa20Poly1305)
    fn encrypt_symmetric(&self, plaintext: &[u8], key: &DataKey, nonce: &Nonce) -> Result<Vec<u8>> {
        use xsalsa20poly1305::KeyInit;

        let cipher = XSalsa20Poly1305::new(key.into());
        let nonce_obj = xsalsa20poly1305::Nonce::from_slice(nonce);

        cipher
            .encrypt(nonce_obj, plaintext)
            .map_err(|_| HappyError::Encryption("Symmetric encryption failed".to_string()))
    }

    fn decrypt_symmetric(
        &self,
        ciphertext: &[u8],
        key: &DataKey,
        nonce: &Nonce,
    ) -> Result<Vec<u8>> {
        use xsalsa20poly1305::KeyInit;

        let cipher = XSalsa20Poly1305::new(key.into());
        let nonce_obj = xsalsa20poly1305::Nonce::from_slice(nonce);

        cipher
            .decrypt(nonce_obj, ciphertext)
            .map_err(|_| HappyError::Decryption("Symmetric decryption failed".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let engine = NaClEngine::new();
        let kp = engine.generate_keypair();
        assert_eq!(kp.public_key.len(), 32);
        assert_eq!(kp.secret_key.len(), 32);
        assert_ne!(kp.public_key, [0u8; 32]);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let engine = NaClEngine::new();
        let alice = engine.generate_keypair();
        let bob = engine.generate_keypair();

        let plaintext = b"Hello, World!";
        let encrypted = engine
            .encrypt(plaintext, &bob.public_key, &alice.secret_key)
            .unwrap();

        // Ensure ciphertext is different
        assert_ne!(encrypted.ciphertext, plaintext);

        let decrypted = engine
            .decrypt(&encrypted, &alice.public_key, &bob.secret_key)
            .unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_failure() {
        let engine = NaClEngine::new();
        let alice = engine.generate_keypair();
        let bob = engine.generate_keypair();
        let eve = engine.generate_keypair();

        let plaintext = b"Secret";
        let encrypted = engine
            .encrypt(plaintext, &bob.public_key, &alice.secret_key)
            .unwrap();

        // Eve tries to decrypt
        let result = engine.decrypt(&encrypted, &alice.public_key, &eve.secret_key);
        assert!(result.is_err());

        // Bob tries to decrypt perceiving sender as Eve
        let result = engine.decrypt(&encrypted, &eve.public_key, &bob.secret_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_data_key_exchange() {
        let engine = NaClEngine::new();
        let alice = engine.generate_keypair(); // Sender
        let bob = engine.generate_keypair(); // Recipient

        let data_key = engine.generate_data_key();

        // Alice sends to Bob. Uses Alice's SK but logic uses Ephemeral inside.
        // Wait, our implementation of encrypt_data_key generates ephemeral.
        // The sender_sk arg is unused currently in that fn.
        let key_exchange = engine
            .encrypt_data_key(&data_key, &bob.public_key, &alice.secret_key)
            .unwrap();

        // Bob decrypts
        let decrypted_key = engine
            .decrypt_data_key(&key_exchange, &alice.public_key, &bob.secret_key)
            .unwrap();

        assert_eq!(decrypted_key, data_key);
    }

    #[test]
    fn test_symmetric_encryption() {
        let engine = NaClEngine::new();
        let key = engine.generate_data_key();
        let nonce = engine.generate_nonce();

        let plaintext = b"Secret message";
        let ciphertext = engine.encrypt_symmetric(plaintext, &key, &nonce).unwrap();

        assert_ne!(ciphertext, plaintext);

        let decrypted = engine.decrypt_symmetric(&ciphertext, &key, &nonce).unwrap();

        assert_eq!(decrypted, plaintext);
    }
}
