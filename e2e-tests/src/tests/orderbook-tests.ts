import { TestClient } from '../test-client.js';
import { TestSuite, assert, assertNotNull, assertGreater, sleep } from '../test-framework.js';

export function createOrderbookTests(): TestSuite {
  const suite = new TestSuite('Orderbook Tests');
  let client: TestClient;

  suite.beforeAll(async () => {
    client = new TestClient();
    await client.connect();
    await client.subscribe();
  });

  suite.afterAll(async () => {
    await client.disconnect();
  });

  suite.beforeEach(async () => {
    client.clearState();
  });

  suite.test('should receive orderbook snapshot on subscribe', async () => {
    assert(client.isSubscribed(), 'Client should be subscribed');
    assert(client.updateCount > 0, 'Should have received at least one update');
  });

  suite.test('should have bid and ask levels', async () => {
    // Wait for orderbook to populate
    await sleep(2000);

    assert(client.orderbook.bids.size > 0, 'Should have bid levels');
    assert(client.orderbook.asks.size > 0, 'Should have ask levels');
  });

  suite.test('should have correct bid/ask ordering', async () => {
    await sleep(1000);

    const bids = client.getBidLevels();
    const asks = client.getAskLevels();

    // Bids should be in descending order
    for (let i = 1; i < bids.length; i++) {
      assert(bids[i - 1].price >= bids[i].price, `Bids should be descending`);
    }

    // Asks should be in ascending order
    for (let i = 1; i < asks.length; i++) {
      assert(asks[i - 1].price <= asks[i].price, `Asks should be ascending`);
    }
  });

  suite.test('should have positive spread (best ask > best bid)', async () => {
    await sleep(1000);

    const bestBid = client.getBestBid();
    const bestAsk = client.getBestAsk();

    assertNotNull(bestBid, 'Should have best bid');
    assertNotNull(bestAsk, 'Should have best ask');

    const spread = client.getSpread();
    assertNotNull(spread, 'Should have spread');
    assertGreater(spread, 0, `Spread should be positive: ${spread}`);
  });

  suite.test('should receive orderbook updates', async () => {
    const startCount = client.updateCount;

    // Wait for updates
    await client.waitForOrderbookUpdate(10000);

    assert(
      client.updateCount > startCount,
      `Update count should increase: ${client.updateCount} > ${startCount}`
    );
  });

  suite.test('should handle multiple rapid updates', async () => {
    const startCount = client.updateCount;

    // Wait for several updates
    for (let i = 0; i < 3; i++) {
      await client.waitForOrderbookUpdate(5000);
    }

    assert(
      client.updateCount >= startCount + 3,
      `Should receive at least 3 updates`
    );
  });

  return suite;
}
