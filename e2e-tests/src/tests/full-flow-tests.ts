/**
 * Full Flow E2E Tests
 * Tests the complete user journey with database verification
 */

import { AccountsClient } from '../accounts-client.js';
import { DbClient } from '../db-client.js';
import { TestSuite, assert, assertNotNull, assertEqual, assertGreater, sleep } from '../test-framework.js';

const TEST_EMAIL_PREFIX = 'e2e-test-';
const TEST_EMAIL_DOMAIN = '@test.mexchange.local';

function generateTestEmail(): string {
  return `${TEST_EMAIL_PREFIX}${Date.now()}-${Math.random().toString(36).substring(7)}${TEST_EMAIL_DOMAIN}`;
}

export function createFullFlowTests(): TestSuite {
  const suite = new TestSuite('Full Flow E2E Tests');
  let api: AccountsClient;
  let db: DbClient;
  let testEmail: string;

  suite.beforeAll(async () => {
    api = new AccountsClient();
    db = new DbClient();
    await db.connect();
  });

  suite.afterAll(async () => {
    await db.disconnect();
  });

  suite.beforeEach(async () => {
    // Generate unique email for each test
    testEmail = generateTestEmail();
    api.logout();
  });

  suite.afterEach(async () => {
    // Clean up test user
    if (testEmail) {
      await db.deleteTestUser(testEmail);
    }
  });

  suite.test('user signup creates user in database', async () => {
    // Login (creates user if doesn't exist in dev mode)
    const auth = await api.login(testEmail);
    assertNotNull(auth.access_token, 'Should receive access token');
    assertNotNull(auth.user.id, 'Should receive user ID');

    // Verify user exists in database
    const dbUser = await db.getUserById(auth.user.id);
    assertNotNull(dbUser, 'User should exist in database');
    assertEqual(dbUser!.email, testEmail.toLowerCase(), 'Email should match');
  });

  suite.test('deposit creates ledger entry and updates balance', async () => {
    // Setup: Create user
    const auth = await api.login(testEmail);
    const userId = auth.user.id;

    // 1. Check initial balance (should be empty or zero)
    const initialBalance = await db.getBalance(userId, 'EUR');
    const initialAvailable = initialBalance ? parseFloat(initialBalance.available) : 0;

    // 2. Make deposit via API
    const depositAmount = '1000.00';
    const depositResult = await api.deposit(depositAmount);
    assert(depositResult.success, 'Deposit should succeed');

    // 3. Verify balance updated in database
    const newBalance = await db.getBalance(userId, 'EUR');
    assertNotNull(newBalance, 'Balance record should exist');
    assertEqual(
      parseFloat(newBalance!.available),
      initialAvailable + parseFloat(depositAmount),
      'Available balance should increase by deposit amount'
    );

    // 4. Verify ledger entry was created
    const ledgerEntries = await db.getLedgerEntriesForUser(userId, 'EUR');
    assert(ledgerEntries.length > 0, 'Should have ledger entries');

    const depositEntry = ledgerEntries.find(e => e.entry_type === 'deposit');
    assertNotNull(depositEntry, 'Should have deposit ledger entry');
    assertEqual(parseFloat(depositEntry!.amount), parseFloat(depositAmount), 'Ledger amount should match');
  });

  suite.test('faucet claim creates ledger entry for KCN', async () => {
    // Setup: Create user
    const auth = await api.login(testEmail);
    const userId = auth.user.id;

    // 1. Claim faucet
    const faucetResult = await api.claimFaucet();
    assert(faucetResult.success, 'Faucet claim should succeed');
    assertEqual(faucetResult.amount, '100', 'Should receive 100 KCN');

    // 2. Verify balance in database
    const balance = await db.getBalance(userId, 'KCN');
    assertNotNull(balance, 'KCN balance should exist');
    assertEqual(parseFloat(balance!.available), 100, 'Should have 100 KCN available');

    // 3. Verify ledger entry
    const ledgerEntries = await db.getLedgerEntriesForUser(userId, 'KCN');
    assert(ledgerEntries.length > 0, 'Should have KCN ledger entries');

    const faucetEntry = ledgerEntries.find(e => e.description?.includes('Faucet'));
    assertNotNull(faucetEntry, 'Should have faucet ledger entry');
  });

  suite.test('placing limit sell order locks funds in database', async () => {
    // Setup: Create user with KCN
    const auth = await api.login(testEmail);
    const userId = auth.user.id;
    await api.claimFaucet(); // Get 100 KCN

    // 1. Check initial balance
    const initialBalance = await db.getBalance(userId, 'KCN');
    assertNotNull(initialBalance, 'Should have KCN balance');
    const initialAvailable = parseFloat(initialBalance!.available);
    const initialLocked = parseFloat(initialBalance!.locked);

    // 2. Place limit sell order
    const orderQty = '10';
    const orderPrice = '50.00';
    const orderResult = await api.placeOrder({
      symbol: 'KCN/EUR',
      side: 'ask',
      order_type: 'limit',
      quantity: orderQty,
      price: orderPrice,
    });

    assertNotNull(orderResult.order.id, 'Should receive order ID');
    assertEqual(orderResult.locked_asset, 'KCN', 'Should lock KCN for sell order');
    assertEqual(orderResult.locked_amount, orderQty, 'Locked amount should match quantity');

    // 3. Verify order exists in database
    const dbOrder = await db.getOrderById(orderResult.order.id);
    assertNotNull(dbOrder, 'Order should exist in database');
    assertEqual(dbOrder!.user_id, userId, 'Order should belong to user');
    assertEqual(dbOrder!.symbol, 'KCN/EUR', 'Symbol should match');
    assertEqual(dbOrder!.side, 'ask', 'Side should be ask');
    assertEqual(dbOrder!.status, 'pending', 'Status should be pending');
    assertEqual(parseFloat(dbOrder!.quantity), parseFloat(orderQty), 'Quantity should match');

    // 4. Verify funds are locked in database (available decreases)
    const newBalance = await db.getBalance(userId, 'KCN');
    assertNotNull(newBalance, 'Balance should exist');
    assertEqual(
      parseFloat(newBalance!.available),
      initialAvailable - parseFloat(orderQty),
      'Available should decrease by locked amount'
    );

    // 5. Verify lock ledger entry
    const ledgerEntries = await db.getLedgerEntriesForOrder(orderResult.order.id);
    const lockEntry = ledgerEntries.find(e => e.entry_type === 'lock');
    assertNotNull(lockEntry, 'Should have lock ledger entry');
    assertEqual(parseFloat(lockEntry!.amount), -parseFloat(orderQty), 'Lock amount should be negative');
  });

  suite.test('placing limit buy order locks EUR funds', async () => {
    // Setup: Create user with EUR
    const auth = await api.login(testEmail);
    const userId = auth.user.id;
    await api.deposit('1000.00');

    // 1. Check initial EUR balance
    const initialBalance = await db.getBalance(userId, 'EUR');
    assertNotNull(initialBalance, 'Should have EUR balance');
    const initialAvailable = parseFloat(initialBalance!.available);

    // 2. Place limit buy order
    const orderQty = '5';
    const orderPrice = '45.00';
    const expectedLock = parseFloat(orderQty) * parseFloat(orderPrice); // 225 EUR

    const orderResult = await api.placeOrder({
      symbol: 'KCN/EUR',
      side: 'bid',
      order_type: 'limit',
      quantity: orderQty,
      price: orderPrice,
    });

    assertEqual(orderResult.locked_asset, 'EUR', 'Should lock EUR for buy order');
    assertEqual(parseFloat(orderResult.locked_amount), expectedLock, 'Locked amount should be qty * price');

    // 3. Verify EUR is locked in database (available decreases)
    const newBalance = await db.getBalance(userId, 'EUR');
    assertEqual(
      parseFloat(newBalance!.available),
      initialAvailable - expectedLock,
      'EUR available should decrease by order total'
    );
  });

  suite.test('canceling order unlocks funds and updates database', async () => {
    // Setup: Create user with KCN
    const auth = await api.login(testEmail);
    const userId = auth.user.id;
    await api.claimFaucet();

    // 1. Place order
    const orderResult = await api.placeOrder({
      symbol: 'KCN/EUR',
      side: 'ask',
      order_type: 'limit',
      quantity: '10',
      price: '999.00', // High price so it won't fill
    });

    // 2. Verify funds locked (available decreased)
    const lockedBalance = await db.getBalance(userId, 'KCN');
    assertEqual(parseFloat(lockedBalance!.available), 90, 'Should have 90 KCN available (10 locked)');

    // 3. Cancel order
    const cancelResult = await api.cancelOrder(orderResult.order.id);
    assertEqual(cancelResult.order.status, 'cancelled', 'Order should be cancelled');

    // 4. Verify order status in database
    const dbOrder = await db.getOrderById(orderResult.order.id);
    assertEqual(dbOrder!.status, 'cancelled', 'DB order status should be cancelled');

    // 5. Verify funds unlocked in database (available restored)
    const unlockedBalance = await db.getBalance(userId, 'KCN');
    assertEqual(parseFloat(unlockedBalance!.available), 100, 'Should have 100 KCN available after cancel');

    // 6. Verify unlock ledger entry
    const ledgerEntries = await db.getLedgerEntriesForOrder(orderResult.order.id);
    const unlockEntry = ledgerEntries.find(e => e.entry_type === 'unlock');
    assertNotNull(unlockEntry, 'Should have unlock ledger entry');
  });

  suite.test('market order against existing liquidity creates trade in database', async () => {
    // This test creates two users: one provides liquidity, one takes it
    const makerEmail = generateTestEmail();
    const takerEmail = generateTestEmail();

    const makerApi = new AccountsClient();
    const takerApi = new AccountsClient();

    try {
      // Setup maker (provides sell liquidity)
      const makerAuth = await makerApi.login(makerEmail);
      await makerApi.claimFaucet(); // Get 100 KCN

      // Problem: Bot liquidity exists around ~42 EUR. A bid at high price matches bot asks first.
      // Solution: Place BOTH orders ABOVE market so they can only match each other.
      // Use a price high enough that no bot would have orders there.
      const testPrice = '500.00';

      // Setup taker FIRST with enough EUR
      const takerAuth = await takerApi.login(takerEmail);
      await takerApi.deposit('3000.00'); // Get EUR to buy with enough for 5 * 500 = 2500 EUR

      // Taker places buy order at high price FIRST
      // This sits on the bid side of the book at a price above bot asks
      const takerOrder = await takerApi.placeOrderWithMatching({
        symbol: 'KCN/EUR',
        side: 'bid',
        order_type: 'limit',
        quantity: '5',
        price: testPrice,
      });

      // Wait for order to reach matching engine
      await sleep(500);

      // Maker places sell at same price - should match the taker's bid immediately
      const makerOrder = await makerApi.placeOrderWithMatching({
        symbol: 'KCN/EUR',
        side: 'ask',
        order_type: 'limit',
        quantity: '5',
        price: testPrice,
      });

      // Wait for settlement
      await sleep(3000);

      // Verify trade exists in database
      const makerTrades = await db.getTradesForUser(makerAuth.user.id);
      const takerTrades = await db.getTradesForUser(takerAuth.user.id);

      assertGreater(makerTrades.length, 0, 'Maker should have trades');
      assertGreater(takerTrades.length, 0, 'Taker should have trades');

      // Verify trade details
      const trade = makerTrades[0];
      assertEqual(trade.seller_id, makerAuth.user.id, 'Maker should be seller');
      assertEqual(trade.buyer_id, takerAuth.user.id, 'Taker should be buyer');
      assertEqual(parseFloat(trade.price), 500, 'Price should be test price');
      assertEqual(parseFloat(trade.quantity), 5, 'Quantity should be 5');

      // Verify maker received EUR
      const makerEurBalance = await db.getBalance(makerAuth.user.id, 'EUR');
      assertNotNull(makerEurBalance, 'Maker should have EUR balance');
      assertGreater(parseFloat(makerEurBalance!.available), 0, 'Maker should have received EUR');

      // Verify taker received KCN
      const takerKcnBalance = await db.getBalance(takerAuth.user.id, 'KCN');
      assertNotNull(takerKcnBalance, 'Taker should have KCN balance');
      assertEqual(parseFloat(takerKcnBalance!.available), 5, 'Taker should have 5 KCN');

      // Verify maker KCN is gone
      const makerKcnBalance = await db.getBalance(makerAuth.user.id, 'KCN');
      assertEqual(parseFloat(makerKcnBalance!.available), 95, 'Maker should have 95 KCN remaining');
      assertEqual(parseFloat(makerKcnBalance!.locked), 0, 'Maker should have 0 KCN locked');

      // Verify orders are filled in database
      const dbMakerOrder = await db.getOrderById(makerOrder.order.id);
      assertEqual(dbMakerOrder!.status, 'filled', 'Maker order should be filled');
      assertEqual(parseFloat(dbMakerOrder!.filled_quantity), 5, 'Maker order filled qty should be 5');

    } finally {
      // Cleanup
      await db.deleteTestUser(makerEmail);
      await db.deleteTestUser(takerEmail);
    }
  });

  suite.test('settlement creates proper ledger entries for both parties', async () => {
    const makerEmail = generateTestEmail();
    const takerEmail = generateTestEmail();

    const makerApi = new AccountsClient();
    const takerApi = new AccountsClient();

    try {
      // Setup maker
      const makerAuth = await makerApi.login(makerEmail);
      await makerApi.claimFaucet();

      // Setup taker
      const takerAuth = await takerApi.login(takerEmail);
      await takerApi.deposit('1000.00');

      // Use high price to avoid bot liquidity. Place buy FIRST, then sell to match.
      const testPrice = '450.00';

      // Taker places buy order FIRST at high price
      await takerApi.placeOrderWithMatching({
        symbol: 'KCN/EUR',
        side: 'bid',
        order_type: 'limit',
        quantity: '2',
        price: testPrice,
      });

      await sleep(500);

      // Maker places sell at same price - should match the taker's bid
      const makerOrder = await makerApi.placeOrderWithMatching({
        symbol: 'KCN/EUR',
        side: 'ask',
        order_type: 'limit',
        quantity: '2',
        price: testPrice,
      });

      await sleep(3000);

      // Verify maker ledger entries
      const makerLedger = await db.getLedgerEntriesForOrder(makerOrder.order.id);

      // Should have: lock (KCN), unlock (KCN), trade debit (KCN), trade credit (EUR)
      const makerLock = makerLedger.find(e => e.entry_type === 'lock' && e.asset === 'KCN');
      const makerUnlock = makerLedger.find(e => e.entry_type === 'unlock' && e.asset === 'KCN');
      const makerDebit = makerLedger.find(e => e.entry_type === 'trade' && parseFloat(e.amount) < 0);
      const makerCredit = makerLedger.find(e => e.entry_type === 'trade' && parseFloat(e.amount) > 0);

      assertNotNull(makerLock, 'Maker should have lock entry');
      assertNotNull(makerUnlock, 'Maker should have unlock entry');
      assertNotNull(makerDebit, 'Maker should have trade debit (KCN sold)');
      assertNotNull(makerCredit, 'Maker should have trade credit (EUR received)');

      // Verify amounts: 2 KCN * 450 EUR = 900 EUR
      assertEqual(parseFloat(makerDebit!.amount), -2, 'Maker should debit 2 KCN');
      assertEqual(parseFloat(makerCredit!.amount), 900, 'Maker should credit 900 EUR (2 * 450)');

    } finally {
      await db.deleteTestUser(makerEmail);
      await db.deleteTestUser(takerEmail);
    }
  });

  // NOTE: This test must run BEFORE the "partial fill" test below, because that test
  // leaves leftover liquidity in the orderbook at $400. The matching engine orderbook
  // persists between tests, so order matters.
  suite.test('market order that cannot be fully filled is cancelled with correct filled_quantity', async () => {
    // Test scenario:
    // 1. Maker provides limited liquidity (5 KCN at very high price)
    // 2. Taker places market buy for 10 KCN (more than available)
    // 3. Taker should get 5 KCN filled, remaining 5 KCN should be cancelled
    // 4. Order status should be 'cancelled' (not 'partially_filled')
    // 5. Unfilled EUR should be unlocked
    //
    // NOTE: We use a very high price ($999) to ensure no other liquidity exists at this level
    // from bot orders or other tests. The matching engine orderbook persists between tests.

    const makerEmail = generateTestEmail();
    const takerEmail = generateTestEmail();

    const makerApi = new AccountsClient();
    const takerApi = new AccountsClient();

    try {
      // Setup maker with sell order at very high price to avoid any pre-existing liquidity
      const makerAuth = await makerApi.login(makerEmail);
      await makerApi.claimFaucet(); // Get 100 KCN

      // Use a unique very high price ($999) to avoid conflicts with bot orders or other tests
      // Bot liquidity typically clusters around $100 range, so $999 should be clean
      const testPrice = '999.00';
      const makerQty = '5';

      // Maker places limit sell order for 5 KCN at $999
      await makerApi.placeOrderWithMatching({
        symbol: 'KCN/EUR',
        side: 'ask',
        order_type: 'limit',
        quantity: makerQty,
        price: testPrice,
      });

      await sleep(500);

      // Setup taker
      const takerAuth = await takerApi.login(takerEmail);
      // Deposit enough for 10 * 999 = 9990 EUR
      await takerApi.deposit('10000.00');

      // Record initial EUR balance
      const initialBalance = await db.getBalance(takerAuth.user.id, 'EUR');
      const initialAvailable = parseFloat(initialBalance!.available);

      // Taker places market buy for 10 KCN when only 5 is available
      // At $999 per KCN, there should be no other liquidity at this price
      const takerQty = '10';
      const takerOrder = await takerApi.placeOrderWithMatching({
        symbol: 'KCN/EUR',
        side: 'bid',
        order_type: 'market',
        quantity: takerQty,
        max_slippage_price: testPrice,
      });

      // Wait for matching and settlement + cancel processing
      await sleep(4000);

      // Verify order is cancelled (not partially_filled)
      const dbOrder = await db.getOrderById(takerOrder.order.id);
      assertNotNull(dbOrder, 'Taker order should exist in database');
      assertEqual(dbOrder!.status, 'cancelled', 'Market order should be cancelled after partial fill');

      // The filled quantity should be exactly 5 KCN (the maker's order at $999)
      // At such a high price, there should be no other liquidity to fill the remaining 5
      const filledQty = parseFloat(dbOrder!.filled_quantity);
      assertEqual(filledQty, 5, 'Should have filled exactly 5 KCN (the maker order)');
      assert(filledQty < parseFloat(takerQty), 'Should not have fully filled (only 5 of 10 KCN available)');

      // Verify taker received KCN (at least 5 from our maker)
      const takerKcnBalance = await db.getBalance(takerAuth.user.id, 'KCN');
      assertNotNull(takerKcnBalance, 'Taker should have KCN balance');
      assertGreater(parseFloat(takerKcnBalance!.available), 0, 'Taker should have received some KCN');

      // Verify EUR is not stuck in locked state (unfilled portion unlocked)
      const takerEurBalance = await db.getBalance(takerAuth.user.id, 'EUR');
      assertNotNull(takerEurBalance, 'Taker should have EUR balance');
      assertEqual(parseFloat(takerEurBalance!.locked), 0, 'Taker should have 0 EUR locked after cancel');

      // Verify unlock ledger entry exists
      const ledgerEntries = await db.getLedgerEntriesForOrder(takerOrder.order.id);
      const unlockEntry = ledgerEntries.find(e => e.entry_type === 'unlock');
      assertNotNull(unlockEntry, 'Should have unlock ledger entry for cancelled portion');

    } finally {
      await db.deleteTestUser(makerEmail);
      await db.deleteTestUser(takerEmail);
    }
  });

  suite.test('partial fill updates order filled_quantity correctly', async () => {
    const makerEmail = generateTestEmail();
    const takerEmail = generateTestEmail();

    const makerApi = new AccountsClient();
    const takerApi = new AccountsClient();

    try {
      // Setup maker with large sell order
      const makerAuth = await makerApi.login(makerEmail);
      await makerApi.claimFaucet();

      // Setup taker
      const takerAuth = await takerApi.login(takerEmail);
      await takerApi.deposit('1500.00'); // Needs 3 * 400 = 1200 EUR

      // Use high price to avoid bot liquidity. Place buy FIRST, then sell to match.
      const testPrice = '400.00';

      // Taker places buy order FIRST (only 3 KCN)
      await takerApi.placeOrderWithMatching({
        symbol: 'KCN/EUR',
        side: 'bid',
        order_type: 'limit',
        quantity: '3',
        price: testPrice,
      });

      await sleep(500);

      // Maker places large sell order (10 KCN) - should partially match taker's bid
      const makerOrder = await makerApi.placeOrderWithMatching({
        symbol: 'KCN/EUR',
        side: 'ask',
        order_type: 'limit',
        quantity: '10',
        price: testPrice,
      });

      await sleep(3000);

      // Verify maker order is partially filled
      const dbMakerOrder = await db.getOrderById(makerOrder.order.id);
      assertNotNull(dbMakerOrder, 'Maker order should exist');
      assertEqual(parseFloat(dbMakerOrder!.filled_quantity), 3, 'Should have 3 filled');
      assertEqual(dbMakerOrder!.status, 'partially_filled', 'Status should be partially_filled');

      // Verify remaining KCN balance after partial fill
      const makerBalance = await db.getBalance(makerAuth.user.id, 'KCN');
      // Started with 100, locked 10 (available=90), sold 3 and unlocked 3, so: available = 90 (still have 7 locked)
      // After fill: unlock 3, debit 3, so net available change is 0 from the fill itself
      // But remaining order still has 7 KCN locked
      assertNotNull(makerBalance, 'Maker should have KCN balance');

    } finally {
      await db.deleteTestUser(makerEmail);
      await db.deleteTestUser(takerEmail);
    }
  });

  return suite;
}
