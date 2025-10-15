# mExchange

A modular, high-performance cryptocurrency exchange platform built with Rust. mExchange is designed as a collection of independent services and libraries that work together to create a complete trading system.

## Overview

mExchange will be a full exchange architecture where each component handles a specific responsibility. This repository currently contains the foundation: a high-performance matching engine library.

**Current Status:** Foundation phase - Core matching engine library complete

**Vision:** Complete exchange platform with modular services for matching, risk management, settlement, market data, and administration

## Architecture Philosophy

Microservices-inspired design where each component is an independent Rust crate:
- Independent scaling
- Fault isolation
- Technology flexibility
- Clear boundaries
- Independent deployment

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

### Pure Matching Engine

The matching engine handles matching only - no validation, balance checks, risk management, networking, or persistence. This separation allows:
- Maximum throughput (validation happens upstream)
- Simple, testable, focused code
- Reusable across contexts (spot, futures, options)

### Performance First

- Zero-allocation hot paths where possible
- Early returns to minimize branching
- Idiomatic Rust for compiler optimizations
- Comprehensive benchmarking

### Code Standards

- Comprehensive test coverage
- Minimal comments (explain why not what)
- Early returns over nested conditionals
- Clean output

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

### Code Style

- Early returns over nested if/else
- Comments explain why not what
- Idiomatic Rust
- Clean output


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

MIT OR Apache-2.0

## Roadmap

**Phase 1: Foundation (Complete)**
- Core matching engine implementation
- Price-time priority matching
- Partial fills across levels
- High-precision decimal support
- Comprehensive test suite (28 tests)
- Performance benchmarks

**Phase 2: Networking Layer**
- Network interface for order submission
- Order state management
- Trade publication
- Integration tests

**Phase 3: Gateway**
- WebSocket server
- REST API
- Authentication integration
- Rate limiting

**Phase 4: Risk & Settlement**
- Risk engine service
- Balance management
- Settlement service
- Ledger system

**Phase 5: Market Data**
- Real-time data streaming
- Historical data storage
- OHLCV aggregation
- Market data APIs

**Phase 6: Operations**
- Admin dashboard
- Monitoring and alerting
- Deployment configurations
- Load testing framework

## Contact

For questions or collaboration, please open an issue on GitHub.
