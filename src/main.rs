use axum::{
    routing::get,
    Router,
    Json,
    Extension,
};
use std::{net::SocketAddr, sync::Arc};
use serde_json::Value;

use wallet_analyzer::modules::transactions::get_transactions;
use wallet_analyzer::modules::swaps::filter_and_name_swaps;
use wallet_analyzer::modules::prices::get_or_load_swaps_with_prices;
use wallet_analyzer::modules::pnl::calc_pnl;

use wallet_analyzer::modules::types::TradeWithPnl;

async fn get_trades(
    Extension(trades): Extension<Arc<Vec<TradeWithPnl>>>,
) -> Json<Value> {
    let value = serde_json::to_value(trades.as_ref()).unwrap_or(Value::Null);
    Json(value)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    
    let wallet_address = "KzNxNJvcieTvAF4bnfsuH1YEAXLHcB1cs468JA4K4QE";

    // 1. Fetch all transactions
    let transactions = get_transactions(&wallet_address).await.unwrap();
    println!("Total transactions fetched/loaded: {}", transactions.len());

    // 2. Filter and name swaps
    let named_swaps = filter_and_name_swaps(&transactions, &wallet_address)?;
    println!(
        "Total swaps with token names filtered/loaded: {}",
        named_swaps.len()
    );

    // 3. Get prices
    let priced_swaps = get_or_load_swaps_with_prices(&named_swaps, &wallet_address).await?;

    // 4. Calculate PnL
    let trades_with_pnl = Arc::new(calc_pnl(&priced_swaps, &wallet_address)?);

    // 5. Create Axum app
    let app = Router::new()
        .route("/api/trades", get(get_trades))
        .layer(Extension(trades_with_pnl));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("📡 Listening on http://{}", addr);
    hyper::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
