use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
    thread::sleep,
    time::Duration,
};

const INPUT_FILE: &str = "cache/swaps_KzNxNJvcieTvAF4bnfsuH1YEAXLHcB1cs468JA4K4QE.json";
const OUTPUT_FILE: &str = "output/swaps_with_sol_prices.json";
const SOL_MINT: &str = "So11111111111111111111111111111111111111112";

#[derive(Debug, Deserialize)]
struct SwapSummary {
    timestamp: u64,
    signature: String,
    sold_mint: String,
    sold_amount: f64,
    bought_mint: String,
    bought_amount: f64,
}

#[derive(Debug, Serialize)]
struct EnrichedSwapSummary {
    timestamp: u64,
    signature: String,
    sold_mint: String,
    sold_amount: f64,
    bought_mint: String,
    bought_amount: f64,
    usd_value: Option<f64>,
    pricing_method: String,
}

fn fetch_sol_price(timestamp: u64) -> Result<Option<f64>, Box<dyn std::error::Error>> {
    let from = timestamp - 300;
    let to = timestamp + 300;

    let url = format!(
        "https://api.coingecko.com/api/v3/coins/solana/market_chart/range?vs_currency=usd&from={}&to={}",
        from, to
    );

    println!("Requesting SOL/USD from CoinGecko: {}", url);

    let client = Client::new();
    let resp = client.get(&url).send()?;
    println!("Status: {}", resp.status());

    let text = resp.text()?;
    let json: Value = serde_json::from_str(&text)?;

    let Some(price_array) = json.get("prices").and_then(|v| v.as_array()) else {
        return Ok(None);
    };

    let closest = price_array.iter()
        .filter_map(|entry| {
            let ts_ms = entry.get(0)?.as_u64()?;
            let price = entry.get(1)?.as_f64()?;
            let ts_sec = ts_ms / 1000;
            Some((ts_sec, price))
        })
        .min_by_key(|(ts, _)| ts.abs_diff(timestamp));

    Ok(closest.map(|(_, price)| price))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw = fs::read_to_string(INPUT_FILE)?;
    let swaps: Vec<SwapSummary> = serde_json::from_str(&raw)?;

    let mut enriched = vec![];

    for swap in swaps {
        let mut method = String::new();
        let mut usd_value = None;

        let has_usd = swap.sold_mint.contains("USDC") || swap.bought_mint.contains("USDC")
                    || swap.sold_mint.contains("USDT") || swap.bought_mint.contains("USDT");

        let has_sol = swap.sold_mint == SOL_MINT || swap.bought_mint == SOL_MINT;

        if has_sol {
            if let Ok(Some(sol_price)) = fetch_sol_price(swap.timestamp) {
                if swap.sold_mint == SOL_MINT {
                    usd_value = Some(swap.sold_amount * sol_price);
                    method = "sold_to_sol_with_usd_rate".into();
                } else if swap.bought_mint == SOL_MINT {
                    usd_value = Some(swap.bought_amount * sol_price);
                    method = "bought_with_sol_with_usd_rate".into();
                } else {
                    method = "sol_involved_but_could_not_price".into();
                }
            } else {
                method = "no_sol_usd_price".into();
            }

            // avoid rate-limiting
            sleep(Duration::from_millis(1000));
        } else if !has_usd {
            method = "no_usd_or_sol".into();
        } else {
            method = "usd_pair_not_processed".into(); // for future
        }

        enriched.push(EnrichedSwapSummary {
            timestamp: swap.timestamp,
            signature: swap.signature,
            sold_mint: swap.sold_mint,
            sold_amount: swap.sold_amount,
            bought_mint: swap.bought_mint,
            bought_amount: swap.bought_amount,
            usd_value,
            pricing_method: method,
        });
    }

    let json = serde_json::to_string_pretty(&enriched)?;
    fs::create_dir_all(Path::new(OUTPUT_FILE).parent().unwrap())?;
    let mut file = File::create(OUTPUT_FILE)?;
    write!(file, "{}", json)?;

    println!("✅ Enriched swap data saved to {}", OUTPUT_FILE);

    Ok(())
}
