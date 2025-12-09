mod market_maker;
mod aggressive;
mod random;
mod mean_reversion;

pub use market_maker::MarketMaker;
pub use aggressive::Aggressive;
pub use random::Random;
pub use mean_reversion::MeanReversion;

use crate::types::{MarketState, OrderRequest};

pub trait Strategy: Send {
    fn name(&self) -> &'static str;
    fn generate_orders(&mut self, market: &MarketState, symbol: &str) -> Vec<OrderRequest>;
    fn interval_ms(&self) -> u64;
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum StrategyType {
    MarketMaker,
    Aggressive,
    Random,
    MeanReversion,
}

impl StrategyType {
    pub fn create(&self) -> Box<dyn Strategy> {
        match self {
            StrategyType::MarketMaker => Box::new(MarketMaker::new()),
            StrategyType::Aggressive => Box::new(Aggressive::new()),
            StrategyType::Random => Box::new(Random::new()),
            StrategyType::MeanReversion => Box::new(MeanReversion::new()),
        }
    }
}
