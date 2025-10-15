use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use matching_engine::{OrderBook, Side};
use rust_decimal::Decimal;
use std::time::Duration;

// Simulates realistic exchange behavior with mixed order types
fn simulate_exchange_orders(ob: &mut OrderBook, order_count: usize) {
    let base_price = 50000;
    let mut order_ids = Vec::new();

    for i in 0..order_count {
        // 70% limit orders, 30% market orders (realistic exchange ratio)
        if i % 10 < 7 {
            // Limit order
            let side = if i % 2 == 0 { Side::Bid } else { Side::Ask };
            let price_offset = (i % 10) as i64 - 5;
            let price = Decimal::from(base_price + price_offset);
            let quantity = Decimal::from(((i % 5) + 1) as i64);

            let result = ob.add_limit_order(side, price, quantity);
            order_ids.push(result.order_id);

            // Cancel 10% of limit orders to simulate real behavior
            if i % 10 == 0 && !order_ids.is_empty() {
                let cancel_idx = i % order_ids.len();
                ob.cancel_order(order_ids[cancel_idx]);
            }
        } else {
            // Market order
            let side = if i % 2 == 0 { Side::Bid } else { Side::Ask };
            let quantity = Decimal::from(((i % 3) + 1) as i64);
            ob.add_market_order(side, quantity);
        }
    }
}

fn bench_mixed_order_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("exchange_simulation");

    for order_count in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*order_count as u64));

        group.bench_with_input(
            format!("{}_orders", order_count),
            order_count,
            |b, &count| {
                b.iter(|| {
                    let mut ob = OrderBook::new();
                    simulate_exchange_orders(black_box(&mut ob), black_box(count));
                });
            },
        );
    }

    group.finish();
}

fn bench_limit_order_placement(c: &mut Criterion) {
    let mut group = c.benchmark_group("limit_orders");

    group.bench_function("place_limit_order", |b| {
        let mut ob = OrderBook::new();
        let price = Decimal::from(50000);
        let quantity = Decimal::from(1);

        b.iter(|| {
            ob.add_limit_order(black_box(Side::Bid), black_box(price), black_box(quantity));
        });
    });

    group.finish();
}

fn bench_market_order_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("market_orders");

    group.bench_function("execute_market_order", |b| {
        let quantity = Decimal::from(5);

        b.iter_batched(
            || {
                let mut ob = OrderBook::new();
                // Build book with liquidity
                for i in 0..10 {
                    ob.add_limit_order(
                        Side::Ask,
                        Decimal::from(50000 + i),
                        Decimal::from(10),
                    );
                }
                ob
            },
            |mut ob| {
                ob.add_market_order(black_box(Side::Bid), black_box(quantity));
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_deep_book_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("deep_book");

    group.bench_function("match_across_levels", |b| {
        b.iter_batched(
            || {
                let mut ob = OrderBook::new();
                // Build deep book
                for i in 0..100 {
                    ob.add_limit_order(
                        Side::Ask,
                        Decimal::from(50000 + i),
                        Decimal::from(100),
                    );
                }
                ob
            },
            |mut ob| {
                // Large market order that crosses many levels
                ob.add_market_order(black_box(Side::Bid), black_box(Decimal::from(5000)));
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_high_frequency_trading(c: &mut Criterion) {
    let mut group = c.benchmark_group("high_frequency");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("hft_simulation", |b| {
        b.iter(|| {
            let mut ob = OrderBook::new();
            let base_price = Decimal::from(50000);

            // Simulate HFT: rapid order placement and cancellation
            for i in 0..1000 {
                let price_offset = ((i % 10) as i64 - 5) / 10;
                let price = base_price + Decimal::from(price_offset);
                let quantity = Decimal::from(1);

                let result = ob.add_limit_order(Side::Bid, price, quantity);

                // Cancel immediately (HFT behavior)
                if i % 3 == 0 {
                    ob.cancel_order(result.order_id);
                }
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_mixed_order_throughput,
    bench_limit_order_placement,
    bench_market_order_execution,
    bench_deep_book_matching,
    bench_high_frequency_trading
);

criterion_main!(benches);
