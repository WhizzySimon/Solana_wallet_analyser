use serde::{Deserialize, Serialize};

/// Configuration loaded from `config.toml`
#[derive(Debug, Deserialize)]
pub struct Config {
    pub use_cached_txns: Option<bool>,
    pub use_cached_named_swaps: Option<bool>,
    pub use_cached_priced_swaps: Option<bool>,
    pub use_token_cache: Option<bool>,
    pub use_jupiter_token_list: Option<bool>,
    pub fifo: Option<bool>,
    pub write_cache_files: Option<bool>,
}

pub struct Settings {
    pub config: Config,
    pub helius_api_key: String,
    pub birdeye_api_key: String,
    pub wallet_address: String,
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
pub struct NamedSwap {
    pub timestamp: u64,
    pub signature: String,
    pub sold_mint: String,
    pub sold_token_name: String,
    pub sold_amount: f64,
    pub sold_decimals: Option<u8>,  
    pub bought_mint: String,
    pub bought_token_name: String,
    pub bought_amount: f64,
    pub bought_decimals: Option<u8>,
}



#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PricedSwap {
    pub timestamp: u64,
    pub signature: String,
    pub sold_mint: String,
    pub sold_token_name: String,
    pub sold_amount: f64,
    pub sold_decimals: Option<u8>,
    pub bought_mint: String,
    pub bought_token_name: String,
    pub bought_amount: f64,
    pub bought_decimals: Option<u8>,
    pub pricing_method: String,
    pub binance_sol_usd_price: Option<f64>,
    pub usd_value: Option<f64>,
}




#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RawTxn {
    pub signature: String,
    pub slot: u64,
    pub timestamp: Option<u64>,
    pub fee: u64,
    #[serde(rename = "feePayer")]
    pub fee_payer: String,
    pub description: String,
    #[serde(rename = "type")]
    pub txn_type: String,
    #[serde(rename = "nativeTransfers")]
    pub native_transfers: Vec<NativeTransfer>,
    #[serde(rename = "tokenTransfers")]
    pub token_transfers: Vec<TokenTransfer>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeTransfer {
    pub amount: i64,
    #[serde(rename = "fromUserAccount")]
    pub from_user_account: String,
    #[serde(rename = "toUserAccount")]
    pub to_user_account: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenTransfer {
    #[serde(rename = "fromUserAccount")]
    pub from_user_account: String,
    #[serde(rename = "toUserAccount")]
    pub to_user_account: String,
    pub mint: String,
    #[serde(rename = "tokenAmount")]
    pub token_amount: f64,
}

#[derive(Serialize)]
pub struct Trade {
    pub token_mint: String,
    pub token_name: String,
    pub buy_signature: String,
    pub sell_signature: String,
    pub buy_timestamp: u64,
    pub sell_timestamp: u64,
    pub amount: f64,
    pub cost_usd: f64,
    pub proceeds_usd: f64,
    pub pnl_usd: f64,
    pub holding_period_secs: u64,
}

#[derive(Debug, Clone)]
pub struct InventoryEntry {
    pub amount: f64,
    pub price_per_token: f64,
    pub total_usd: f64,
    pub timestamp: u64,
    pub signature: String,
}

#[derive(Debug, Serialize)]
pub struct TokenPnl {
    pub token: String,
    pub buys: Vec<BuyPart>,
    pub sells: Vec<SellPart>,
    pub realized_pnl: f64,
    pub total_bought: f64,
    pub total_sold: f64,
    pub remaining_amount: f64,
    pub average_cost_usd: f64,
}

#[derive(Debug, Serialize, Clone, Copy)]
pub struct BuyPart {
    pub timestamp: u64,
    pub amount: f64,
    pub cost_usd: f64,
}

#[derive(Debug, Serialize, Clone, Copy)]
pub struct SellPart {
    pub timestamp: u64,
    pub amount: f64,
    pub proceeds_usd: f64,
}

#[derive(Deserialize)]
pub struct PnlRequest {
    pub wallet_address: String,
}

