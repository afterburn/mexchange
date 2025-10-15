# matching_engine

High-performance matching engine library for cryptocurrency exchanges.

## Features

- Price-time priority matching
- Limit and market orders
- Partial fills across multiple price levels
- High-precision decimals (rust_decimal)
- Pure matching engine (no validation, networking, or storage)

## Design Philosophy

This matching engine is designed to be pure:

- No order validation (price limits, circuit breakers, etc.)
- No balance/margin checks
- No risk management
- Just fast, correct order matching

Validation and risk management should be handled in upstream services before orders reach the matching engine.

## Performance

|            | Ours            | NASDAQ           | CME Globex | Coinbase Pro  | Binance         |
| ---------- | --------------- | ---------------- | ---------- | ------------- | --------------- |
| Latency    | 105ns           | 14-40µs          | 52µs       | sub-50µs      | 5ms             |
| Throughput | 5.2M orders/sec | 100K+ orders/sec | -          | 2M orders/sec | 1.4M orders/sec |

Benchmarked on Apple Silicon.

## Architecture

- BTreeMap for price-sorted levels (O(log n))
- HashMap for O(1) order lookup
- VecDeque for FIFO within price levels
- Continuous matching

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
matching_engine = { path = "../matching_engine" }
rust_decimal = "1.35"
```

## Usage

### Basic Example

```rust
use matching_engine::{OrderBook, Side};
use rust_decimal::Decimal;

let mut ob = OrderBook::new();

ob.add_limit_order(Side::Bid, Decimal::from(100), Decimal::from(10));
ob.add_limit_order(Side::Ask, Decimal::from(101), Decimal::from(10));

println!("Spread: {:?}", ob.spread());

let result = ob.add_market_order(Side::Bid, Decimal::from(5));
println!("Filled: {} orders", result.fills.len());
```

### Limit Orders

```rust
let result = ob.add_limit_order(Side::Bid, Decimal::from(99), Decimal::from(50));
println!("Order ID: {}", result.order_id);
println!("Fills: {}", result.fills.len());
```

### Market Orders

```rust
let result = ob.add_market_order(Side::Ask, Decimal::from(25));
for fill in &result.fills {
    println!("{} units @ {}", fill.quantity, fill.price);
}
```

### Order Cancellation

```rust
let result = ob.add_limit_order(Side::Bid, Decimal::from(98), Decimal::from(100));
let order_id = result.order_id;

if ob.cancel_order(order_id) {
    println!("Order cancelled");
}
```

### Market Data

```rust
let best_bid = ob.best_bid();
let best_ask = ob.best_ask();
let spread = ob.spread();
let quantity = ob.quantity_at_price(Side::Bid, Decimal::from(100));
```

## API

- `add_limit_order(side, price, quantity) -> OrderResult`
- `add_market_order(side, quantity) -> OrderResult`
- `cancel_order(order_id) -> bool`
- `best_bid() -> Option<Price>`
- `best_ask() -> Option<Price>`
- `spread() -> Option<Price>`
- `quantity_at_price(side, price) -> Quantity`

## Types

- `OrderId` - u64
- `Price` - Decimal
- `Quantity` - Decimal
- `Side` - Bid | Ask
- `OrderType` - Limit | Market

## Examples

Run the included examples:

```bash
cargo run --example building_orderbook
cargo run --example market_orders
cargo run --example crossing_spread
cargo run --example partial_fills
cargo run --example order_cancellation
```

## Benchmarks

```bash
cargo bench
```

## Testing

```bash
cargo test
```

All 28 tests cover:

- Basic order placement and matching
- Partial fills across levels
- Price and time priority
- Order cancellation
- Edge cases and error conditions

## License

MIT OR Apache-2.0
