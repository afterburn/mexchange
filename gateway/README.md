# Gateway Server

Client-facing API server for mExchange. Provides WebSocket and REST interfaces for traders, handles authentication integration and rate limiting.

## Architecture

The gateway server is designed to run across multiple servers for horizontal scaling:

```
Matching Engine → Kafka → Gateway Server 1 → WebSocket → Frontend Clients
                  ↓       Gateway Server 2 → WebSocket → Frontend Clients
                          Gateway Server N → WebSocket → Frontend Clients
```

### Components

- **HTTP/WebSocket Server**: Handles client connections using Axum
- **Kafka Consumer**: Subscribes to market events from the matching engine
- **Event Broadcasting**: Distributes market events to connected WebSocket clients
- **REST API**: Order placement and cancellation endpoints

## Features

- WebSocket connections for real-time market data
- REST API for order management
- Horizontal scaling via Kafka consumer groups
- Automatic client reconnection handling
- Health check endpoint

## Configuration

Environment variables:

- `KAFKA_BROKERS`: Kafka broker addresses (default: `localhost:9092`)
- `KAFKA_TOPIC`: Kafka topic for market events (default: `market-events`)
- `KAFKA_GROUP_ID`: Consumer group ID (default: `gateway`)
- `BIND_ADDR`: Server bind address (default: `0.0.0.0:3000`)

## Running

```bash
cargo run --bin gateway
```

With custom configuration:

```bash
KAFKA_BROKERS=localhost:9092 KAFKA_TOPIC=market-events BIND_ADDR=0.0.0.0:3000 cargo run --bin gateway
```

## API Endpoints

### WebSocket

- `ws://localhost:3000/ws` - Real-time market data stream

### REST

- `GET /health` - Health check
- `POST /api/order` - Place order
- `POST /api/order/cancel` - Cancel order

## Kafka Integration

The gateway subscribes to Kafka topics published by the matching engine. Multiple gateway instances can run in the same consumer group for load balancing, or different groups for broadcasting to all instances.

## Industry Standard

Uses **Apache Kafka** - the industry standard for event streaming in financial exchanges (used by Binance, Coinbase, NASDAQ, and other major exchanges).


