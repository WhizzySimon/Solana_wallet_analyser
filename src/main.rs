use axum::{
    routing::get,
    Router,
    response::IntoResponse,
};
use std::{net::SocketAddr, fs};
use serde_json::Value;


use wallet_analyzer::modules::transactions::get_transactions;
use wallet_analyzer::modules::swaps::filter_and_name_swaps;
use wallet_analyzer::modules::prices::get_or_load_swaps_with_prices;
use wallet_analyzer::modules::pnl::calc_pnl;

async fn get_trades() -> impl IntoResponse {
    let data = fs::read_to_string("cache/trades_KzNxNJvcieTvAF4bnfsuH1YEAXLHcB1cs468JA4K4QE.json")
        .unwrap_or_else(|_| "[]".to_string());
    let parsed: Value = serde_json::from_str(&data).unwrap_or_else(|_| Value::Null);
    axum::Json(parsed)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let transactions = get_transactions()?;
    println!("Total transactions fetched/loaded: {}", transactions.len());

    let named_swaps = filter_and_name_swaps(&transactions)?;
    println!("Total swaps with token names filtered/loaded: {}", named_swaps.len());

    let priced_swaps = get_or_load_swaps_with_prices(&named_swaps);
    
    calc_pnl(&priced_swaps?)?;

        let app = Router::new().route("/api/trades", get(get_trades));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("📡 Listening on http://{}", addr);
    hyper::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    
    Ok(())
}
