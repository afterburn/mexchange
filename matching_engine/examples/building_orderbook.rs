use matching_engine::{OrderBook, Side, Uuid};
use rust_decimal::Decimal;

fn main() {
    let mut ob = OrderBook::new();

    ob.add_limit_order(Uuid::new_v4(), Side::Bid, Decimal::from(99), Decimal::from(100));
    ob.add_limit_order(Uuid::new_v4(), Side::Bid, Decimal::from(98), Decimal::from(150));
    ob.add_limit_order(Uuid::new_v4(), Side::Bid, Decimal::from(97), Decimal::from(200));

    ob.add_limit_order(Uuid::new_v4(), Side::Ask, Decimal::from(101), Decimal::from(100));
    ob.add_limit_order(Uuid::new_v4(), Side::Ask, Decimal::from(102), Decimal::from(150));
    ob.add_limit_order(Uuid::new_v4(), Side::Ask, Decimal::from(103), Decimal::from(200));

    println!("Best bid: {:?}", ob.best_bid());
    println!("Best ask: {:?}", ob.best_ask());
    println!("Spread: {:?}", ob.spread());

    assert_eq!(ob.best_bid(), Some(Decimal::from(99)));
    assert_eq!(ob.best_ask(), Some(Decimal::from(101)));
    assert_eq!(ob.spread(), Some(Decimal::from(2)));
}
