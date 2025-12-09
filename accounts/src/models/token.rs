use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RefreshToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    pub sub: String,  // user_id
    pub email: String,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshTokenClaims {
    pub sub: String,  // user_id
    pub jti: String,  // token_id for revocation
    pub exp: i64,
    pub iat: i64,
}

impl RefreshToken {
    const REFRESH_TOKEN_DAYS: i64 = 30;

    fn hash_token(token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub async fn create(pool: &PgPool, user_id: Uuid, secret: &str) -> Result<(Self, String), sqlx::Error> {
        let token_id = Uuid::new_v4();
        let expires_at = Utc::now() + Duration::days(Self::REFRESH_TOKEN_DAYS);
        let now = Utc::now();

        let claims = RefreshTokenClaims {
            sub: user_id.to_string(),
            jti: token_id.to_string(),
            exp: expires_at.timestamp(),
            iat: now.timestamp(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        ).expect("Failed to encode refresh token");

        let token_hash = Self::hash_token(&token);

        let record = sqlx::query_as::<_, Self>(
            "INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at)
             VALUES ($1, $2, $3, $4) RETURNING *"
        )
        .bind(token_id)
        .bind(user_id)
        .bind(&token_hash)
        .bind(expires_at)
        .fetch_one(pool)
        .await?;

        Ok((record, token))
    }

    pub async fn find_valid(pool: &PgPool, token: &str) -> Result<Option<Self>, sqlx::Error> {
        let token_hash = Self::hash_token(token);
        sqlx::query_as::<_, Self>(
            "SELECT * FROM refresh_tokens
             WHERE token_hash = $1 AND expires_at > NOW() AND revoked_at IS NULL"
        )
        .bind(token_hash)
        .fetch_optional(pool)
        .await
    }

    pub async fn revoke(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE refresh_tokens SET revoked_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn revoke_all_for_user(pool: &PgPool, user_id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL"
        )
        .bind(user_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}

pub struct TokenService;

impl TokenService {
    const ACCESS_TOKEN_MINUTES: i64 = 15;

    pub fn create_access_token(user_id: Uuid, email: &str, secret: &str) -> String {
        let now = Utc::now();
        let exp = now + Duration::minutes(Self::ACCESS_TOKEN_MINUTES);

        let claims = AccessTokenClaims {
            sub: user_id.to_string(),
            email: email.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        ).expect("Failed to encode access token")
    }

    pub fn verify_access_token(token: &str, secret: &str) -> Option<AccessTokenClaims> {
        decode::<AccessTokenClaims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .ok()
        .map(|data| data.claims)
    }

    pub fn verify_refresh_token(token: &str, secret: &str) -> Option<RefreshTokenClaims> {
        decode::<RefreshTokenClaims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .ok()
        .map(|data| data.claims)
    }
}
