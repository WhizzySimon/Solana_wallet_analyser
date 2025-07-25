use axum::{
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use std::net::SocketAddr;
use wallet_analyzer::modules::transactions::get_transactions;
use wallet_analyzer::modules::swaps::filter_and_name_swaps;
use wallet_analyzer::modules::prices::get_or_load_swaps_with_prices;
use wallet_analyzer::modules::pnl::calc_pnl;
use wallet_analyzer::modules::types::{PnlRequest, TradeWithPnl};

/// Run the entire pipeline for a wallet and return enriched PnL trades
pub async fn run_pipeline(wallet_address: &str) -> Result<Vec<TradeWithPnl>, Box<dyn std::error::Error>> {
    let transactions = get_transactions(wallet_address).await.unwrap();
    println!("Total transactions fetched/loaded: {}", transactions.len());

    let named_swaps = filter_and_name_swaps(&transactions, wallet_address)?;
    println!("Total swaps with token names: {}", named_swaps.len());

    let priced_swaps = get_or_load_swaps_with_prices(&named_swaps, wallet_address).await?;
    let trades_with_pnl = calc_pnl(&priced_swaps, wallet_address)?;

    Ok(trades_with_pnl)
}

/// POST /api/pnl { "wallet": "..." } → returns { trades: [...] } or { error: ... }
async fn handle_pnl(Json(payload): Json<PnlRequest>) -> Json<Value> {
    let wallet_address = payload.wallet_address;
    match run_pipeline(&wallet_address).await {
        Ok(trades) => Json(json!({ "trades": trades })),
        Err(e) => {
            eprintln!("❌ Error: {e}");
            Json(json!({ "error": e.to_string() }))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new().route("/api/pnl", post(handle_pnl));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("📡 Listening on http://{}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
