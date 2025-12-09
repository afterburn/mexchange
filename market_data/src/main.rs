use axum::{routing::get, Router};
use futures_util::StreamExt;
use rdkafka::{
    consumer::{stream_consumer::StreamConsumer, Consumer},
    Message as KafkaMessage,
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tracing::{error, info};

mod events;
mod ohlcv;

use events::MarketEvent;
use ohlcv::OhlcvAggregator;

fn main() {
    use std::io::Write;

    std::panic::set_hook(Box::new(|panic_info| {
        let _ = std::io::stderr().write_all(format!("PANIC: {:?}\n", panic_info).as_bytes());
        let _ = std::io::stderr().flush();
        std::process::exit(1);
    }));

    let _ = std::io::stderr().write_all(b"Starting market data service...\n");
    let _ = std::io::stderr().flush();

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let _ =
                std::io::stderr().write_all(format!("Failed to create runtime: {}\n", e).as_bytes());
            let _ = std::io::stderr().flush();
            std::process::exit(1);
        }
    };

    match rt.block_on(tokio_main()) {
        Ok(_) => {}
        Err(e) => {
            let _ = std::io::stderr().write_all(format!("Error: {}\n", e).as_bytes());
            let _ = std::io::stderr().flush();
            std::process::exit(1);
        }
    }
}

async fn tokio_main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "market_data=info".into()),
        )
        .init();

    info!("Starting market data service...");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/accounts".to_string());
    let kafka_brokers =
        std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string());
    let kafka_topic =
        std::env::var("KAFKA_MARKET_EVENTS_TOPIC").unwrap_or_else(|_| "market-events".to_string());
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3002".to_string());

    info!("Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    info!("Connected to database");

    let aggregator = Arc::new(OhlcvAggregator::new(pool));

    start_kafka_consumer(&kafka_brokers, &kafka_topic, aggregator.clone()).await?;

    let app = Router::new().route("/health", get(health));

    info!("Market data service listening on {}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn start_kafka_consumer(
    brokers: &str,
    topic: &str,
    aggregator: Arc<OhlcvAggregator>,
) -> anyhow::Result<()> {
    let brokers = brokers.to_string();
    let topic = topic.to_string();

    tokio::spawn(async move {
        loop {
            info!("Connecting to Kafka consumer...");
            let consumer: StreamConsumer = match rdkafka::config::ClientConfig::new()
                .set("bootstrap.servers", &brokers)
                .set("group.id", "market-data")
                .set("enable.partition.eof", "false")
                .set("session.timeout.ms", "6000")
                .set("enable.auto.commit", "true")
                .set("auto.offset.reset", "latest")
                .create()
            {
                Ok(c) => c,
                Err(e) => {
                    error!("Failed to create Kafka consumer: {}, retrying in 3s...", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                    continue;
                }
            };

            if let Err(e) = consumer.subscribe(&[&topic]) {
                error!("Failed to subscribe to topic: {}, retrying in 3s...", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                continue;
            }

            info!("Subscribed to Kafka topic: {} for market events", topic);

            let mut message_stream = consumer.stream();
            loop {
                match message_stream.next().await {
                    Some(Ok(msg)) => {
                        if let Some(payload) = msg.payload() {
                            match serde_json::from_slice::<MarketEvent>(payload) {
                                Ok(MarketEvent::Fill {
                                    symbol,
                                    price,
                                    quantity,
                                    timestamp,
                                    ..
                                }) => {
                                    info!(
                                        "Processing fill: {} @ {} x {}",
                                        symbol, price, quantity
                                    );
                                    if let Err(e) = aggregator
                                        .process_trade(&symbol, price, quantity, timestamp)
                                        .await
                                    {
                                        error!("Failed to process trade: {}", e);
                                    }
                                }
                                Ok(MarketEvent::Unknown) => {
                                    // Ignore other event types
                                }
                                Err(e) => {
                                    error!("Failed to deserialize market event: {}", e);
                                }
                            }
                        }
                    }
                    Some(Err(e)) => {
                        error!("Kafka consumer error: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                    None => {
                        error!("Kafka stream ended, reconnecting...");
                        break;
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        }
    });

    Ok(())
}
