use matching_engine::{OrderBook, Side};
use rust_decimal::Decimal;

fn main() {
    let mut ob = OrderBook::new();

    ob.add_limit_order(Side::Ask, Decimal::from(100), Decimal::from(50));

    let result = ob.add_market_order(Side::Bid, Decimal::from(100));
    let filled: Decimal = result.fills.iter().map(|f| f.quantity).sum();

    println!("Requested: 100 units");
    println!("Filled: {} units", filled);
    println!("Unfilled: {} units", Decimal::from(100) - filled);

    assert_eq!(filled, Decimal::from(50));
    assert_eq!(ob.quantity_at_price(Side::Ask, Decimal::from(100)), Decimal::from(0));

    let mut ob2 = OrderBook::new();

    ob2.add_limit_order(Side::Ask, Decimal::from(100), Decimal::from(30));
    ob2.add_limit_order(Side::Ask, Decimal::from(101), Decimal::from(30));
    ob2.add_limit_order(Side::Ask, Decimal::from(102), Decimal::from(30));

    let result2 = ob2.add_market_order(Side::Bid, Decimal::from(50));

    println!("Multi-level fill:");
    for fill in &result2.fills {
        println!("{} units @ {}", fill.quantity, fill.price);
    }
}
