use serde::{Deserialize, Serialize};

/// Configuration loaded from `config.toml`
#[derive(Debug, Deserialize)]
pub struct Settings {
    pub helius_api_key: String,
    pub use_cached_txns: Option<bool>,
    pub use_cached_named_swaps: Option<bool>,
    pub use_cached_priced_swaps: Option<bool>,
    pub use_token_cache: Option<bool>,
    pub use_jupiter_token_list: Option<bool>,
    pub fifo: Option<bool>,
    pub write_cache_files: Option<bool>,
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
    pub bought_mint: String,
    pub bought_token_name: String,
    pub bought_amount: f64,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PricedSwap {
    pub timestamp: u64,
    pub signature: String,
    pub sold_mint: String,
    pub sold_token_name: String,
    pub sold_amount: f64,
    pub bought_mint: String,
    pub bought_token_name: String,
    pub bought_amount: f64,
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
pub struct TradeWithPnl {
    pub sold_token: String,
    pub sold_amount: f64,
    pub received_usd: f64,
    pub cost_basis_usd: f64,
    pub profit_loss: f64,
    pub timestamp: u64,
    pub signature: String,
}