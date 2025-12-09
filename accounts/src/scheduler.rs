use sqlx::PgPool;
use std::time::Duration;
use chrono::Utc;

/// Spawns a background task that clears trades and ledger at midnight UTC
pub fn spawn_cleanup_task(pool: PgPool) {
    tokio::spawn(async move {
        tracing::info!("Scheduler started - trades cleanup scheduled for midnight UTC");

        loop {
            let now = Utc::now();

            // Calculate duration until next midnight UTC
            let next_midnight = (now + chrono::Duration::days(1))
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();

            let duration_until_midnight = (next_midnight - now).to_std().unwrap_or(Duration::from_secs(3600));

            tracing::info!(
                "Next cleanup in {} hours {} minutes",
                duration_until_midnight.as_secs() / 3600,
                (duration_until_midnight.as_secs() % 3600) / 60
            );

            tokio::time::sleep(duration_until_midnight).await;

            // Run cleanup
            if let Err(e) = cleanup_trades(&pool).await {
                tracing::error!("Failed to cleanup trades: {}", e);
            }
        }
    });
}

async fn cleanup_trades(pool: &PgPool) -> Result<(), sqlx::Error> {
    tracing::info!("Starting scheduled trades cleanup...");

    // Clear trades table
    sqlx::query("TRUNCATE TABLE trades CASCADE")
        .execute(pool)
        .await?;

    // Clear ledger table
    sqlx::query("TRUNCATE TABLE ledger CASCADE")
        .execute(pool)
        .await?;

    tracing::info!(
        "Cleanup complete - trades and ledger tables cleared at {}",
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    );

    Ok(())
}
