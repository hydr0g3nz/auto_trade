// src/config.rs
use crate::domain::errors::{AppError, AppResult};
use dotenv::dotenv;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Trading bot configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Exchange API credentials
    pub exchange: ExchangeConfig,
    
    /// Trading configuration
    pub trading: TradingConfig,
    
    /// Risk management configuration
    pub risk: RiskConfig,
    
    /// Logging configuration
    pub logging: LoggingConfig,
}

/// Exchange API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    /// Exchange name (e.g., "binance")
    pub name: String,
    
    /// API key
    pub api_key: String,
    
    /// API secret
    pub api_secret: String,
    
    /// Use testnet
    pub testnet: bool,
}

/// Trading configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConfig {
    /// Trading symbols (e.g., ["BTCUSDT", "ETHUSDT"])
    pub symbols: Vec<String>,
    
    /// Default trading interval (e.g., "1m", "5m", "1h")
    pub interval: String,
    
    /// Active strategies
    pub strategies: Vec<StrategyConfig>,
    
    /// Auto trading enabled
    pub auto_trading: bool,
}

/// Strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    /// Strategy ID
    pub id: String,
    
    /// Strategy type
    pub strategy_type: String,
    
    /// Strategy parameters
    pub parameters: serde_json::Value,
}

/// Risk management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    /// Maximum position size in USDT
    pub max_position_size: Decimal,
    
    /// Maximum order size in USDT
    pub max_order_size: Decimal,
    
    /// Maximum daily loss in USDT
    pub max_daily_loss: Decimal,
    
    /// Stop loss percentage
    pub stop_loss_percent: Decimal,
    
    /// Take profit percentage
    pub take_profit_percent: Decimal,
    
    /// Maximum number of open positions
    pub max_open_positions: usize,
    
    /// Maximum number of trades per day
    pub max_trades_per_day: usize,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (e.g., "info", "debug", "warn", "error")
    pub level: String,
    
    /// Log to file
    pub to_file: bool,
    
    /// Log file path
    pub file_path: Option<String>,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> AppResult<Self> {
        // Load .env file if it exists
        dotenv().ok();
        
        // Create Exchange config
        let exchange_config = ExchangeConfig {
            name: env::var("EXCHANGE_NAME").unwrap_or_else(|_| "binance".to_string()),
            api_key: env::var("API_KEY").map_err(|_| {
                AppError::Config("Missing API_KEY environment variable".to_string())
            })?,
            api_secret: env::var("API_SECRET").map_err(|_| {
                AppError::Config("Missing API_SECRET environment variable".to_string())
            })?,
            testnet: env::var("USE_TESTNET")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
        };
        
        // Create Trading config
        let symbols = env::var("TRADING_SYMBOLS")
            .unwrap_or_else(|_| "BTCUSDT".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        
        let trading_config = TradingConfig {
            symbols,
            interval: env::var("TRADING_INTERVAL").unwrap_or_else(|_| "1m".to_string()),
            strategies: Vec::new(), // Will be loaded from config file
            auto_trading: env::var("AUTO_TRADING")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
        };
        
        // Create Risk config
        let risk_config = RiskConfig {
            max_position_size: env::var("MAX_POSITION_SIZE")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .unwrap_or(Decimal::new(1000, 0)),
            max_order_size: env::var("MAX_ORDER_SIZE")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .unwrap_or(Decimal::new(100, 0)),
            max_daily_loss: env::var("MAX_DAILY_LOSS")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .unwrap_or(Decimal::new(100, 0)),
            stop_loss_percent: env::var("STOP_LOSS_PERCENT")
                .unwrap_or_else(|_| "2".to_string())
                .parse()
                .unwrap_or(Decimal::new(2, 0)),
            take_profit_percent: env::var("TAKE_PROFIT_PERCENT")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .unwrap_or(Decimal::new(5, 0)),
            max_open_positions: env::var("MAX_OPEN_POSITIONS")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .unwrap_or(5),
            max_trades_per_day: env::var("MAX_TRADES_PER_DAY")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),
        };
        
        // Create Logging config
        let logging_config = LoggingConfig {
            level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            to_file: env::var("LOG_TO_FILE")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            file_path: env::var("LOG_FILE_PATH").ok(),
        };
        
        Ok(Config {
            exchange: exchange_config,
            trading: trading_config,
            risk: risk_config,
            logging: logging_config,
        })
    }
    
    /// Load configuration from a file
    pub fn from_file<P: AsRef<Path>>(path: P) -> AppResult<Self> {
        let mut file = File::open(path).map_err(|e| {
            AppError::Config(format!("Failed to open config file: {}", e))
        })?;
        
        let mut contents = String::new();
        file.read_to_string(&mut contents).map_err(|e| {
            AppError::Config(format!("Failed to read config file: {}", e))
        })?;
        
        let config: Config = serde_json::from_str(&contents).map_err(|e| {
            AppError::Config(format!("Failed to parse config file: {}", e))
        })?;
        
        Ok(config)
    }
    
    /// Save configuration to a file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> AppResult<()> {
        let contents = serde_json::to_string_pretty(self).map_err(|e| {
            AppError::Config(format!("Failed to serialize config: {}", e))
        })?;
        
        std::fs::write(path, contents).map_err(|e| {
            AppError::Config(format!("Failed to write config file: {}", e))
        })?;
        
        Ok(())
    }
    
    /// Initialize logging based on configuration
    pub fn init_logging(&self) -> AppResult<()> {
        let mut builder = env_logger::Builder::new();
        
        // Set log level
        let log_level = match self.logging.level.to_lowercase().as_str() {
            "trace" => log::LevelFilter::Trace,
            "debug" => log::LevelFilter::Debug,
            "info" => log::LevelFilter::Info,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Info,
        };
        
        builder.filter_level(log_level);
        
        // Configure output
        if self.logging.to_file {
            if let Some(file_path) = &self.logging.file_path {
                let file = File::create(file_path).map_err(|e| {
                    AppError::Config(format!("Failed to create log file: {}", e))
                })?;
                
                builder.target(env_logger::Target::Pipe(Box::new(file)));
            }
        }
        
        // Initialize the logger
        builder.init();
        
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            exchange: ExchangeConfig {
                name: "binance".to_string(),
                api_key: "".to_string(),
                api_secret: "".to_string(),
                testnet: true,
            },
            trading: TradingConfig {
                symbols: vec!["BTCUSDT".to_string()],
                interval: "1m".to_string(),
                strategies: Vec::new(),
                auto_trading: false,
            },
            risk: RiskConfig {
                max_position_size: Decimal::new(1000, 0),
                max_order_size: Decimal::new(100, 0),
                max_daily_loss: Decimal::new(100, 0),
                stop_loss_percent: Decimal::new(2, 0),
                take_profit_percent: Decimal::new(5, 0),
                max_open_positions: 5,
                max_trades_per_day: 10,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                to_file: false,
                file_path: None,
            },
        }
    }
}