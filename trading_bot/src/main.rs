use clap::Parser;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use trading_bot::{GatewayClient, StrategyType};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Parser, Debug)]
#[command(name = "trading_bot")]
#[command(about = "Automated trading bot for mexchange")]
struct Args {
    /// Gateway host
    #[arg(long, default_value = "localhost")]
    gateway_host: String,

    /// Gateway port
    #[arg(long, default_value_t = 3000)]
    gateway_port: u16,

    /// Trading symbol
    #[arg(long, default_value = "KCN/EUR")]
    symbol: String,

    /// Strategies to run (can specify multiple)
    #[arg(long, value_enum, num_args = 1..)]
    strategies: Vec<StrategyType>,

    /// Run all strategies
    #[arg(long, default_value_t = false)]
    all: bool,
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    let strategies: Vec<StrategyType> = if args.all {
        vec![
            StrategyType::MarketMaker,
            StrategyType::Aggressive,
            StrategyType::Random,
            StrategyType::MeanReversion,
        ]
    } else if args.strategies.is_empty() {
        warn!("No strategies specified, defaulting to MarketMaker");
        vec![StrategyType::MarketMaker]
    } else {
        args.strategies
    };

    info!(
        "Starting trading bot with strategies: {:?}",
        strategies.iter().map(|s| format!("{:?}", s)).collect::<Vec<_>>()
    );
    info!("Gateway: {}:{}", args.gateway_host, args.gateway_port);
    info!("Symbol: {}", args.symbol);

    let client = Arc::new(GatewayClient::new(&args.gateway_host, args.gateway_port));

    // Connect to WebSocket for market data
    if let Err(e) = client.connect_websocket().await {
        error!("Failed to connect to WebSocket: {}", e);
        return Err(e);
    }

    info!("Connected to gateway WebSocket");

    // Give WebSocket time to receive initial data
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Spawn a task for each strategy
    let mut handles = Vec::new();

    for strategy_type in strategies {
        let client = Arc::clone(&client);
        let symbol = args.symbol.clone();
        let market_state = client.market_state();

        let handle = tokio::spawn(async move {
            run_strategy(strategy_type, client, market_state, symbol).await;
        });

        handles.push(handle);
    }

    // Wait for all strategies (they run forever unless error)
    for handle in handles {
        if let Err(e) = handle.await {
            error!("Strategy task failed: {}", e);
        }
    }

    Ok(())
}

async fn run_strategy(
    strategy_type: StrategyType,
    client: Arc<GatewayClient>,
    market_state: Arc<RwLock<trading_bot::MarketState>>,
    symbol: String,
) {
    let mut strategy = strategy_type.create();
    let interval = Duration::from_millis(strategy.interval_ms());

    info!(
        "Starting {} strategy (interval: {}ms)",
        strategy.name(),
        strategy.interval_ms()
    );

    loop {
        // Get current market state
        let state = market_state.read().await.clone();

        // Generate orders
        let orders = strategy.generate_orders(&state, &symbol);

        if !orders.is_empty() {
            info!(
                "[{}] Generated {} orders (mid={:?})",
                strategy.name(),
                orders.len(),
                state.mid_price()
            );

            // Submit orders
            for order in &orders {
                if let Err(e) = client.submit_order(order).await {
                    warn!("[{}] Failed to submit order: {}", strategy.name(), e);
                }
            }
        }

        tokio::time::sleep(interval).await;
    }
}
