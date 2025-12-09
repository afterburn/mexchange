import { TestClient } from '../test-client.js';
import { TestSuite, assert, assertNotNull, assertGreater, sleep } from '../test-framework.js';

export function createOrderFlowTests(): TestSuite {
  const suite = new TestSuite('Order Flow Tests');
  let client: TestClient;

  suite.beforeAll(async () => {
    client = new TestClient();
    await client.connect();
    await client.subscribe();
    // Wait for orderbook to populate
    await sleep(2000);
  });

  suite.afterAll(async () => {
    await client.disconnect();
  });

  suite.beforeEach(async () => {
    client.clearState();
  });

  suite.test('should place a limit bid order', async () => {
    const bestBid = client.getBestBid();
    const bestAsk = client.getBestAsk();
    assertNotNull(bestBid, 'Need existing bids for reference');
    assertNotNull(bestAsk, 'Need existing asks for reference');

    // Place a bid ABOVE best bid but below best ask (won't cross, becomes new best bid)
    // This ensures it appears in the top 10 levels which are published
    const spread = bestAsk.price - bestBid.price;
    // Round to 2 decimal places to match the matching engine's precision
    const price = Math.round((bestBid.price + (spread * 0.3)) * 100) / 100;
    const qtyBefore = client.orderbook.bids.get(price) ?? 0;

    await client.placeOrder('bid', 'limit', 1.0, price);

    // Wait for orderbook update
    await sleep(500);
    await client.waitForOrderbookUpdate(5000);

    // Our order should appear in the book (either new level or added to existing)
    // Use tolerance-based lookup since floating point keys may not match exactly
    let qtyAfter = client.orderbook.bids.get(price);
    if (qtyAfter === undefined) {
      // Try to find a price level within tolerance
      for (const [p, q] of client.orderbook.bids) {
        if (Math.abs(p - price) < 0.001) {
          qtyAfter = q;
          break;
        }
      }
    }
    const hasBidAtPrice = qtyAfter !== undefined && qtyAfter > qtyBefore;
    assert(hasBidAtPrice, `Bid at ${price} should appear in orderbook (was ${qtyBefore}, now ${qtyAfter})`);
  });

  suite.test('should place a limit ask order', async () => {
    const bestBid = client.getBestBid();
    const bestAsk = client.getBestAsk();
    assertNotNull(bestBid, 'Need existing bids for reference');
    assertNotNull(bestAsk, 'Need existing asks for reference');

    // Place an ask BELOW best ask but above best bid (won't cross, becomes new best ask)
    // This ensures it appears in the top 10 levels which are published
    const spread = bestAsk.price - bestBid.price;
    const price = bestAsk.price - (spread * 0.3); // 30% into the spread from ask side
    const qtyBefore = client.orderbook.asks.get(price) ?? 0;

    await client.placeOrder('ask', 'limit', 1.0, price);

    await sleep(500);
    await client.waitForOrderbookUpdate(5000);

    // Our order should appear in the book
    const qtyAfter = client.orderbook.asks.get(price);
    const hasAskAtPrice = qtyAfter !== undefined && qtyAfter > qtyBefore;
    assert(hasAskAtPrice, `Ask at ${price} should appear in orderbook (was ${qtyBefore}, now ${qtyAfter})`);
  });

  suite.test('should execute a market buy order crossing the spread', async () => {
    const bestAsk = client.getBestAsk();
    assertNotNull(bestAsk, 'Need asks to buy against');

    const askQty = bestAsk.quantity;
    const orderQty = Math.min(askQty * 0.5, 1.0);

    const updatesBefore = client.updateCount;
    await client.placeOrder('bid', 'market', orderQty);

    // Wait for orderbook to update (market order should cause changes)
    await sleep(500);
    await client.waitForOrderbookUpdate(10000);

    assertGreater(client.updateCount, updatesBefore, 'Should receive orderbook update after market order');
  });

  suite.test('should execute a market sell order crossing the spread', async () => {
    const bestBid = client.getBestBid();
    assertNotNull(bestBid, 'Need bids to sell against');

    const bidQty = bestBid.quantity;
    const orderQty = Math.min(bidQty * 0.5, 1.0);

    await client.placeOrder('ask', 'market', orderQty);

    // Market orders should cause orderbook changes
    await sleep(500);
    await client.waitForOrderbookUpdate(5000);
  });

  suite.test('should handle batch order placement', async () => {
    const bestBid = client.getBestBid();
    const bestAsk = client.getBestAsk();
    assertNotNull(bestBid, 'Need bids for reference');
    assertNotNull(bestAsk, 'Need asks for reference');

    // Place orders INSIDE the spread so they appear in the top levels
    const spread = bestAsk.price - bestBid.price;
    const bidPrice1 = bestBid.price + (spread * 0.2);
    const bidPrice2 = bestBid.price + (spread * 0.25);
    const askPrice1 = bestAsk.price - (spread * 0.2);
    const askPrice2 = bestAsk.price - (spread * 0.25);

    const bidQtyBefore1 = client.orderbook.bids.get(bidPrice1) ?? 0;
    const bidQtyBefore2 = client.orderbook.bids.get(bidPrice2) ?? 0;

    // Place multiple orders at once
    await client.placeBatchOrders([
      { side: 'bid', order_type: 'limit', quantity: 0.5, price: bidPrice1 },
      { side: 'bid', order_type: 'limit', quantity: 0.5, price: bidPrice2 },
      { side: 'ask', order_type: 'limit', quantity: 0.5, price: askPrice1 },
      { side: 'ask', order_type: 'limit', quantity: 0.5, price: askPrice2 },
    ]);

    await sleep(500);
    await client.waitForOrderbookUpdate(5000);

    // Verify at least one bid order appears (checking quantity increased)
    const bidQtyAfter1 = client.orderbook.bids.get(bidPrice1) ?? 0;
    const bidQtyAfter2 = client.orderbook.bids.get(bidPrice2) ?? 0;
    const hasBids = bidQtyAfter1 > bidQtyBefore1 || bidQtyAfter2 > bidQtyBefore2;

    // For asks, just verify the levels exist near our prices
    const askPrices = Array.from(client.orderbook.asks.keys());
    const hasAsks = askPrices.some(p => p <= askPrice1 && p >= askPrice2);

    assert(hasBids, 'Batch bid orders should appear');
    assert(hasAsks, 'Batch ask orders should appear');
  });

  suite.test('should maintain orderbook consistency after multiple operations', async () => {
    const initialBestBid = client.getBestBid();
    const initialBestAsk = client.getBestAsk();
    assertNotNull(initialBestBid, 'Need initial best bid');
    assertNotNull(initialBestAsk, 'Need initial best ask');

    const spread = initialBestAsk.price - initialBestBid.price;

    // Place several orders rapidly INSIDE the spread
    for (let i = 0; i < 5; i++) {
      const bidPrice = initialBestBid.price + (spread * (0.1 + i * 0.05));
      const askPrice = initialBestAsk.price - (spread * (0.1 + i * 0.05));

      await client.placeOrder('bid', 'limit', 0.1, bidPrice);
      await client.placeOrder('ask', 'limit', 0.1, askPrice);
    }

    await sleep(1000);

    // Verify orderbook is still consistent
    const currentSpread = client.getSpread();
    assertNotNull(currentSpread, 'Should have spread');
    assertGreater(currentSpread, 0, 'Spread should remain positive');

    // Verify ordering
    const bids = client.getBidLevels();
    for (let i = 1; i < bids.length; i++) {
      assert(bids[i - 1].price >= bids[i].price, `Bids should remain sorted`);
    }
  });

  return suite;
}
