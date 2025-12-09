use accounts::db;
use accounts::models::{Balance, EntryType, LedgerEntry};
use rust_decimal::Decimal;
use serial_test::serial;
use sqlx::PgPool;
use std::str::FromStr;
use uuid::Uuid;

/// Test helper to create a database pool and run migrations
async fn setup_db() -> PgPool {
    // Use TEST_DATABASE_URL if set, otherwise fall back to DATABASE_URL, otherwise default
    let database_url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/mexchange".to_string());

    let pool = db::create_pool(&database_url).await.expect("Failed to create pool");
    db::run_migrations(&pool).await.expect("Failed to run migrations");

    // Clean up test data - use TRUNCATE with CASCADE to handle foreign keys
    // Also disable the ledger immutability trigger temporarily
    sqlx::query("ALTER TABLE ledger DISABLE TRIGGER ledger_immutable").execute(&pool).await.ok();
    sqlx::query("TRUNCATE trades, orders, faucet_claims, ledger, balances CASCADE").execute(&pool).await.ok();
    sqlx::query("ALTER TABLE ledger ENABLE TRIGGER ledger_immutable").execute(&pool).await.ok();

    pool
}

/// Create a test user and return their ID
async fn create_test_user(pool: &PgPool, email: &str) -> Uuid {
    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2) ON CONFLICT (email) DO UPDATE SET email = $2 RETURNING id")
        .bind(user_id)
        .bind(email)
        .execute(pool)
        .await
        .expect("Failed to create test user");

    // Get actual user ID (in case of conflict)
    let row: (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_one(pool)
        .await
        .expect("Failed to get user ID");

    row.0
}

// =============================================================================
// BASIC DEPOSIT TESTS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_deposit_creates_ledger_entry() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "deposit_test@test.com").await;

    let entry = LedgerEntry::append(
        &pool,
        user_id,
        "EUR",
        Decimal::from_str("100.00").unwrap(),
        EntryType::Deposit,
        None,
        Some("Test deposit"),
    )
    .await
    .expect("Deposit should succeed");

    assert_eq!(entry.user_id, user_id);
    assert_eq!(entry.asset, "EUR");
    assert_eq!(entry.amount, Decimal::from_str("100.00").unwrap());
    assert_eq!(entry.balance_after, Decimal::from_str("100.00").unwrap());
    assert_eq!(entry.entry_type, "deposit");
    assert_eq!(entry.description, Some("Test deposit".to_string()));
}

#[tokio::test]
#[serial]
async fn test_deposit_updates_cached_balance() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "cached_balance_test@test.com").await;

    LedgerEntry::append(
        &pool,
        user_id,
        "EUR",
        Decimal::from_str("250.50").unwrap(),
        EntryType::Deposit,
        None,
        None,
    )
    .await
    .expect("Deposit should succeed");

    let cached_balance = Balance::get_or_zero(&pool, user_id, "EUR")
        .await
        .expect("Should get cached balance");

    assert_eq!(cached_balance, Decimal::from_str("250.50").unwrap());
}

#[tokio::test]
#[serial]
async fn test_multiple_deposits_accumulate() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "multi_deposit@test.com").await;

    // First deposit
    let entry1 = LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from_str("100.00").unwrap(),
        EntryType::Deposit, None, None,
    ).await.unwrap();
    assert_eq!(entry1.balance_after, Decimal::from_str("100.00").unwrap());

    // Second deposit
    let entry2 = LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from_str("50.00").unwrap(),
        EntryType::Deposit, None, None,
    ).await.unwrap();
    assert_eq!(entry2.balance_after, Decimal::from_str("150.00").unwrap());

    // Third deposit
    let entry3 = LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from_str("25.25").unwrap(),
        EntryType::Deposit, None, None,
    ).await.unwrap();
    assert_eq!(entry3.balance_after, Decimal::from_str("175.25").unwrap());

    // Verify cached balance
    let cached = Balance::get_or_zero(&pool, user_id, "EUR").await.unwrap();
    assert_eq!(cached, Decimal::from_str("175.25").unwrap());
}

// =============================================================================
// WITHDRAWAL TESTS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_withdrawal_reduces_balance() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "withdrawal_test@test.com").await;

    // Deposit first
    LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from_str("100.00").unwrap(),
        EntryType::Deposit, None, None,
    ).await.unwrap();

    // Withdraw
    let entry = LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from_str("-30.00").unwrap(),
        EntryType::Withdrawal, None, Some("Test withdrawal"),
    ).await.expect("Withdrawal should succeed");

    assert_eq!(entry.amount, Decimal::from_str("-30.00").unwrap());
    assert_eq!(entry.balance_after, Decimal::from_str("70.00").unwrap());
    assert_eq!(entry.entry_type, "withdrawal");

    // Verify cached balance
    let cached = Balance::get_or_zero(&pool, user_id, "EUR").await.unwrap();
    assert_eq!(cached, Decimal::from_str("70.00").unwrap());
}

#[tokio::test]
#[serial]
async fn test_withdrawal_exact_balance() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "exact_withdrawal@test.com").await;

    // Deposit
    LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from_str("100.00").unwrap(),
        EntryType::Deposit, None, None,
    ).await.unwrap();

    // Withdraw exact amount
    let entry = LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from_str("-100.00").unwrap(),
        EntryType::Withdrawal, None, None,
    ).await.expect("Exact withdrawal should succeed");

    assert_eq!(entry.balance_after, Decimal::ZERO);

    let cached = Balance::get_or_zero(&pool, user_id, "EUR").await.unwrap();
    assert_eq!(cached, Decimal::ZERO);
}

#[tokio::test]
#[serial]
async fn test_withdrawal_insufficient_funds_rejected() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "insufficient_funds@test.com").await;

    // Deposit
    LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from_str("50.00").unwrap(),
        EntryType::Deposit, None, None,
    ).await.unwrap();

    // Try to withdraw more than available
    let result = LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from_str("-100.00").unwrap(),
        EntryType::Withdrawal, None, None,
    ).await;

    assert!(result.is_err(), "Withdrawal exceeding balance should fail");

    // Balance should remain unchanged
    let cached = Balance::get_or_zero(&pool, user_id, "EUR").await.unwrap();
    assert_eq!(cached, Decimal::from_str("50.00").unwrap());
}

#[tokio::test]
#[serial]
async fn test_withdrawal_from_zero_balance_rejected() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "zero_balance_withdrawal@test.com").await;

    // Try to withdraw without any deposits
    let result = LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from_str("-10.00").unwrap(),
        EntryType::Withdrawal, None, None,
    ).await;

    assert!(result.is_err(), "Withdrawal from zero balance should fail");
}

// =============================================================================
// MULTI-ASSET TESTS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_multiple_assets_independent() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "multi_asset@test.com").await;

    // Deposit EUR
    LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from_str("100.00").unwrap(),
        EntryType::Deposit, None, None,
    ).await.unwrap();

    // Deposit BTC
    LedgerEntry::append(
        &pool, user_id, "BTC",
        Decimal::from_str("0.5").unwrap(),
        EntryType::Deposit, None, None,
    ).await.unwrap();

    // Deposit more EUR
    LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from_str("50.00").unwrap(),
        EntryType::Deposit, None, None,
    ).await.unwrap();

    // Verify EUR balance
    let eur_balance = Balance::get_or_zero(&pool, user_id, "EUR").await.unwrap();
    assert_eq!(eur_balance, Decimal::from_str("150.00").unwrap());

    // Verify BTC balance (should be unaffected)
    let btc_balance = Balance::get_or_zero(&pool, user_id, "BTC").await.unwrap();
    assert_eq!(btc_balance, Decimal::from_str("0.5").unwrap());

    // Withdraw from EUR shouldn't affect BTC
    LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from_str("-25.00").unwrap(),
        EntryType::Withdrawal, None, None,
    ).await.unwrap();

    let eur_after = Balance::get_or_zero(&pool, user_id, "EUR").await.unwrap();
    let btc_after = Balance::get_or_zero(&pool, user_id, "BTC").await.unwrap();

    assert_eq!(eur_after, Decimal::from_str("125.00").unwrap());
    assert_eq!(btc_after, Decimal::from_str("0.5").unwrap());
}

// =============================================================================
// MULTI-USER TESTS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_multiple_users_independent() {
    let pool = setup_db().await;
    let alice = create_test_user(&pool, "alice@test.com").await;
    let bob = create_test_user(&pool, "bob@test.com").await;

    // Alice deposits
    LedgerEntry::append(
        &pool, alice, "EUR",
        Decimal::from_str("1000.00").unwrap(),
        EntryType::Deposit, None, None,
    ).await.unwrap();

    // Bob deposits
    LedgerEntry::append(
        &pool, bob, "EUR",
        Decimal::from_str("500.00").unwrap(),
        EntryType::Deposit, None, None,
    ).await.unwrap();

    // Alice withdraws
    LedgerEntry::append(
        &pool, alice, "EUR",
        Decimal::from_str("-200.00").unwrap(),
        EntryType::Withdrawal, None, None,
    ).await.unwrap();

    // Verify balances are independent
    let alice_balance = Balance::get_or_zero(&pool, alice, "EUR").await.unwrap();
    let bob_balance = Balance::get_or_zero(&pool, bob, "EUR").await.unwrap();

    assert_eq!(alice_balance, Decimal::from_str("800.00").unwrap());
    assert_eq!(bob_balance, Decimal::from_str("500.00").unwrap());
}

// =============================================================================
// RECONCILIATION TESTS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_derived_balance_matches_cached() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "reconciliation@test.com").await;

    // Multiple operations
    LedgerEntry::append(&pool, user_id, "EUR", Decimal::from_str("100.00").unwrap(), EntryType::Deposit, None, None).await.unwrap();
    LedgerEntry::append(&pool, user_id, "EUR", Decimal::from_str("50.00").unwrap(), EntryType::Deposit, None, None).await.unwrap();
    LedgerEntry::append(&pool, user_id, "EUR", Decimal::from_str("-30.00").unwrap(), EntryType::Withdrawal, None, None).await.unwrap();
    LedgerEntry::append(&pool, user_id, "EUR", Decimal::from_str("25.00").unwrap(), EntryType::Deposit, None, None).await.unwrap();
    LedgerEntry::append(&pool, user_id, "EUR", Decimal::from_str("-45.00").unwrap(), EntryType::Withdrawal, None, None).await.unwrap();

    // Derived balance (SUM of ledger)
    let derived = LedgerEntry::derive_balance(&pool, user_id, "EUR").await.unwrap();

    // Cached balance
    let cached = Balance::get_or_zero(&pool, user_id, "EUR").await.unwrap();

    assert_eq!(derived, cached, "Derived and cached balances must match");
    assert_eq!(derived, Decimal::from_str("100.00").unwrap()); // 100+50-30+25-45 = 100

    // Reconciliation check
    let reconciled = LedgerEntry::reconcile(&pool, user_id, "EUR").await.unwrap();
    assert!(reconciled, "Reconciliation should pass");
}

#[tokio::test]
#[serial]
async fn test_reconcile_nonexistent_asset() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "reconcile_empty@test.com").await;

    // No deposits for this asset
    let reconciled = LedgerEntry::reconcile(&pool, user_id, "XYZ").await.unwrap();
    assert!(reconciled, "Empty account should reconcile (0 == 0)");
}

// =============================================================================
// LEDGER HISTORY TESTS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_ledger_history_order() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "history_order@test.com").await;

    // Create multiple entries
    LedgerEntry::append(&pool, user_id, "EUR", Decimal::from_str("100.00").unwrap(), EntryType::Deposit, None, Some("First")).await.unwrap();
    LedgerEntry::append(&pool, user_id, "EUR", Decimal::from_str("50.00").unwrap(), EntryType::Deposit, None, Some("Second")).await.unwrap();
    LedgerEntry::append(&pool, user_id, "EUR", Decimal::from_str("-25.00").unwrap(), EntryType::Withdrawal, None, Some("Third")).await.unwrap();

    let history = LedgerEntry::get_history(&pool, user_id, "EUR", 10).await.unwrap();

    assert_eq!(history.len(), 3);
    // Should be in reverse chronological order (newest first)
    assert_eq!(history[0].description, Some("Third".to_string()));
    assert_eq!(history[1].description, Some("Second".to_string()));
    assert_eq!(history[2].description, Some("First".to_string()));
}

#[tokio::test]
#[serial]
async fn test_ledger_history_limit() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "history_limit@test.com").await;

    // Create 5 entries
    for i in 1..=5 {
        LedgerEntry::append(
            &pool, user_id, "EUR",
            Decimal::from(i),
            EntryType::Deposit, None, None,
        ).await.unwrap();
    }

    // Request only 3
    let history = LedgerEntry::get_history(&pool, user_id, "EUR", 3).await.unwrap();
    assert_eq!(history.len(), 3);

    // Should be the 3 most recent
    assert_eq!(history[0].amount, Decimal::from(5));
    assert_eq!(history[1].amount, Decimal::from(4));
    assert_eq!(history[2].amount, Decimal::from(3));
}

// =============================================================================
// REFERENCE ID TESTS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_ledger_with_reference_id() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "reference@test.com").await;
    let order_id = Uuid::new_v4();

    let entry = LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from_str("100.00").unwrap(),
        EntryType::Trade,
        Some(order_id),
        Some("Trade fill"),
    ).await.unwrap();

    assert_eq!(entry.reference_id, Some(order_id));
    assert_eq!(entry.entry_type, "trade");
}

// =============================================================================
// PRECISION TESTS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_decimal_precision_preserved() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "precision@test.com").await;

    // Use high precision decimals
    let precise_amount = Decimal::from_str("0.00000001").unwrap(); // 1 satoshi

    LedgerEntry::append(
        &pool, user_id, "BTC",
        precise_amount,
        EntryType::Deposit, None, None,
    ).await.unwrap();

    let balance = Balance::get_or_zero(&pool, user_id, "BTC").await.unwrap();
    assert_eq!(balance, precise_amount);
}

#[tokio::test]
#[serial]
async fn test_large_amounts() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "large_amount@test.com").await;

    // 1 billion EUR
    let large = Decimal::from_str("1000000000.00").unwrap();

    LedgerEntry::append(
        &pool, user_id, "EUR",
        large,
        EntryType::Deposit, None, None,
    ).await.unwrap();

    let balance = Balance::get_or_zero(&pool, user_id, "EUR").await.unwrap();
    assert_eq!(balance, large);
}

// =============================================================================
// ENTRY TYPE TESTS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_all_entry_types() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "entry_types@test.com").await;

    // Start with deposit
    LedgerEntry::append(&pool, user_id, "EUR", Decimal::from(1000), EntryType::Deposit, None, None).await.unwrap();

    // Test each entry type
    let types = [
        (EntryType::Withdrawal, Decimal::from(-100), "withdrawal"),
        (EntryType::Trade, Decimal::from(-50), "trade"),
        (EntryType::Fee, Decimal::from(-5), "fee"),
    ];

    for (entry_type, amount, expected_str) in types {
        let entry = LedgerEntry::append(&pool, user_id, "EUR", amount, entry_type, None, None).await.unwrap();
        assert_eq!(entry.entry_type, expected_str);
    }

    // Final balance should be 1000 - 100 - 50 - 5 = 845
    let balance = Balance::get_or_zero(&pool, user_id, "EUR").await.unwrap();
    assert_eq!(balance, Decimal::from(845));
}

// =============================================================================
// EDGE CASE TESTS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_zero_amount_deposit() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "zero_deposit@test.com").await;

    // Zero amount deposit should work (though not useful)
    let entry = LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::ZERO,
        EntryType::Deposit, None, None,
    ).await.unwrap();

    assert_eq!(entry.amount, Decimal::ZERO);
    assert_eq!(entry.balance_after, Decimal::ZERO);
}

#[tokio::test]
#[serial]
async fn test_balance_for_user_returns_all_assets() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "all_assets@test.com").await;

    // Deposit multiple assets
    LedgerEntry::append(&pool, user_id, "EUR", Decimal::from(100), EntryType::Deposit, None, None).await.unwrap();
    LedgerEntry::append(&pool, user_id, "BTC", Decimal::from_str("0.5").unwrap(), EntryType::Deposit, None, None).await.unwrap();
    LedgerEntry::append(&pool, user_id, "ETH", Decimal::from(2), EntryType::Deposit, None, None).await.unwrap();

    let balances = Balance::get_for_user(&pool, user_id).await.unwrap();

    assert_eq!(balances.len(), 3);
    // Should be ordered by asset name
    assert_eq!(balances[0].asset, "BTC");
    assert_eq!(balances[1].asset, "ETH");
    assert_eq!(balances[2].asset, "EUR");
}

#[tokio::test]
#[serial]
async fn test_nonexistent_user_balance() {
    let pool = setup_db().await;
    let fake_user_id = Uuid::new_v4();

    let balance = Balance::get_or_zero(&pool, fake_user_id, "EUR").await.unwrap();
    assert_eq!(balance, Decimal::ZERO);
}

// =============================================================================
// ATOMICITY / TRANSACTION TESTS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_failed_withdrawal_doesnt_create_entry() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "atomic_withdrawal@test.com").await;

    // Deposit 50
    LedgerEntry::append(&pool, user_id, "EUR", Decimal::from(50), EntryType::Deposit, None, None).await.unwrap();

    // Try to withdraw 100 (should fail)
    let result = LedgerEntry::append(&pool, user_id, "EUR", Decimal::from(-100), EntryType::Withdrawal, None, None).await;
    assert!(result.is_err());

    // Check that no failed entry was created
    let history = LedgerEntry::get_history(&pool, user_id, "EUR", 10).await.unwrap();
    assert_eq!(history.len(), 1, "Only the deposit entry should exist");
    assert_eq!(history[0].entry_type, "deposit");

    // Balance should still be 50
    let balance = Balance::get_or_zero(&pool, user_id, "EUR").await.unwrap();
    assert_eq!(balance, Decimal::from(50));
}

// =============================================================================
// RUNNING BALANCE (balance_after) CORRECTNESS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_running_balance_in_ledger() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "running_balance@test.com").await;

    // Series of operations
    let e1 = LedgerEntry::append(&pool, user_id, "EUR", Decimal::from(100), EntryType::Deposit, None, None).await.unwrap();
    let e2 = LedgerEntry::append(&pool, user_id, "EUR", Decimal::from(50), EntryType::Deposit, None, None).await.unwrap();
    let e3 = LedgerEntry::append(&pool, user_id, "EUR", Decimal::from(-30), EntryType::Withdrawal, None, None).await.unwrap();
    let e4 = LedgerEntry::append(&pool, user_id, "EUR", Decimal::from(-20), EntryType::Fee, None, None).await.unwrap();

    assert_eq!(e1.balance_after, Decimal::from(100));
    assert_eq!(e2.balance_after, Decimal::from(150));
    assert_eq!(e3.balance_after, Decimal::from(120));
    assert_eq!(e4.balance_after, Decimal::from(100));
}

// =============================================================================
// CONCURRENCY TESTS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_concurrent_deposits_maintain_consistency() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "concurrent_deposits@test.com").await;

    // Spawn 10 concurrent deposit tasks
    let mut handles = vec![];
    for i in 0..10 {
        let pool = pool.clone();
        let handle = tokio::spawn(async move {
            LedgerEntry::append(
                &pool,
                user_id,
                "EUR",
                Decimal::from(10), // Each deposits 10
                EntryType::Deposit,
                None,
                Some(&format!("Concurrent deposit {}", i)),
            )
            .await
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap().expect("Concurrent deposit should succeed");
    }

    // Final balance should be exactly 100 (10 deposits of 10 each)
    let balance = Balance::get_or_zero(&pool, user_id, "EUR").await.unwrap();
    assert_eq!(balance, Decimal::from(100));

    // Derived balance should also be 100
    let derived = LedgerEntry::derive_balance(&pool, user_id, "EUR").await.unwrap();
    assert_eq!(derived, Decimal::from(100));

    // Should have exactly 10 ledger entries
    let history = LedgerEntry::get_history(&pool, user_id, "EUR", 100).await.unwrap();
    assert_eq!(history.len(), 10);

    // Reconciliation should pass
    assert!(LedgerEntry::reconcile(&pool, user_id, "EUR").await.unwrap());
}

#[tokio::test]
#[serial]
async fn test_concurrent_withdrawals_prevent_overdraft() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "concurrent_withdrawals@test.com").await;

    // Start with 50 EUR
    LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from(50),
        EntryType::Deposit, None, None,
    ).await.unwrap();

    // Try 10 concurrent withdrawals of 10 each (total 100, but only 50 available)
    let mut handles = vec![];
    for _ in 0..10 {
        let pool = pool.clone();
        let handle = tokio::spawn(async move {
            LedgerEntry::append(
                &pool,
                user_id,
                "EUR",
                Decimal::from(-10),
                EntryType::Withdrawal,
                None,
                None,
            )
            .await
        });
        handles.push(handle);
    }

    // Count successes and failures
    let mut successes = 0;
    let mut failures = 0;
    for handle in handles {
        match handle.await.unwrap() {
            Ok(_) => successes += 1,
            Err(_) => failures += 1,
        }
    }

    // Exactly 5 should succeed (50 / 10 = 5 possible withdrawals)
    assert_eq!(successes, 5, "Exactly 5 withdrawals should succeed");
    assert_eq!(failures, 5, "Exactly 5 withdrawals should fail");

    // Balance should be exactly 0
    let balance = Balance::get_or_zero(&pool, user_id, "EUR").await.unwrap();
    assert_eq!(balance, Decimal::ZERO);

    // Reconciliation should pass
    assert!(LedgerEntry::reconcile(&pool, user_id, "EUR").await.unwrap());
}

#[tokio::test]
#[serial]
async fn test_interleaved_deposits_and_withdrawals() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "interleaved@test.com").await;

    // Start with 100 EUR
    LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from(100),
        EntryType::Deposit, None, None,
    ).await.unwrap();

    // Concurrent: 5 deposits of 20 and 5 withdrawals of 20
    let mut handles = vec![];

    for i in 0..5 {
        let pool = pool.clone();
        let handle = tokio::spawn(async move {
            LedgerEntry::append(
                &pool, user_id, "EUR",
                Decimal::from(20),
                EntryType::Deposit, None, Some(&format!("Deposit {}", i)),
            ).await
        });
        handles.push(handle);
    }

    for i in 0..5 {
        let pool = pool.clone();
        let handle = tokio::spawn(async move {
            LedgerEntry::append(
                &pool, user_id, "EUR",
                Decimal::from(-20),
                EntryType::Withdrawal, None, Some(&format!("Withdrawal {}", i)),
            ).await
        });
        handles.push(handle);
    }

    // All should succeed (deposits always work, withdrawals have enough)
    for handle in handles {
        handle.await.unwrap().expect("Operation should succeed");
    }

    // Final balance: 100 + (5*20) - (5*20) = 100
    let balance = Balance::get_or_zero(&pool, user_id, "EUR").await.unwrap();
    assert_eq!(balance, Decimal::from(100));

    // Reconciliation
    assert!(LedgerEntry::reconcile(&pool, user_id, "EUR").await.unwrap());
}

// =============================================================================
// STRESS TEST
// =============================================================================

#[tokio::test]
#[serial]
async fn test_rapid_sequential_operations() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "rapid_sequential@test.com").await;

    // 100 rapid deposits
    for i in 0..100 {
        LedgerEntry::append(
            &pool, user_id, "EUR",
            Decimal::from(1),
            EntryType::Deposit, None, Some(&format!("Op {}", i)),
        ).await.unwrap();
    }

    // Balance should be exactly 100
    let balance = Balance::get_or_zero(&pool, user_id, "EUR").await.unwrap();
    assert_eq!(balance, Decimal::from(100));

    // Derived should match
    let derived = LedgerEntry::derive_balance(&pool, user_id, "EUR").await.unwrap();
    assert_eq!(derived, Decimal::from(100));

    // 100 entries in ledger
    let history = LedgerEntry::get_history(&pool, user_id, "EUR", 200).await.unwrap();
    assert_eq!(history.len(), 100);
}

// =============================================================================
// ASSET VALIDATION TESTS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_asset_too_long_rejected() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "asset_long@test.com").await;

    let result = LedgerEntry::append(
        &pool, user_id, "VERYLONGASSET", // 13 chars, exceeds 10 limit
        Decimal::from(100),
        EntryType::Deposit, None, None,
    ).await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("1-10 characters"), "Error should mention length: {}", err);
}

#[tokio::test]
#[serial]
async fn test_asset_empty_rejected() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "asset_empty@test.com").await;

    let result = LedgerEntry::append(
        &pool, user_id, "", // empty
        Decimal::from(100),
        EntryType::Deposit, None, None,
    ).await;

    assert!(result.is_err());
}

#[tokio::test]
#[serial]
async fn test_asset_non_alphanumeric_rejected() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "asset_special@test.com").await;

    let result = LedgerEntry::append(
        &pool, user_id, "BTC-USD", // contains hyphen
        Decimal::from(100),
        EntryType::Deposit, None, None,
    ).await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("alphanumeric"), "Error should mention alphanumeric: {}", err);
}

#[tokio::test]
#[serial]
async fn test_asset_valid_boundary() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "asset_boundary@test.com").await;

    // Exactly 10 characters should work
    let result = LedgerEntry::append(
        &pool, user_id, "ABCDEFGHIJ", // exactly 10 chars
        Decimal::from(100),
        EntryType::Deposit, None, None,
    ).await;

    assert!(result.is_ok());
}

// =============================================================================
// NEGATIVE AMOUNT WITH WRONG ENTRY TYPE TESTS
// =============================================================================

#[tokio::test]
#[serial]
async fn test_negative_deposit_treated_as_withdrawal() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "neg_deposit@test.com").await;

    // First add some funds
    LedgerEntry::append(&pool, user_id, "EUR", Decimal::from(100), EntryType::Deposit, None, None).await.unwrap();

    // Try to "deposit" a negative amount - this should work but reduce balance
    // (entry type is just metadata, the amount determines the effect)
    let entry = LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from(-50),
        EntryType::Deposit, // Misleading entry type
        None, None,
    ).await.unwrap();

    // Balance should still be correct (100 - 50 = 50)
    assert_eq!(entry.balance_after, Decimal::from(50));

    // The entry type is recorded as-is (it's metadata)
    assert_eq!(entry.entry_type, "deposit");
}

#[tokio::test]
#[serial]
async fn test_negative_deposit_fails_on_insufficient_balance() {
    let pool = setup_db().await;
    let user_id = create_test_user(&pool, "neg_deposit_fail@test.com").await;

    // No funds
    let result = LedgerEntry::append(
        &pool, user_id, "EUR",
        Decimal::from(-50),
        EntryType::Deposit, // Entry type doesn't matter
        None, None,
    ).await;

    // Should fail due to insufficient balance (regardless of entry type)
    assert!(result.is_err());
}
