// src/main.rs
mod analysis;
mod config;
mod domain;
mod exchange;
mod market_data;
mod trading;

use crate::config::Config;
use crate::domain::errors::{AppError, AppResult};
use crate::exchange::binance::BinanceClient;
use crate::exchange::client::ExchangeClient;
use crate::market_data::processor::MarketDataProcessor;
use crate::trading::execution::TradeExecutor;
use crate::trading::signals::SignalProcessor;
use crate::trading::strategies::{
    MACDStrategy, RSIStrategy, SMACrossoverStrategy, TradingStrategy,
};

use std::sync::Arc;
use tokio::signal::ctrl_c;
use tokio::sync::broadcast;
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() -> AppResult<()> {
    // Load configuration
    let config = Config::from_env()?;
    
    // Initialize logging
    config.init_logging()?;
    
    log::info!("Starting auto_trade v{}", env!("CARGO_PKG_VERSION"));
    log::info!("Using {} exchange", config.exchange.name);
    
    // Create exchange client
    let exchange_client = create_exchange_client(&config).await?;
    let exchange_client = Arc::new(exchange_client);
    
    // Connect to exchange
    log::info!("Connecting to exchange...");
    exchange_client.connect().await?;
    log::info!("Connected to exchange!");
    
    // Create market data processor
    let market_data = Arc::new(MarketDataProcessor::new());
    
    // Initialize data by fetching historical klines
    log::info!("Fetching initial market data...");
    for symbol in &config.trading.symbols {
        log::info!("Fetching data for {}/{}", symbol, config.trading.interval);
        let history = exchange_client
            .get_klines(symbol, &config.trading.interval, Some(100))
            .await?;
        
        market_data.add_price_history(history);
    }
    
    // Subscribe to real-time market data
    log::info!("Subscribing to real-time market data...");
    exchange_client
        .subscribe_to_market_data(&config.trading.symbols, Box::new(market_data.clone()))
        .await?;
    
    // Create signal processor
    log::info!("Initializing trading strategies...");
    let signal_processor = Arc::new(SignalProcessor::new(market_data.clone()));
    
    // Add trading strategies based on configuration
    for symbol in &config.trading.symbols {
        // Add RSI strategy
        signal_processor.add_strategy(
            &format!("rsi-{}", symbol),
            Box::new(RSIStrategy::new(symbol, 14, 70.0, 30.0)),
        );
        
        // Add MACD strategy
        signal_processor.add_strategy(
            &format!("macd-{}", symbol),
            Box::new(MACDStrategy::new(symbol, 12, 26, 9)),
        );
        
        // Add SMA Crossover strategy
        signal_processor.add_strategy(
            &format!("sma-{}", symbol),
            Box::new(SMACrossoverStrategy::new(symbol, 9, 21)),
        );
    }
    
    // Start signal processor
    log::info!("Starting signal processor...");
    signal_processor
        .start(config.trading.symbols.clone(), &config.trading.interval)
        .await?;
    
    // Create and start trade executor if auto-trading is enabled
    let trade_executor = Arc::new(TradeExecutor::new(exchange_client.clone(), market_data.clone()));
    
    if config.trading.auto_trading {
        log::info!("Auto-trading is enabled. Starting trade executor...");
        trade_executor.start().await?;
        
        // Subscribe to signals and execute trades
        let mut signal_rx = signal_processor.subscribe();
        
        tokio::spawn(async move {
            while let Ok(signal) = signal_rx.recv().await {
                log::info!("Received trading signal: {:?}", signal);
                
                match trade_executor.execute_signal(signal).await {
                    Ok(Some(response)) => {
                        log::info!("Order executed: {:?}", response);
                    }
                    Ok(None) => {
                        log::info!("Signal not executed");
                    }
                    Err(e) => {
                        log::error!("Failed to execute signal: {:?}", e);
                    }
                }
            }
        });
    } else {
        log::info!("Auto-trading is disabled. Trades will not be executed automatically.");
    }
    
    // Start position monitoring
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            
            // Log positions
            let positions = trade_executor.get_positions();
            if !positions.is_empty() {
                log::info!("=== Current Positions ===");
                for position in positions {
                    log::info!(
                        "{}: {} {} @ {}, PnL: {}",
                        position.symbol,
                        position.side,
                        position.quantity,
                        position.entry_price,
                        position.unrealized_pnl
                    );
                }
            }
            
            // Log daily PnL
            let daily_pnl = trade_executor.get_daily_pnl();
            log::info!("Daily PnL: {}", daily_pnl);
        }
    });
    
    // Wait for shutdown signal
    log::info!("Bot is running. Press Ctrl+C to stop.");
    ctrl_c().await.expect("Failed to listen for control-c event");
    
    // Shutdown
    log::info!("Shutting down...");
    signal_processor.stop();
    exchange_client.disconnect().await?;
    
    log::info!("Shutdown complete. Goodbye!");
    Ok(())
}

/// Create exchange client based on configuration
async fn create_exchange_client(config: &Config) -> AppResult<impl ExchangeClient> {
    match config.exchange.name.to_lowercase().as_str() {
        "binance" => {
            let client = BinanceClient::new(&config.exchange.api_key, &config.exchange.api_secret);
            Ok(client)
        }
        _ => Err(AppError::Config(format!(
            "Unsupported exchange: {}",
            config.exchange.name
        ))),
    }
}