use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Run migrations in order
    let migrations = [
        include_str!("../../migrations/001_create_users.sql"),
        include_str!("../../migrations/002_create_ledger.sql"),
        include_str!("../../migrations/003_add_constraints.sql"),
        include_str!("../../migrations/004_create_orders_trades.sql"),
        include_str!("../../migrations/005_nullable_trade_ids.sql"),
        include_str!("../../migrations/006_exchange_fill_id_index.sql"),
        // 007 was for removing exchange_order_id, now consolidated into 004
    ];

    for migration in migrations {
        sqlx::raw_sql(migration).execute(pool).await?;
    }

    tracing::info!("Database migrations completed");
    Ok(())
}
