# mExchange

A modular, high-performance cryptocurrency exchange platform built with Rust. mExchange is designed as a collection of independent services and libraries that work together to create a complete trading system.

## Overview

mExchange is a full exchange architecture where each component handles a specific responsibility. This repository currently contains the foundation - a high-performance matching engine library.

The matching engine is complete. Future components will include risk management, settlement, market data distribution, and administration services.

## Architecture Philosophy

Microservices-inspired design where each component is an independent Rust crate:
- Independent scaling and deployment
- Fault isolation
- Technology flexibility
- Clear boundaries

## Components

### matching_engine (Complete)

High-performance matching engine library. Price-time priority matching with support for limit and market orders. Partial fills across multiple price levels. Pure matching engine with no validation, networking, or storage.

Performance: 5.2M orders/sec throughput, comparable to tier-1 exchanges.

[See matching_engine/README.md for complete documentation](matching_engine/README.md)

### gateway (Planned)

Client-facing API server. Provides WebSocket and REST interfaces for traders, handles authentication integration and rate limiting.

### risk_engine (Planned)

Pre-trade risk management service. Validates orders against balance limits, position limits, and circuit breakers before routing to matching engine.

### settlement (Planned)

Post-trade settlement and balance management. Updates account balances, handles deposits/withdrawals, maintains ledger.

### market_data (Planned)

Market data distribution service. Provides real-time orderbook depth snapshots, trade history, OHLCV aggregation, and historical data APIs.

### auth (Planned)

Authentication and authorization service. Manages user authentication, API keys, sessions, and permissions.

### admin (Planned)

Administrative interface and monitoring. Provides trading controls, user management, system monitoring, and performance metrics.

## Design Philosophy

### Separation of Concerns

Each component has a single, well-defined responsibility. The matching engine handles order matching, risk engine handles validation, gateway handles client connections. This separation enables independent scaling, testing, and deployment.

### Performance First

Built for high-throughput, low-latency trading. Critical paths are optimized for minimal allocations and maximum efficiency. Rust's zero-cost abstractions and ownership system ensure memory safety without runtime overhead.

### Code Standards

- Comprehensive test coverage
- Clean, idiomatic Rust
- Minimal comments (explain why not what)
- Benchmark-driven optimization

## Getting Started

### Prerequisites

- Rust 1.70+ (edition 2021)
- Cargo

### Building

```bash
cd matching_engine
cargo build --release
```

### Running Tests

```bash
cd matching_engine
cargo test
```

### Running Benchmarks

```bash
cd matching_engine
cargo bench
```

### Examples

```bash
cd matching_engine
cargo run --example building_orderbook
cargo run --example market_orders
cargo run --example crossing_spread
cargo run --example partial_fills
cargo run --example order_cancellation
```

## Using the Matching Engine

Add to your `Cargo.toml`:

```toml
[dependencies]
matching_engine = { path = "../matching_engine" }
rust_decimal = "1.35"
```

### API

- `add_limit_order(side, price, quantity) -> OrderResult`
- `add_market_order(side, quantity) -> OrderResult`
- `cancel_order(order_id) -> bool`
- `best_bid() -> Option<Price>`
- `best_ask() -> Option<Price>`
- `spread() -> Option<Price>`
- `quantity_at_price(side, price) -> Quantity`

## Architecture Decisions

### Why Separate Binaries?

1. **Scalability** - Scale matching engine independently from gateway
2. **Fault isolation** - Gateway crash doesn't affect matching
3. **Deployment flexibility** - Update components without full system restart
4. **Technology choice** - Can rewrite components in different languages if needed

### Why Rust

- Zero-cost abstractions, no garbage collection
- Memory safety without runtime overhead
- Fearless concurrency with ownership system
- Excellent ecosystem for async/networking/cryptography

## Contributing

Contributions are welcome! Please:
- Ensure all tests pass
- Add tests for new functionality
- Follow the code style guidelines
- Run benchmarks to check for performance regressions
- Update documentation

## License

MIT
