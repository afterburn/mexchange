import { TestClient } from '../test-client.js';
import { ServiceMonitor } from '../service-monitor.js';
import { TestSuite, assert, assertNotNull, sleep, retry } from '../test-framework.js';

export function createConnectivityTests(): TestSuite {
  const suite = new TestSuite('Connectivity & Resilience Tests');
  const monitor = new ServiceMonitor();

  suite.test('all services should be healthy', async () => {
    const status = await monitor.checkAllServices();
    assert(status.allHealthy, `Not all services healthy: ${monitor.formatStatus(status)}`);
  });

  suite.test('should connect to WebSocket within 5s', async () => {
    const client = new TestClient();
    try {
      await client.connect(5000);
      assert(client.isConnected(), 'Client should be connected');
    } finally {
      await client.disconnect();
    }
  });

  suite.test('should receive orderbook snapshot within 5s of subscribe', async () => {
    const client = new TestClient();
    try {
      await client.connect();
      await client.subscribe(5000);
      assert(client.isSubscribed(), 'Client should be subscribed');
      // Check that we have orderbook data (bids or asks populated)
      assert(client.orderbook.bids.size > 0 || client.orderbook.asks.size > 0, 'Should have orderbook data');
    } finally {
      await client.disconnect();
    }
  });

  suite.test('should handle rapid connect/disconnect cycles', async () => {
    for (let i = 0; i < 5; i++) {
      const client = new TestClient();
      await client.connect(3000);
      assert(client.isConnected(), `Connection ${i + 1} should succeed`);
      await client.subscribe(3000);
      await client.disconnect();
    }
  });

  suite.test('should handle multiple concurrent connections', async () => {
    const clients: TestClient[] = [];
    const numClients = 5;

    try {
      // Connect all clients
      for (let i = 0; i < numClients; i++) {
        clients.push(new TestClient());
      }

      await Promise.all(clients.map(c => c.connect()));

      // All should be connected
      for (const client of clients) {
        assert(client.isConnected(), 'All clients should be connected');
      }

      // Subscribe all
      await Promise.all(clients.map(c => c.subscribe()));

      // All should have orderbook data
      for (const client of clients) {
        assert(client.orderbook.bids.size > 0 || client.orderbook.asks.size > 0, 'All clients should have data');
      }
    } finally {
      await Promise.all(clients.map(c => c.disconnect()));
    }
  });

  suite.test('should recover from temporary disconnection', async () => {
    const client = new TestClient();

    try {
      await client.connect();
      await client.subscribe();

      // Verify we have initial data
      assert(client.orderbook.bids.size > 0 || client.orderbook.asks.size > 0, 'Should have initial data');

      // Disconnect and reconnect
      await client.disconnect();
      await sleep(500);

      await client.connect();
      await client.subscribe();

      // Should have new data after reconnect
      await client.waitForOrderbookUpdate(5000);
      assert(client.orderbook.bids.size > 0 || client.orderbook.asks.size > 0, 'Should have data after reconnect');
    } finally {
      await client.disconnect();
    }
  });

  suite.test('gateway should respond to health checks under load', async () => {
    // Create several active connections
    const clients: TestClient[] = [];
    const numClients = 10;

    try {
      for (let i = 0; i < numClients; i++) {
        const client = new TestClient();
        await client.connect();
        await client.subscribe();
        clients.push(client);
      }

      // Health check should still respond quickly
      const status = await monitor.checkAllServices();
      assert(status.allHealthy, 'Services should be healthy under load');

      const gatewayLatency = status.services.find(s => s.name === 'gateway')?.latencyMs;
      assertNotNull(gatewayLatency, 'Should have gateway latency');
      assert(gatewayLatency < 1000, `Gateway latency ${gatewayLatency}ms should be < 1000ms`);
    } finally {
      await Promise.all(clients.map(c => c.disconnect()));
    }
  });

  suite.test('should handle high-frequency order placement', async () => {
    const client = new TestClient();

    try {
      await client.connect();
      await client.subscribe();
      await sleep(1000);

      const bestBid = client.getBestBid();
      const bestAsk = client.getBestAsk();
      assertNotNull(bestBid, 'Need best bid');
      assertNotNull(bestAsk, 'Need best ask');

      // Place many orders rapidly
      const numOrders = 50;
      const orders = [];
      for (let i = 0; i < numOrders; i++) {
        const side = i % 2 === 0 ? 'bid' : 'ask';
        const basePrice = side === 'bid'
          ? parseFloat(bestBid.price) - 5
          : parseFloat(bestAsk.price) + 5;
        orders.push({
          side: side as 'bid' | 'ask',
          order_type: 'limit' as const,
          quantity: 0.1,
          price: basePrice + (i * 0.01),
        });
      }

      await client.placeBatchOrders(orders);

      // Wait for orderbook to update
      await sleep(2000);
      await client.waitForOrderbookUpdate(5000);

      // Verify no errors
      assert(client.errors.length === 0, `Should have no errors: ${client.errors.join(', ')}`);
    } finally {
      await client.disconnect();
    }
  });

  return suite;
}
