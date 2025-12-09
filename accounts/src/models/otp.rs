use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Otp {
    pub id: Uuid,
    pub email: String,
    pub code: String,
    pub expires_at: DateTime<Utc>,
    pub used: bool,
    pub created_at: DateTime<Utc>,
}

impl Otp {
    const CODE_LENGTH: usize = 6;
    const EXPIRY_MINUTES: i64 = 10;

    fn generate_code() -> String {
        let mut rng = rand::thread_rng();
        (0..Self::CODE_LENGTH)
            .map(|_| rng.gen_range(0..10).to_string())
            .collect()
    }

    pub async fn create(pool: &PgPool, email: &str) -> Result<Self, sqlx::Error> {
        let code = Self::generate_code();
        let expires_at = Utc::now() + Duration::minutes(Self::EXPIRY_MINUTES);

        sqlx::query_as::<_, Self>(
            "INSERT INTO otps (email, code, expires_at) VALUES ($1, $2, $3) RETURNING *"
        )
        .bind(email)
        .bind(&code)
        .bind(expires_at)
        .fetch_one(pool)
        .await
    }

    pub async fn verify(pool: &PgPool, email: &str, code: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query_as::<_, Self>(
            "UPDATE otps SET used = TRUE
             WHERE email = $1 AND code = $2 AND used = FALSE AND expires_at > NOW()
             RETURNING *"
        )
        .bind(email)
        .bind(code)
        .fetch_optional(pool)
        .await?;

        Ok(result.is_some())
    }

    pub async fn cleanup_expired(pool: &PgPool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM otps WHERE expires_at < NOW() OR used = TRUE")
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }
}
