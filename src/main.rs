// src/main.rs
// Application entry point

use std::sync::Arc;
use tokio::sync::Mutex;
use env_logger::Builder;
use log::LevelFilter;

mod domain;
mod application;
mod infrastructure;
// mod interface;
mod config;

use crate::config::Config;
use crate::infrastructure::exchange::binance::BinanceExchangeRepository;
use crate::infrastructure::market::binance_market::BinanceMarketRepository;
use crate::infrastructure::strategy::basic_strategy::BasicTradingStrategy;
use crate::infrastructure::risk::basic_risk_manager::BasicRiskManager;
use crate::infrastructure::analysis::technical_analysis::TechnicalAnalysisImpl;
use crate::interface::coordinator::TradingCoordinator;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = Config::from_env()?;
    
    // Setup logging
    let log_level = match config.log_level.to_lowercase().as_str() {
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => LevelFilter::Info,
    };
    
    Builder::new()
        .filter(None, log_level)
        .init();
    
    log::info!("Starting Auto Trade System");
    log::info!("Trading symbols: {:?}", config.symbols);
    log::info!("Trading enabled: {}", config.trading_enabled);
    
    // Create repositories
    let exchange_repo = Arc::new(Mutex::new(BinanceExchangeRepository::new(
        config.api_key.clone(),
        config.api_secret.clone(),
    )));
    
    let market_repo = Arc::new(Mutex::new(BinanceMarketRepository::new(
        config.kline_interval,
    )));
    
    // Create services
    let technical_analysis = Arc::new(Mutex::new(TechnicalAnalysisImpl::new()));
    
    let risk_manager = Arc::new(Mutex::new(BasicRiskManager::new(
        config.risk_max_position_size,
        config.risk_max_drawdown_percent,
        config.risk_max_positions,
    )));
    
    // Create strategies for each symbol
    let mut strategies = Vec::new();
    for symbol in &config.symbols {
        strategies.push(Arc::new(Mutex::new(BasicTradingStrategy::default(
            technical_analysis.clone(),
            symbol.clone(),
        ))));
    }
    
    // Use the first strategy for now (in a real system, would need to handle multiple)
    let trading_strategy = strategies.first().unwrap().clone();
    
    // Create and start the trading coordinator
    let mut coordinator = TradingCoordinator::new(
        exchange_repo,
        market_repo,
        trading_strategy,
        risk_manager,
        technical_analysis,
        config.symbols.clone(),
    );
    
    coordinator.start().await?;
    
    // Keep the application running
    log::info!("Auto Trade System is running. Press Ctrl+C to exit.");
    
    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    
    log::info!("Shutdown signal received, stopping services...");
    coordinator.stop().await?;
    log::info!("Auto Trade System stopped.");
    
    Ok(())
}