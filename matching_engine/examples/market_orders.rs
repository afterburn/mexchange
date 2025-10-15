use matching_engine::{OrderBook, Side};
use rust_decimal::Decimal;

fn main() {
    let mut ob = OrderBook::new();

    ob.add_limit_order(Side::Ask, Decimal::from(101), Decimal::from(100));
    ob.add_limit_order(Side::Ask, Decimal::from(102), Decimal::from(150));
    ob.add_limit_order(Side::Ask, Decimal::from(103), Decimal::from(200));

    let result = ob.add_market_order(Side::Bid, Decimal::from(120));

    println!("Order ID: {}", result.order_id);
    for fill in &result.fills {
        println!("{} units @ {}", fill.quantity, fill.price);
    }

    let total_filled: Decimal = result.fills.iter().map(|f| f.quantity).sum();
    println!("Total filled: {}", total_filled);

    assert_eq!(result.fills.len(), 2);
    assert_eq!(result.fills[0].price, Decimal::from(101));
    assert_eq!(result.fills[0].quantity, Decimal::from(100));
    assert_eq!(result.fills[1].price, Decimal::from(102));
    assert_eq!(result.fills[1].quantity, Decimal::from(20));
}
