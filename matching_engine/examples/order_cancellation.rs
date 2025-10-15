use matching_engine::{OrderBook, Side};
use rust_decimal::Decimal;

fn main() {
    let mut ob = OrderBook::new();

    let result1 = ob.add_limit_order(Side::Bid, Decimal::from(100), Decimal::from(50));
    let _result2 = ob.add_limit_order(Side::Bid, Decimal::from(99), Decimal::from(75));
    let _result3 = ob.add_limit_order(Side::Ask, Decimal::from(101), Decimal::from(60));

    println!("Best bid: {:?}", ob.best_bid());
    println!("Best ask: {:?}", ob.best_ask());
    println!("Spread: {:?}", ob.spread());

    let cancelled = ob.cancel_order(result1.order_id);
    println!("Cancelled order {}: {}", result1.order_id, cancelled);

    println!("Best bid after cancellation: {:?}", ob.best_bid());

    assert!(cancelled);
    assert_eq!(ob.best_bid(), Some(Decimal::from(99)));

    let failed = ob.cancel_order(999);
    println!("Cancelled non-existent order 999: {}", failed);

    assert!(!failed);
}
