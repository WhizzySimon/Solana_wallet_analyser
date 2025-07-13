use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{self, File},
    io::Write,
    time::Duration,
};

const INPUT_FILE: &str = "cache/swaps_KzNxNJvcieTvAF4bnfsuH1YEAXLHcB1cs468JA4K4QE.json";
const OUTPUT_FILE: &str = "output/swaps_with_sol_prices_binance.json";
const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
const MIN_BINANCE_INTERVAL: u64 = 300; // 5 minutes in seconds

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

fn fetch_sol_price_binance(timestamp: u64) -> Result<Option<f64>, Box<dyn std::error::Error>> {
    let from = (timestamp.saturating_sub(MIN_BINANCE_INTERVAL)) * 1000;
    let to = (timestamp + MIN_BINANCE_INTERVAL) * 1000;

    let url = format!(
        "https://api.binance.com/api/v3/klines?symbol=SOLUSDT&interval=1m&startTime={}&endTime={}",
        from, to
    );

    println!("Requesting Binance SOL price from {} to {}", from, to);
    let client = Client::new();
    let resp = client.get(&url).send()?;
    println!("Status: {}", resp.status());

    let body = resp.text()?;
    let candles: Vec<Value> = serde_json::from_str(&body)?;

    let target_ts = timestamp * 1000;
    let mut closest: Option<(u64, f64)> = None;

    for entry in candles {
        let open_time = entry[0].as_u64().unwrap_or(0);
        let close_price = entry[4].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);

        if closest.is_none()
            || (open_time as i64 - target_ts as i64).abs()
                < (closest.unwrap().0 as i64 - target_ts as i64).abs()
        {
            closest = Some((open_time, close_price));
        }
    }

    Ok(closest.map(|(_, price)| price))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw = fs::read_to_string(INPUT_FILE)?;
    let swaps: Vec<SwapSummary> = serde_json::from_str(&raw)?;

    let mut enriched: Vec<EnrichedSwapSummary> = vec![];

    for swap in swaps {
        let (usd_value, pricing_method) = if swap.sold_mint == SOL_MINT {
            match fetch_sol_price_binance(swap.timestamp)? {
                Some(price) => (Some(swap.sold_amount * price), "sold_to_usd_binance".into()),
                None => (None, "no_price".into()),
            }
        } else if swap.bought_mint == SOL_MINT {
            match fetch_sol_price_binance(swap.timestamp)? {
                Some(price) => (Some(swap.bought_amount * price), "bought_with_usd_binance".into()),
                None => (None, "no_price".into()),
            }
        } else {
            (None, "not_a_sol_trade".into())
        };

        enriched.push(EnrichedSwapSummary {
            timestamp: swap.timestamp,
            signature: swap.signature,
            sold_mint: swap.sold_mint,
            sold_amount: swap.sold_amount,
            bought_mint: swap.bought_mint,
            bought_amount: swap.bought_amount,
            usd_value,
            pricing_method,
        });

        std::thread::sleep(Duration::from_millis(200));
    }

    fs::create_dir_all("output")?;
    let mut file = File::create(OUTPUT_FILE)?;
    write!(file, "{}", serde_json::to_string_pretty(&enriched)?)?;
    println!("✅ Saved enriched swaps to {}", OUTPUT_FILE);

    Ok(())
}
