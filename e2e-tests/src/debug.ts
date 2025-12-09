import { TestClient } from './test-client.js';

async function debug() {
  console.log('Starting debug...');

  const client = new TestClient();

  // Log all messages
  client.addMessageHandler((msg) => {
    const m = msg as { channel_name?: string; notification?: { bid_changes: unknown[]; ask_changes: unknown[] } };
    if (m.notification) {
      console.log(`[MSG] bids: ${m.notification.bid_changes.length}, asks: ${m.notification.ask_changes.length}`);
    }
  });

  await client.connect();
  console.log('Connected');

  await client.subscribe();
  console.log('Subscribed');
  console.log(`Initial: ${client.orderbook.bids.size} bids, ${client.orderbook.asks.size} asks`);

  // Wait for more data
  await new Promise(r => setTimeout(r, 2000));
  console.log(`After 2s: ${client.orderbook.bids.size} bids, ${client.orderbook.asks.size} asks`);

  const bestBid = client.getBestBid();
  const bestAsk = client.getBestAsk();
  console.log(`Best bid: ${bestBid?.price} @ ${bestBid?.quantity}`);
  console.log(`Best ask: ${bestAsk?.price} @ ${bestAsk?.quantity}`);

  if (bestBid) {
    // Place an order below best bid
    const testPrice = bestBid.price - 0.5;
    console.log(`\nPlacing bid at ${testPrice}...`);
    await client.placeOrder('bid', 'limit', 5.0, testPrice);

    // Wait for updates
    console.log('Waiting for updates...');
    for (let i = 0; i < 5; i++) {
      await new Promise(r => setTimeout(r, 1000));
      const found = client.orderbook.bids.get(testPrice);
      console.log(`  Check ${i+1}: price ${testPrice} -> ${found ?? 'NOT FOUND'}`);
      console.log(`  Total bids: ${client.orderbook.bids.size}`);

      // List all bids
      const bids = client.getBidLevels(20);
      console.log(`  Levels: ${bids.map(b => `${b.price}:${b.quantity}`).join(', ')}`);
    }
  }

  await client.disconnect();
  console.log('\nDone');
}

debug().catch(console.error);
