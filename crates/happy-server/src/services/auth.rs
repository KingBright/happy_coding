//! Authentication service

use crate::storage::Database;
use anyhow::{Context, Result};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub struct AuthService {
    db: Arc<Database>,
    jwt_secret: String,
}

impl AuthService {
    pub fn new(db: Arc<Database>, jwt_secret: String) -> Self {
        Self { db, jwt_secret }
    }

    pub async fn register(
        &self,
        email: &str,
        password: &str,
        name: Option<&str>,
    ) -> Result<AuthTokens> {
        // Hash password
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?
            .to_string();

        // Create user in database
        let user_id = self.db.create_user(email, &password_hash, name).await?;

        // Generate tokens
        self.generate_tokens(&user_id).await
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<AuthTokens> {
        // Get user from database
        let user = self.db.get_user_by_email(email).await?;

        if let Some((user_id, password_hash)) = user {
            // Verify password
            let parsed_hash = PasswordHash::new(&password_hash)
                .map_err(|e| anyhow::anyhow!("Invalid password hash: {}", e))?;
            let argon2 = Argon2::default();

            if argon2
                .verify_password(password.as_bytes(), &parsed_hash)
                .is_ok()
            {
                return self.generate_tokens(&user_id).await;
            }
        }

        anyhow::bail!("Invalid credentials")
    }

    pub async fn validate_token(&self, token: &str) -> Result<String> {
        let validation = Validation::default();
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &validation,
        )?;

        Ok(token_data.claims.sub)
    }

    async fn generate_tokens(&self, user_id: &str) -> Result<AuthTokens> {
        let now = Utc::now();

        // Access token (permanent - 100 years)
        // Using a very long expiration instead of no expiration to maintain JWT compatibility
        let access_exp = now + Duration::days(365 * 100);
        let access_claims = Claims {
            sub: user_id.to_string(),
            exp: access_exp.timestamp(),
            iat: now.timestamp(),
            token_type: "access".to_string(),
        };

        let access_token = encode(
            &Header::default(),
            &access_claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;

        // Refresh token (also permanent - 100 years)
        let refresh_exp = now + Duration::days(365 * 100);
        let refresh_claims = Claims {
            sub: user_id.to_string(),
            exp: refresh_exp.timestamp(),
            iat: now.timestamp(),
            token_type: "refresh".to_string(),
        };

        let refresh_token = encode(
            &Header::default(),
            &refresh_claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;

        Ok(AuthTokens {
            access_token,
            refresh_token,
            // Set expires_in to 0 to indicate no expiration
            expires_in: 0,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String, // user_id
    exp: i64,    // expiration time
    iat: i64,    // issued at
    token_type: String,
}

#[derive(Debug, Clone)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}
