use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
};

#[derive(Debug, Deserialize)]
struct Swap {
    timestamp: u64,
    signature: String,
    sold_mint: String,
    sold_amount: f64,
    bought_mint: String,
    bought_amount: f64,
}

#[derive(Debug, Serialize)]
struct PricedSwap {
    timestamp: u64,
    signature: String,
    sold_mint: String,
    sold_amount: f64,
    bought_mint: String,
    bought_amount: f64,
    usd_value: f64,
    pricing_method: String,
}

const SOLANA_MINT: &str = "So11111111111111111111111111111111111111112";
const BINANCE_SYMBOL: &str = "SOLUSDT";

fn load_sol_swaps(path: &str) -> Vec<Swap> {
    let content = fs::read_to_string(path).expect("failed to read file");
    let all: Vec<Swap> = serde_json::from_str(&content).expect("failed to parse JSON");
    all.into_iter()
        .filter(|s| s.sold_mint == SOLANA_MINT || s.bought_mint == SOLANA_MINT)
        .collect()
}
fn group_by_time(swaps: &[Swap]) -> Vec<Vec<&Swap>> {
    const MAX_GROUP_SPAN: u64 = 86400; // 1 day

    let mut sorted = swaps.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|s| s.timestamp);

    let mut groups = vec![];
    let mut current_group = vec![sorted[0]];
    let mut start = sorted[0].timestamp;

    for s in sorted.iter().skip(1) {
        let span = s.timestamp - start;
        if span > MAX_GROUP_SPAN {
            groups.push(current_group);
            current_group = vec![*s];
            start = s.timestamp;
        } else {
            current_group.push(*s);
        }
    }

    if !current_group.is_empty() {
        groups.push(current_group);
    }

    groups
}

fn fetch_binance_price(client: &Client, ts: u64) -> Option<f64> {
    let end = ts * 1000;
    let start = end - 60_000;
    let url = format!(
        "https://api.binance.com/api/v3/klines?symbol={}&interval=1m&startTime={}&endTime={}",
        BINANCE_SYMBOL, start, end
    );
    let res = client.get(&url).send().ok()?;
    let data: Vec<Vec<serde_json::Value>> = res.json().ok()?;
    if let Some(first) = data.first() {
        first.get(4)?.as_str()?.parse().ok() // closing price
    } else {
        None
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let swaps = load_sol_swaps("cache/swaps_KzNxNJvcieTvAF4bnfsuH1YEAXLHcB1cs468JA4K4QE.json");

    println!("Total SOL swaps: {}", swaps.len());
    let groups = group_by_time(&swaps);
    println!("Grouped into {} time buckets", groups.len());

    let mut enriched = vec![];

    for (i, group) in groups.iter().enumerate() {
        let first_ts = group.first().unwrap().timestamp;
        let last_ts = group.last().unwrap().timestamp;
        let from_utc: DateTime<Utc> = DateTime::<Utc>::from_timestamp(first_ts as i64, 0).unwrap();
        let to_utc: DateTime<Utc> = DateTime::<Utc>::from_timestamp(last_ts as i64, 0).unwrap();
        let span = last_ts - first_ts;
        println!(
            "Group {}: {} → {} ({} swaps, {} min)",
            i + 1,
            from_utc,
            to_utc,
            group.len(),
            span / 60
        );

        for s in group {
            let price = fetch_binance_price(&client, s.timestamp);
            let (usd_value, method) = match (price, s.sold_mint.as_str(), s.bought_mint.as_str()) {
                (Some(p), mint, _) if mint == SOLANA_MINT => (p * s.sold_amount, "sold_to_usd_binance"),
                (Some(p), _, mint) if mint == SOLANA_MINT => (p * s.bought_amount, "bought_with_usd_binance"),
                _ => (0.0, "price_unavailable"),
            };

            enriched.push(PricedSwap {
                timestamp: s.timestamp,
                signature: s.signature.clone(),
                sold_mint: s.sold_mint.clone(),
                sold_amount: s.sold_amount,
                bought_mint: s.bought_mint.clone(),
                bought_amount: s.bought_amount,
                usd_value,
                pricing_method: method.to_string(),
            });
        }
    }

    fs::create_dir_all("output")?;
    let mut f = File::create("output/swaps_with_sol_prices_binance.json")?;
    write!(f, "{}", serde_json::to_string_pretty(&enriched)?)?;
    println!("✅ Saved enriched swaps to output/swaps_with_sol_prices_binance.json");
    Ok(())
}
