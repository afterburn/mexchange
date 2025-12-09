import { TestClient } from '../test-client.js';
import { AccountsClient } from '../accounts-client.js';
import { DbClient } from '../db-client.js';
import { TestSuite, assert, assertNotNull, assertGreater, sleep } from '../test-framework.js';

// Generate unique test email
function generateTestEmail(): string {
  const timestamp = Date.now();
  const random = Math.random().toString(36).substring(2, 8);
  return `e2e-settlement-${timestamp}-${random}@test.mexchange.local`;
}

export function createSettlementTests(): TestSuite {
  const suite = new TestSuite('Settlement & Fill Tests');
  let client: TestClient;
  let db: DbClient;

  // Liquidity provider accounts
  let askMaker: AccountsClient;
  let bidMaker: AccountsClient;
  let askMakerEmail: string;
  let bidMakerEmail: string;

  // Price levels for liquidity - use prices that won't cross with each other
  // or with leftover orders in the book
  // Ask price must be HIGH (sellers want high prices)
  // Bid price must be LOW (buyers want low prices)
  const ASK_PRICE = 950;  // High ask - won't match low bids
  const BID_PRICE = 50;   // Low bid - won't match high asks
  const LIQUIDITY_QTY = 50;

  suite.beforeAll(async () => {
    db = new DbClient();
    await db.connect();

    // Create WebSocket client for receiving trade notifications
    client = new TestClient();
    await client.connect();
    await client.subscribe();

    // Create liquidity providers
    askMakerEmail = generateTestEmail();
    bidMakerEmail = generateTestEmail();

    askMaker = new AccountsClient();
    bidMaker = new AccountsClient();

    // Setup ask-side liquidity (seller needs KCN)
    await askMaker.login(askMakerEmail);
    await askMaker.claimFaucet(); // Get 100 KCN

    // Place sell order to provide ask liquidity
    await askMaker.placeOrderWithMatching({
      symbol: 'KCN/EUR',
      side: 'ask',
      order_type: 'limit',
      quantity: LIQUIDITY_QTY.toString(),
      price: ASK_PRICE.toString(),
    });

    // Setup bid-side liquidity (buyer needs EUR)
    await bidMaker.login(bidMakerEmail);
    await bidMaker.deposit('50000.00'); // Deposit EUR

    // Place buy order to provide bid liquidity
    await bidMaker.placeOrderWithMatching({
      symbol: 'KCN/EUR',
      side: 'bid',
      order_type: 'limit',
      quantity: LIQUIDITY_QTY.toString(),
      price: BID_PRICE.toString(),
    });

    // Wait for orders to reach matching engine and orderbook to update
    await sleep(2000);

    // Verify liquidity is in place
    const bestAsk = client.getBestAsk();
    const bestBid = client.getBestBid();

    if (!bestAsk || !bestBid) {
      console.log('Warning: Liquidity may not have propagated yet');
      console.log('Best ask:', bestAsk);
      console.log('Best bid:', bestBid);
    }
  });

  suite.afterAll(async () => {
    await client.disconnect();

    // Cleanup test users
    if (askMakerEmail) await db.deleteTestUser(askMakerEmail);
    if (bidMakerEmail) await db.deleteTestUser(bidMakerEmail);

    await db.disconnect();
  });

  suite.beforeEach(async () => {
    client.clearState();
  });

  suite.test('market buy order should produce a trade', async () => {
    const bestAsk = client.getBestAsk();
    assertNotNull(bestAsk, 'Need asks to buy against - liquidity setup may have failed');

    // Record initial state
    const initialTradeCount = client.trades.length;

    // Place a market buy order (anonymous via WebSocket)
    const quantity = 1.0;
    await client.placeOrder('bid', 'market', quantity);

    // Wait for trade to be recorded
    await sleep(2000);

    // Verify a trade was recorded
    assertGreater(
      client.trades.length,
      initialTradeCount,
      `Should have received trade notification. Initial: ${initialTradeCount}, Current: ${client.trades.length}`
    );

    // Verify the trade details make sense
    const lastTrade = client.trades[client.trades.length - 1];
    assert(lastTrade.quantity > 0, 'Trade should have positive quantity');
    assert(lastTrade.price > 0, 'Trade should have positive price');
  });

  suite.test('market sell order should produce a trade', async () => {
    const bestBid = client.getBestBid();
    assertNotNull(bestBid, 'Need bids to sell against - liquidity setup may have failed');

    // Record initial state
    const initialTradeCount = client.trades.length;

    // Place a market sell order (anonymous via WebSocket)
    const quantity = 1.0;
    await client.placeOrder('ask', 'market', quantity);

    // Wait for trade to be recorded
    await sleep(2000);

    // Verify a trade was recorded
    assertGreater(
      client.trades.length,
      initialTradeCount,
      `Should have received trade notification. Initial: ${initialTradeCount}, Current: ${client.trades.length}`
    );
  });

  suite.test('limit order crossing spread should produce immediate fill', async () => {
    const bestAsk = client.getBestAsk();
    assertNotNull(bestAsk, 'Need asks to cross - liquidity setup may have failed');

    // Record initial state
    const initialTradeCount = client.trades.length;

    // Place a limit bid at the ask price (should immediately cross)
    await client.placeOrder('bid', 'limit', 0.5, bestAsk.price);

    // Wait for trade
    await sleep(2000);

    // Verify a trade was recorded
    assertGreater(
      client.trades.length,
      initialTradeCount,
      `Crossing limit order should produce trade. Initial: ${initialTradeCount}, Current: ${client.trades.length}`
    );
  });

  suite.test('multiple concurrent market orders should all produce trades', async () => {
    const bestAsk = client.getBestAsk();
    const bestBid = client.getBestBid();
    assertNotNull(bestAsk, 'Need asks - liquidity setup may have failed');
    assertNotNull(bestBid, 'Need bids - liquidity setup may have failed');

    // Record initial state
    const initialTradeCount = client.trades.length;

    // Place multiple orders concurrently (all anonymous via WebSocket)
    await Promise.all([
      client.placeOrder('bid', 'market', 0.5),
      client.placeOrder('ask', 'market', 0.5),
      client.placeOrder('bid', 'market', 0.5),
    ]);

    // Wait for all trades
    await sleep(3000);

    // Verify multiple trades were recorded (at least 2, since some might partially fill)
    assertGreater(
      client.trades.length,
      initialTradeCount + 1,
      `Should have at least 2 new trades. Initial: ${initialTradeCount}, Current: ${client.trades.length}`
    );
  });

  suite.test('orderbook should update after fill', async () => {
    const bestAsk = client.getBestAsk();
    assertNotNull(bestAsk, 'Need asks - liquidity setup may have failed');

    const initialQty = bestAsk.quantity;

    // Place a market buy that should partially consume the best ask
    const consumeQty = 0.5;
    await client.placeOrder('bid', 'market', consumeQty);

    // Wait for orderbook update
    await sleep(1000);
    await client.waitForOrderbookUpdate(5000);

    // The best ask should have less quantity or be a different price level
    const newBestAsk = client.getBestAsk();
    assertNotNull(newBestAsk, 'Should still have asks');

    // Either quantity decreased or price changed (if level was consumed)
    const quantityDecreased = newBestAsk.price === bestAsk.price && newBestAsk.quantity < initialQty;
    const priceChanged = newBestAsk.price !== bestAsk.price;

    assert(
      quantityDecreased || priceChanged,
      `Orderbook should update after fill. Initial: ${bestAsk.price}@${initialQty}, New: ${newBestAsk.price}@${newBestAsk.quantity}`
    );
  });

  return suite;
}
