# Matching Engine Service

Service that wraps the matching engine library and publishes events to Kafka.

## Architecture

```
HTTP API → Matching Engine Service → OrderBook → Kafka → Gateway Servers
```

## Features

- REST API for order placement and cancellation
- Publishes fill events to Kafka when orders match
- Publishes orderbook updates every second
- Publishes order accepted/cancelled events

## Configuration

Environment variables:

- `KAFKA_BROKERS`: Kafka broker addresses (default: `localhost:9092`)
- `KAFKA_TOPIC`: Kafka topic for market events (default: `market-events`)
- `SYMBOL`: Trading pair symbol (default: `KCN/EUR`)
- `BIND_ADDR`: Server bind address (default: `0.0.0.0:8080`)

## Running

```bash
cargo run --bin matching_engine_service
```

With Kafka:

```bash
KAFKA_BROKERS=localhost:9092 KAFKA_TOPIC=market-events cargo run --bin matching_engine_service
```

## API Endpoints

- `GET /health` - Health check
- `POST /api/order` - Place order
  ```json
  {
    "side": "bid",
    "order_type": "limit",
    "price": 50000.0,
    "quantity": 0.1
  }
  ```
- `POST /api/order/cancel` - Cancel order
  ```json
  {
    "order_id": 123
  }
  ```

## Kafka Events Published

- `fill` - When orders are matched
- `order_accepted` - When an order is accepted
- `order_cancelled` - When an order is cancelled
- `orderbook_update` - Periodic orderbook snapshots (every 1 second)

## Testing Kafka

To verify events are being published to Kafka:

```bash
# Start Kafka consumer to see events
kafka-console-consumer --bootstrap-server localhost:9092 --topic market-events --from-beginning

# In another terminal, place an order
curl -X POST http://localhost:8080/api/order \
  -H "Content-Type: application/json" \
  -d '{"side": "bid", "order_type": "limit", "price": 50000.0, "quantity": 0.1}'
```


