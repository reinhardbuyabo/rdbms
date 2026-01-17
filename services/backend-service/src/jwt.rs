use crate::models::Claims;
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};

pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl JwtService {
    pub fn new(secret: &str) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_ref()),
            decoding_key: DecodingKey::from_secret(secret.as_ref()),
        }
    }

    pub fn generate_token(&self, user_id: &str, email: &str, ttl_seconds: u64) -> Result<String> {
        let now = Utc::now();
        let exp = now + Duration::seconds(ttl_seconds as i64);

        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            exp: exp.timestamp() as usize,
            iat: now.timestamp() as usize,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .context("Failed to generate JWT token")
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims> {
        let validation = Validation::new(Algorithm::HS256);
        let token_data = decode::<Claims>(token, &self.decoding_key, &validation)
            .context("Failed to verify JWT token")?;

        Ok(token_data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_roundtrip() -> Result<()> {
        let jwt_service = JwtService::new("test_secret_key_12345");
        let user_id = "123";
        let email = "test@example.com";
        let ttl = 3600;

        let token = jwt_service.generate_token(user_id, email, ttl)?;
        let claims = jwt_service.verify_token(&token)?;

        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.email, email);
        assert!(claims.exp > claims.iat);

        Ok(())
    }

    #[test]
    fn test_invalid_token() {
        let jwt_service = JwtService::new("test_secret_key_12345");
        let invalid_token = "invalid.jwt.token";

        assert!(jwt_service.verify_token(invalid_token).is_err());
    }
}
