use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::time::Duration;
use config::Config;
use chrono::{Utc, TimeZone};

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
    const MAX_GROUP_SPAN: u64 = 86400;

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

fn fetch_price_map_for_range(client: &Client, start_ts: u64, end_ts: u64) -> HashMap<u64, f64> {
    let url = format!(
        "https://api.binance.com/api/v3/klines?symbol={}&interval=1m&startTime={}&endTime={}",
        BINANCE_SYMBOL,
        start_ts * 1000,
        end_ts * 1000
    );

    println!("Requesting Binance Klines: {}", url);

    let resp = client
        .get(&url)
        .timeout(Duration::from_secs(10))
        .send()
        .expect("failed to send request")
        .json::<Vec<Vec<serde_json::Value>>>()
        .expect("failed to parse response");

    let mut map = HashMap::new();
    for entry in resp {
        if let (Some(open_time), Some(close_price)) = (
            entry.get(0).and_then(|v| v.as_u64()),
            entry.get(4).and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()),
        ) {
            map.insert(open_time / 1000, close_price);
        }
    }
    map
}

fn enrich_cache_swaps_with_sol_prices(swaps_path: &str, priced_swaps: &[PricedSwap]) {
    let content = fs::read_to_string(swaps_path).expect("failed to read raw swap file");
    let mut swaps: Vec<Value> = serde_json::from_str(&content).expect("failed to parse JSON");

    let price_map: HashMap<String, &PricedSwap> = priced_swaps
        .iter()
        .map(|p| (p.signature.clone(), p))
        .collect();

    for swap in swaps.iter_mut() {
        if let Some(sig) = swap.get("signature").and_then(|s| s.as_str()) {
            if let Some(p) = price_map.get(sig) {
                let sol_amount = if p.sold_mint == SOLANA_MINT {
                    p.sold_amount
                } else {
                    p.bought_amount
                };
                let sol_price = p.usd_value / sol_amount;

                swap["binance_sol_usd_price"] = serde_json::json!(sol_price);
                swap["pricing_method"] = serde_json::json!(p.pricing_method);
            } else {
                swap["pricing_method"] = serde_json::json!("unverified");
            }
        } else {
            swap["pricing_method"] = serde_json::json!("unverified");
        }
    }

    let json = serde_json::to_string_pretty(&swaps).unwrap();
    fs::write("output/swaps_enriched_with_sol_price.json", json).unwrap();
}

fn main() {
    let settings = Config::builder()
        .add_source(config::File::with_name("config/config"))
        .build()
        .unwrap();
    let swaps_path: String = settings.get("swaps_path").unwrap();

    let swaps = load_sol_swaps(&swaps_path);
    let groups = group_by_time(&swaps);

    println!("{:<6} | {:<20} | {:<20} | {}", "Group", "Start Time", "End Time", "Swaps");
    println!("{}", "-".repeat(65));

    for (i, group) in groups.iter().enumerate() {
        let start_ts = group.first().unwrap().timestamp;
        let end_ts = group.last().unwrap().timestamp;
        let start_dt = Utc.timestamp_opt(start_ts as i64, 0).unwrap();
        let end_dt = Utc.timestamp_opt(end_ts as i64, 0).unwrap();
        println!(
            "{:<6} | {:<20} | {:<20} | {}",
            i + 1,
            start_dt.format("%Y-%m-%d %H:%M:%S"),
            end_dt.format("%Y-%m-%d %H:%M:%S"),
            group.len()
        );
    }

    let client = Client::new();
    let mut results = vec![];

    for group in groups {
        let start_ts = group.first().unwrap().timestamp;
        let end_ts = group.last().unwrap().timestamp + 60;
        let price_map = fetch_price_map_for_range(&client, start_ts, end_ts);

        for swap in group {
            let mut matched_price = None;
            let mut timestamps: Vec<_> = price_map.keys().cloned().collect();
            timestamps.sort_unstable();

            for ts in timestamps.into_iter().rev() {
                if ts <= swap.timestamp {
                    matched_price = price_map.get(&ts).cloned();
                    break;
                }
            }

            if let Some(price) = matched_price {
                let usd_value = if swap.sold_mint == SOLANA_MINT {
                    swap.sold_amount * price
                } else {
                    swap.bought_amount * price
                };

                results.push(PricedSwap {
                    timestamp: swap.timestamp,
                    signature: swap.signature.clone(),
                    sold_mint: swap.sold_mint.clone(),
                    sold_amount: swap.sold_amount,
                    bought_mint: swap.bought_mint.clone(),
                    bought_amount: swap.bought_amount,
                    usd_value,
                    pricing_method: "binance_1m".to_string(),
                });
            }
        }
    }

    let json = serde_json::to_string_pretty(&results).unwrap();
    fs::write("output/swaps_with_sol_prices_binance.json", json).unwrap();

    enrich_cache_swaps_with_sol_prices(&swaps_path, &results);
}
