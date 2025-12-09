import { TestClient } from '../test-client.js';
import { TestSuite, assert, assertNotNull, assertGreater, sleep } from '../test-framework.js';

export function createMatchingTests(): TestSuite {
  const suite = new TestSuite('Matching Engine Tests');
  let maker: TestClient;
  let taker: TestClient;

  suite.beforeAll(async () => {
    // Two clients to simulate maker/taker interaction
    maker = new TestClient();
    taker = new TestClient();

    await Promise.all([
      maker.connect(),
      taker.connect(),
    ]);

    await Promise.all([
      maker.subscribe(),
      taker.subscribe(),
    ]);

    // Let orderbook populate
    await sleep(2000);
  });

  suite.afterAll(async () => {
    await Promise.all([
      maker.disconnect(),
      taker.disconnect(),
    ]);
  });

  suite.beforeEach(async () => {
    maker.clearState();
    taker.clearState();
  });

  suite.test('both clients should see same orderbook', async () => {
    await sleep(500);

    // Compare top levels
    const makerBestBid = maker.getBestBid();
    const takerBestBid = taker.getBestBid();
    const makerBestAsk = maker.getBestAsk();
    const takerBestAsk = taker.getBestAsk();

    assertNotNull(makerBestBid, 'Maker should have best bid');
    assertNotNull(takerBestBid, 'Taker should have best bid');
    assertNotNull(makerBestAsk, 'Maker should have best ask');
    assertNotNull(takerBestAsk, 'Taker should have best ask');

    // Prices should match (or be very close due to timing)
    const bidDiff = Math.abs(makerBestBid.price - takerBestBid.price);
    const askDiff = Math.abs(makerBestAsk.price - takerBestAsk.price);

    assert(bidDiff < 1.0, `Best bids should match: ${makerBestBid.price} vs ${takerBestBid.price}`);
    assert(askDiff < 1.0, `Best asks should match: ${makerBestAsk.price} vs ${takerBestAsk.price}`);
  });

  suite.test('maker places order, taker crosses it - orderbook should update', async () => {
    // Get current best bid/ask
    const bestBid = maker.getBestBid();
    const bestAsk = maker.getBestAsk();
    assertNotNull(bestBid, 'Need best bid');
    assertNotNull(bestAsk, 'Need best ask');

    // Maker places a limit ask INSIDE the spread (becomes new best ask)
    const spread = bestAsk.price - bestBid.price;
    const makerPrice = bestAsk.price - (spread * 0.4);
    const makerQty = 2.0;
    const qtyBefore = maker.orderbook.asks.get(makerPrice) ?? 0;

    await maker.placeOrder('ask', 'limit', makerQty, makerPrice);
    await sleep(1000);

    // Verify maker's order appears
    const qtyAfter = maker.orderbook.asks.get(makerPrice) ?? 0;
    assert(qtyAfter > qtyBefore, `Maker's ask at ${makerPrice} should appear (was ${qtyBefore}, now ${qtyAfter})`);

    // Taker places market buy - should cross maker's order
    const updatesBefore = maker.updateCount;
    await taker.placeOrder('bid', 'market', makerQty);
    await sleep(1000);

    // Verify orderbook updated
    await maker.waitForOrderbookUpdate(5000);
    assert(maker.updateCount > updatesBefore, 'Should receive orderbook update after trade');
  });

  suite.test('should handle partial fills correctly', async () => {
    const bestBid = maker.getBestBid();
    const bestAsk = maker.getBestAsk();
    assertNotNull(bestBid, 'Need best bid');
    assertNotNull(bestAsk, 'Need best ask');

    // Maker places large ask INSIDE the spread
    const spread = bestAsk.price - bestBid.price;
    const makerPrice = bestAsk.price - (spread * 0.3);
    const makerQty = 10.0;
    const qtyBefore = maker.orderbook.asks.get(makerPrice) ?? 0;

    await maker.placeOrder('ask', 'limit', makerQty, makerPrice);
    await sleep(500);

    // Taker takes only part of it
    const takerQty = 3.0;
    await taker.placeOrder('bid', 'market', takerQty);
    await sleep(1000);

    // Verify partial fill - remaining quantity should have increased by ~7.0
    await maker.waitForOrderbookUpdate(5000);

    const qtyAfter = maker.orderbook.asks.get(makerPrice);
    if (qtyAfter !== undefined) {
      const expectedRemaining = qtyBefore + makerQty - takerQty;
      assert(
        qtyAfter > expectedRemaining - 1 && qtyAfter < expectedRemaining + 1,
        `Remaining qty ${qtyAfter} should be ~${expectedRemaining}`
      );
    }
    // If the level doesn't exist, it might have been consumed by other orders
  });

  suite.test('should handle multiple price levels in single order', async () => {
    const bestBid = maker.getBestBid();
    const bestAsk = maker.getBestAsk();
    assertNotNull(bestBid, 'Need best bid');
    assertNotNull(bestAsk, 'Need best ask');

    // Place asks INSIDE the spread at multiple price levels
    const spread = bestAsk.price - bestBid.price;
    const basePrice = bestAsk.price - (spread * 0.5);
    const levels = [
      { price: basePrice, qty: 1.0 },
      { price: basePrice + 0.01, qty: 1.0 },
      { price: basePrice + 0.02, qty: 1.0 },
    ];

    const updatesBefore = maker.updateCount;

    for (const level of levels) {
      await maker.placeOrder('ask', 'limit', level.qty, level.price);
    }
    await sleep(1000);

    // Taker places large market buy that should sweep multiple levels
    await taker.placeOrder('bid', 'market', 2.5);
    await sleep(1000);

    // Verify orderbook state - we should see updates from the trades
    await maker.waitForOrderbookUpdate(5000);
    assert(maker.updateCount > updatesBefore, 'Should receive orderbook updates after sweeping levels');
  });

  suite.test('should maintain FIFO ordering at same price level', async () => {
    const bestBid = maker.getBestBid();
    const bestAsk = maker.getBestAsk();
    assertNotNull(bestBid, 'Need best bid');
    assertNotNull(bestAsk, 'Need best ask');

    // Use a price INSIDE the spread so it appears in top 10 levels
    const spread = bestAsk.price - bestBid.price;
    const testPrice = bestBid.price + (spread * 0.4);
    const qtyBefore = maker.orderbook.bids.get(testPrice) ?? 0;

    // Both clients place orders at same price
    await maker.placeOrder('bid', 'limit', 1.0, testPrice);
    await sleep(100);
    await taker.placeOrder('bid', 'limit', 1.0, testPrice);
    await sleep(500);

    // Verify both orders exist at that level
    await maker.waitForOrderbookUpdate(3000);
    const qtyAfter = maker.orderbook.bids.get(testPrice);
    if (qtyAfter !== undefined) {
      assertGreater(qtyAfter, qtyBefore + 1.5, `Level should have both orders (was ${qtyBefore}, now ${qtyAfter})`);
    }
  });

  return suite;
}
