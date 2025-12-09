use matching_engine::{OrderBook, Side, Uuid};
use rust_decimal::Decimal;

fn main() {
    let mut ob = OrderBook::new();

    let order_id = Uuid::new_v4();
    let result1 = ob.add_limit_order(order_id, Side::Bid, Decimal::from(100), Decimal::from(50));
    let _result2 = ob.add_limit_order(Uuid::new_v4(), Side::Bid, Decimal::from(99), Decimal::from(75));
    let _result3 = ob.add_limit_order(Uuid::new_v4(), Side::Ask, Decimal::from(101), Decimal::from(60));

    println!("Best bid: {:?}", ob.best_bid());
    println!("Best ask: {:?}", ob.best_ask());
    println!("Spread: {:?}", ob.spread());

    let cancelled = ob.cancel_order(result1.order_id);
    println!("Cancelled order {}: {}", result1.order_id, cancelled);

    println!("Best bid after cancellation: {:?}", ob.best_bid());

    assert!(cancelled);
    assert_eq!(ob.best_bid(), Some(Decimal::from(99)));

    let non_existent_id = Uuid::new_v4();
    let failed = ob.cancel_order(non_existent_id);
    println!("Cancelled non-existent order {}: {}", non_existent_id, failed);

    assert!(!failed);
}
