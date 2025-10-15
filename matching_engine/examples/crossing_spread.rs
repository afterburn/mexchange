use matching_engine::{OrderBook, Side};
use rust_decimal::Decimal;

fn main() {
    let mut ob = OrderBook::new();

    ob.add_limit_order(Side::Bid, Decimal::from(99), Decimal::from(100));
    ob.add_limit_order(Side::Bid, Decimal::from(98), Decimal::from(150));
    ob.add_limit_order(Side::Bid, Decimal::from(97), Decimal::from(200));

    let result = ob.add_limit_order(Side::Ask, Decimal::from(98), Decimal::from(250));

    let total_filled: Decimal = result.fills.iter().map(|f| f.quantity).sum();
    println!("Order {} filled {} units across {} levels", result.order_id, total_filled, result.fills.len());

    for fill in &result.fills {
        println!("{} units @ {}", fill.quantity, fill.price);
    }

    println!("Best bid: {:?}", ob.best_bid());
    println!("Best ask: {:?}", ob.best_ask());

    assert_eq!(result.fills.len(), 2);
    assert_eq!(total_filled, Decimal::from(250));
    assert_eq!(ob.best_bid(), Some(Decimal::from(97)));
}
