use axum::{
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use std::net::SocketAddr;
use tower_http::cors::{CorsLayer, Any};
use wallet_analyzer::modules::transactions::get_transactions;
use wallet_analyzer::modules::swaps::filter_and_name_swaps;
use wallet_analyzer::modules::prices::get_or_load_swaps_with_prices;
use wallet_analyzer::modules::pnl::calc_pnl;
use wallet_analyzer::modules::types::{PnlRequest, TokenPnl, Settings};
use wallet_analyzer::modules::utils::{load_config};

/// Run the entire pipeline for a wallet and return enriched PnL trades
pub async fn run_pipeline(wallet_address: String) -> Result<Vec<TokenPnl>, Box<dyn std::error::Error>> {
    let config = load_config().map_err(|e| format!("Failed to load config: {}", e))?;
    
    dotenvy::dotenv().ok(); // loads .env if available
    let helius_api_key = std::env::var("helius_api_key").expect("Missing helius_api_key");
    println!("🔑 Using Helius key: {}", helius_api_key);
    let birdeye_api_key = std::env::var("birdeye_api_key").expect("Missing birdeye_api_key");
    println!("🔑 Using birdeye key: {}", birdeye_api_key);
    let settings = Settings {
        config,
        helius_api_key,
        birdeye_api_key,
        wallet_address,
    };
    
    let transactions = match get_transactions(&settings).await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("❌ Failed to get transactions: {e}");
            return Err(e);
        }
    };
    println!("Total transactions fetched/loaded: {}", transactions.len());

    let named_swaps = filter_and_name_swaps(&transactions, &settings).await?;
    println!("Total swaps with token names: {}", named_swaps.len());

    let priced_swaps = get_or_load_swaps_with_prices(&named_swaps, &settings).await?;

    let trades_with_pnl = calc_pnl(&priced_swaps, &settings).await?;

    Ok(trades_with_pnl)
}

/// POST /api/pnl { "wallet": "..." } → returns { trades: [...] } or { error: ... }
async fn handle_pnl(Json(payload): Json<PnlRequest>) -> Json<Value> {
    let wallet_address = payload.wallet_address;
    match run_pipeline(wallet_address).await {
        Ok(trades) => Json(json!({ "trades": trades })),
        Err(e) => {
            eprintln!("❌ Error: {e}");
            Json(json!({ "error": e.to_string() }))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/api/pnl", post(handle_pnl))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );


    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("📡 Listening on http://{}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
