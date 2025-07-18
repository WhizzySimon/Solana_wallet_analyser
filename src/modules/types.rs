use serde::{Deserialize, Serialize};

/// Configuration loaded from `config.toml`
#[derive(Debug, Deserialize)]
pub struct Settings {
    pub helius_api_key: String,
    pub wallet_address: String,
    pub use_cached_txns: Option<bool>,
    pub use_cached_swaps: Option<bool>,
    pub use_token_cache: Option<bool>,
    pub use_jupiter_token_list: Option<bool>,
}

/// Minimal raw swap structure parsed from transactions
#[derive(Debug, Serialize, Deserialize)]
pub struct Swap {
    pub timestamp: u64,
    pub signature: String,
    pub sold_mint: String,
    pub sold_amount: f64,
    pub bought_mint: String,
    pub bought_amount: f64,
}

/// Final swap structure including resolved token names
#[derive(Debug, Serialize, Deserialize)]
pub struct SwapWithTokenNames {
    pub timestamp: u64,
    pub signature: String,
    pub sold_mint: String,
    pub sold_token_name: String,
    pub sold_amount: f64,
    pub bought_mint: String,
    pub bought_token_name: String,
    pub bought_amount: f64,
}


#[derive(Debug, Serialize, Deserialize)]
pub struct PricedSwap {
    pub timestamp: u64,
    pub signature: String,
    pub sold_mint: String,
    pub sold_token_name: String,
    pub sold_amount: f64,
    pub bought_mint: String,
    pub bought_token_name: String,
    pub bought_amount: f64,
    pub usd_value: f64,
    pub pricing_method: String,
}
