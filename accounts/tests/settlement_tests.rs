use accounts::db;
use accounts::models::{
    Balance, EntryType, Fill, LedgerEntry, Order, OrderType, PlaceOrderRequest, Side, Trade,
};
use rust_decimal::Decimal;
use serial_test::serial;
use sqlx::PgPool;
use std::str::FromStr;
use uuid::Uuid;

/// Test helper to create a database pool and run migrations
async fn setup_db() -> PgPool {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/mexchange".to_string());

    let pool = db::create_pool(&database_url).await.expect("Failed to create pool");
    db::run_migrations(&pool).await.expect("Failed to run migrations");

    // Clean up test data
    sqlx::query("ALTER TABLE ledger DISABLE TRIGGER ledger_immutable")
        .execute(&pool)
        .await
        .ok();
    sqlx::query("TRUNCATE trades, orders, faucet_claims, ledger, balances CASCADE")
        .execute(&pool)
        .await
        .ok();
    sqlx::query("ALTER TABLE ledger ENABLE TRIGGER ledger_immutable")
        .execute(&pool)
        .await
        .ok();

    pool
}

/// Create a test user and return their ID
async fn create_test_user(pool: &PgPool, email: &str) -> Uuid {
    let user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, email) VALUES ($1, $2) ON CONFLICT (email) DO UPDATE SET email = $2",
    )
    .bind(user_id)
    .bind(email)
    .execute(pool)
    .await
    .expect("Failed to create test user");

    let row: (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_one(pool)
        .await
        .expect("Failed to get user ID");

    row.0
}

/// Fund a user with EUR and KCN for testing
async fn fund_user(pool: &PgPool, user_id: Uuid, eur: &str, kcn: &str) {
    LedgerEntry::append(
        pool,
        user_id,
        "EUR",
        Decimal::from_str(eur).unwrap(),
        EntryType::Deposit,
        None,
        Some("Test funding"),
    )
    .await
    .unwrap();

    LedgerEntry::append(
        pool,
        user_id,
        "KCN",
        Decimal::from_str(kcn).unwrap(),
        EntryType::Deposit,
        None,
        Some("Test funding"),
    )
    .await
    .unwrap();
}

// =============================================================================
// SETTLEMENT PRECISION TESTS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_settlement_handles_eur_precision() {
    // This test ensures that price × quantity producing > 2 decimal places
    // for EUR is handled correctly (should be rounded, not rejected)
    let pool = setup_db().await;

    let buyer_id = create_test_user(&pool, "buyer@test.com").await;
    let seller_id = create_test_user(&pool, "seller@test.com").await;

    // Fund users
    fund_user(&pool, buyer_id, "10000.00", "0").await;
    fund_user(&pool, seller_id, "0", "1000.00000000").await;

    // Create buy order - price that will cause precision issues when multiplied
    let buy_order = Order::place(
        &pool,
        buyer_id,
        PlaceOrderRequest {
            symbol: "KCN/EUR".to_string(),
            side: Side::Bid,
            order_type: OrderType::Limit,
            price: Some(Decimal::from_str("1.23").unwrap()), // 1.23 EUR
            quantity: Decimal::from_str("100.12345678").unwrap(), // High precision quantity
            max_slippage_price: None,
            quote_amount: None,
        },
    )
    .await
    .unwrap()
    .order;

    // Create sell order
    let sell_order = Order::place(
        &pool,
        seller_id,
        PlaceOrderRequest {
            symbol: "KCN/EUR".to_string(),
            side: Side::Ask,
            order_type: OrderType::Limit,
            price: Some(Decimal::from_str("1.23").unwrap()),
            quantity: Decimal::from_str("100.12345678").unwrap(),
            max_slippage_price: None,
            quote_amount: None,
        },
    )
    .await
    .unwrap()
    .order;

    // Create a fill that would produce: 1.23 × 100.12345678 = 123.15185183... EUR
    // This has more than 2 decimal places and MUST be rounded
    let fill = Fill {
        buy_order_id: buy_order.id,
        sell_order_id: sell_order.id,
        price: Decimal::from_str("1.23").unwrap(),
        quantity: Decimal::from_str("100.12345678").unwrap(),
        timestamp: chrono::Utc::now().timestamp_millis(),
    };

    // Settlement should succeed (not fail due to precision)
    let trade = Trade::settle(&pool, "KCN/EUR", &fill).await;
    assert!(trade.is_ok(), "Settlement failed: {:?}", trade.err());

    let trade = trade.unwrap();
    assert_eq!(trade.buyer_id, Some(buyer_id));
    assert_eq!(trade.seller_id, Some(seller_id));
}

#[tokio::test]
#[serial]
async fn test_settlement_with_worst_case_precision() {
    // Test with values that produce maximum decimal expansion
    let pool = setup_db().await;

    let buyer_id = create_test_user(&pool, "buyer2@test.com").await;
    let seller_id = create_test_user(&pool, "seller2@test.com").await;

    fund_user(&pool, buyer_id, "100000.00", "0").await;
    fund_user(&pool, seller_id, "0", "10000.00000000").await;

    // Price with 2 decimals × quantity with 8 decimals = up to 10 decimal places
    let buy_order = Order::place(
        &pool,
        buyer_id,
        PlaceOrderRequest {
            symbol: "KCN/EUR".to_string(),
            side: Side::Bid,
            order_type: OrderType::Limit,
            price: Some(Decimal::from_str("0.99").unwrap()),
            quantity: Decimal::from_str("123.45678901").unwrap(),
            max_slippage_price: None,
            quote_amount: None,
        },
    )
    .await
    .unwrap()
    .order;

    let sell_order = Order::place(
        &pool,
        seller_id,
        PlaceOrderRequest {
            symbol: "KCN/EUR".to_string(),
            side: Side::Ask,
            order_type: OrderType::Limit,
            price: Some(Decimal::from_str("0.99").unwrap()),
            quantity: Decimal::from_str("123.45678901").unwrap(),
            max_slippage_price: None,
            quote_amount: None,
        },
    )
    .await
    .unwrap()
    .order;

    // 0.99 × 123.45678901 = 122.22222112... EUR (many decimal places)
    let fill = Fill {
        buy_order_id: buy_order.id,
        sell_order_id: sell_order.id,
        price: Decimal::from_str("0.99").unwrap(),
        quantity: Decimal::from_str("123.45678901").unwrap(),
        timestamp: chrono::Utc::now().timestamp_millis(),
    };

    let trade = Trade::settle(&pool, "KCN/EUR", &fill).await;
    assert!(trade.is_ok(), "Settlement failed with worst-case precision: {:?}", trade.err());
}

#[tokio::test]
#[serial]
async fn test_settlement_rounds_eur_correctly() {
    // Verify that EUR amounts are rounded to 2 decimal places
    let pool = setup_db().await;

    let buyer_id = create_test_user(&pool, "buyer3@test.com").await;
    let seller_id = create_test_user(&pool, "seller3@test.com").await;

    // Give buyer exactly what they need (plus a tiny bit extra for rounding)
    fund_user(&pool, buyer_id, "200.00", "0").await;
    fund_user(&pool, seller_id, "0", "100.00000000").await;

    let buy_order = Order::place(
        &pool,
        buyer_id,
        PlaceOrderRequest {
            symbol: "KCN/EUR".to_string(),
            side: Side::Bid,
            order_type: OrderType::Limit,
            price: Some(Decimal::from_str("1.999").unwrap()), // Will round
            quantity: Decimal::from_str("50.00000000").unwrap(),
            max_slippage_price: None,
            quote_amount: None,
        },
    )
    .await
    .unwrap()
    .order;

    let sell_order = Order::place(
        &pool,
        seller_id,
        PlaceOrderRequest {
            symbol: "KCN/EUR".to_string(),
            side: Side::Ask,
            order_type: OrderType::Limit,
            price: Some(Decimal::from_str("1.999").unwrap()),
            quantity: Decimal::from_str("50.00000000").unwrap(),
            max_slippage_price: None,
            quote_amount: None,
        },
    )
    .await
    .unwrap()
    .order;

    // 1.999 × 50 = 99.95 EUR (already 2 decimals, but tests the path)
    let fill = Fill {
        buy_order_id: buy_order.id,
        sell_order_id: sell_order.id,
        price: Decimal::from_str("1.999").unwrap(),
        quantity: Decimal::from_str("50.00000000").unwrap(),
        timestamp: chrono::Utc::now().timestamp_millis(),
    };

    let trade = Trade::settle(&pool, "KCN/EUR", &fill).await;
    assert!(trade.is_ok(), "Settlement failed: {:?}", trade.err());

    // Verify seller received EUR (rounded to 2 decimals)
    let seller_eur = Balance::get_or_zero(&pool, seller_id, "EUR").await.unwrap();
    // Should be 99.95 (1.999 × 50)
    let expected_eur = Decimal::from_str("99.95").unwrap();
    assert_eq!(seller_eur, expected_eur, "Seller should receive 99.95 EUR");
}

#[tokio::test]
#[serial]
async fn test_settlement_idempotency() {
    // Ensure the same fill can't be settled twice
    let pool = setup_db().await;

    let buyer_id = create_test_user(&pool, "buyer4@test.com").await;
    let seller_id = create_test_user(&pool, "seller4@test.com").await;

    fund_user(&pool, buyer_id, "1000.00", "0").await;
    fund_user(&pool, seller_id, "0", "100.00000000").await;

    let buy_order = Order::place(
        &pool,
        buyer_id,
        PlaceOrderRequest {
            symbol: "KCN/EUR".to_string(),
            side: Side::Bid,
            order_type: OrderType::Limit,
            price: Some(Decimal::from_str("5.00").unwrap()),
            quantity: Decimal::from_str("10.00000000").unwrap(),
            max_slippage_price: None,
            quote_amount: None,
        },
    )
    .await
    .unwrap()
    .order;

    let sell_order = Order::place(
        &pool,
        seller_id,
        PlaceOrderRequest {
            symbol: "KCN/EUR".to_string(),
            side: Side::Ask,
            order_type: OrderType::Limit,
            price: Some(Decimal::from_str("5.00").unwrap()),
            quantity: Decimal::from_str("10.00000000").unwrap(),
            max_slippage_price: None,
            quote_amount: None,
        },
    )
    .await
    .unwrap()
    .order;

    let timestamp = chrono::Utc::now().timestamp_millis();
    let fill = Fill {
        buy_order_id: buy_order.id,
        sell_order_id: sell_order.id,
        price: Decimal::from_str("5.00").unwrap(),
        quantity: Decimal::from_str("10.00000000").unwrap(),
        timestamp,
    };

    // First settlement should succeed
    let trade1 = Trade::settle(&pool, "KCN/EUR", &fill).await.unwrap();

    // Second settlement with same fill should return the existing trade (idempotent)
    let trade2 = Trade::settle(&pool, "KCN/EUR", &fill).await.unwrap();

    assert_eq!(trade1.id, trade2.id, "Idempotent settlement should return same trade");

    // Buyer should only have been debited once
    let buyer_eur = Balance::get_or_zero(&pool, buyer_id, "EUR").await.unwrap();
    let expected = Decimal::from_str("950.00").unwrap(); // 1000 - 50
    assert_eq!(buyer_eur, expected, "Buyer should only be debited once");
}

#[tokio::test]
#[serial]
async fn test_settlement_partial_fill() {
    // Test that partial fills work correctly
    let pool = setup_db().await;

    let buyer_id = create_test_user(&pool, "buyer5@test.com").await;
    let seller_id = create_test_user(&pool, "seller5@test.com").await;

    fund_user(&pool, buyer_id, "1000.00", "0").await;
    fund_user(&pool, seller_id, "0", "100.00000000").await;

    // Create orders for 100 KCN
    let buy_order = Order::place(
        &pool,
        buyer_id,
        PlaceOrderRequest {
            symbol: "KCN/EUR".to_string(),
            side: Side::Bid,
            order_type: OrderType::Limit,
            price: Some(Decimal::from_str("5.00").unwrap()),
            quantity: Decimal::from_str("100.00000000").unwrap(),
            max_slippage_price: None,
            quote_amount: None,
        },
    )
    .await
    .unwrap()
    .order;

    let sell_order = Order::place(
        &pool,
        seller_id,
        PlaceOrderRequest {
            symbol: "KCN/EUR".to_string(),
            side: Side::Ask,
            order_type: OrderType::Limit,
            price: Some(Decimal::from_str("5.00").unwrap()),
            quantity: Decimal::from_str("100.00000000").unwrap(),
            max_slippage_price: None,
            quote_amount: None,
        },
    )
    .await
    .unwrap()
    .order;

    // First partial fill: 30 KCN
    let fill1 = Fill {
        buy_order_id: buy_order.id,
        sell_order_id: sell_order.id,
        price: Decimal::from_str("5.00").unwrap(),
        quantity: Decimal::from_str("30.00000000").unwrap(),
        timestamp: chrono::Utc::now().timestamp_millis(),
    };
    Trade::settle(&pool, "KCN/EUR", &fill1).await.unwrap();

    // Second partial fill: 25 KCN (with precision-challenging quantity)
    let fill2 = Fill {
        buy_order_id: buy_order.id,
        sell_order_id: sell_order.id,
        price: Decimal::from_str("5.00").unwrap(),
        quantity: Decimal::from_str("25.12345678").unwrap(),
        timestamp: chrono::Utc::now().timestamp_millis() + 1,
    };
    Trade::settle(&pool, "KCN/EUR", &fill2).await.unwrap();

    // Verify balances
    let buyer_kcn = Balance::get_or_zero(&pool, buyer_id, "KCN").await.unwrap();
    let expected_kcn = Decimal::from_str("55.12345678").unwrap(); // 30 + 25.12345678
    assert_eq!(buyer_kcn, expected_kcn, "Buyer KCN balance mismatch");
}

// =============================================================================
// EUR PRECISION VALIDATION TESTS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_eur_ledger_rejects_excessive_precision() {
    // Direct ledger append with > 2 decimal EUR should fail
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "precision_test@test.com").await;

    // Try to deposit EUR with 3 decimal places - should fail
    let result = LedgerEntry::append(
        &pool,
        user_id,
        "EUR",
        Decimal::from_str("100.123").unwrap(), // 3 decimal places
        EntryType::Deposit,
        None,
        None,
    )
    .await;

    assert!(result.is_err(), "EUR with 3 decimal places should be rejected");
}

#[tokio::test]
#[serial]
async fn test_kcn_allows_8_decimal_places() {
    // KCN should allow up to 8 decimal places
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "kcn_precision@test.com").await;

    let result = LedgerEntry::append(
        &pool,
        user_id,
        "KCN",
        Decimal::from_str("100.12345678").unwrap(), // 8 decimal places
        EntryType::Deposit,
        None,
        None,
    )
    .await;

    assert!(result.is_ok(), "KCN with 8 decimal places should be allowed");

    let balance = Balance::get_or_zero(&pool, user_id, "KCN").await.unwrap();
    assert_eq!(balance, Decimal::from_str("100.12345678").unwrap());
}
