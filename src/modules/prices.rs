use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap};
use std::fs;
use std::time::Duration;
use config::Config;
use chrono::{Utc, TimeZone};
use crate::modules::utils::{get_project_root, get_swaps_path};



#[derive(Debug, Deserialize)]
struct Swap {
    timestamp: u64,
    signature: String,
    sold_mint: String,
    sold_token_name: String,
    sold_amount: f64,
    bought_mint: String,
    bought_token_name: String,
    bought_amount: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct PricedSwap {
    timestamp: u64,
    signature: String,
    sold_mint: String,
    sold_token_name: String,
    sold_amount: f64,
    bought_mint: String,
    bought_token_name: String,
    bought_amount: f64,
    usd_value: f64,
    pricing_method: String,
}


const SOLANA_MINT: &str = "So11111111111111111111111111111111111111112";
const BINANCE_SYMBOL: &str = "SOLUSDT";

fn is_usd_token(token_name: &str) -> bool {
    matches!(
        token_name,
        "USDC" | "USD Coin" | "USDT" | "Tether" | "USDH" | "UXD" | "cUSDC" | "stUSDT"
    )
}

fn load_sol_swaps(path: &str) -> Vec<Swap> {
    let content = fs::read_to_string(path).expect("failed to read file");
    let all: Vec<Swap> = serde_json::from_str(&content).expect("failed to parse JSON");

    let mut filtered = vec![];
    let mut skipped_usd_sol = 0;

    for s in all.into_iter() {
        let has_sol = s.sold_mint == SOLANA_MINT || s.bought_mint == SOLANA_MINT;
        let sold_name = s.sold_token_name.as_str();
        let bought_name = s.bought_token_name.as_str();
        let is_usd_pair = is_usd_token(sold_name) || is_usd_token(bought_name);

        if has_sol && is_usd_pair {
            skipped_usd_sol += 1;
        } else if has_sol {
            filtered.push(s);
        }
    }

    println!("Skipped {} SOL-USD swaps (handled separately)", skipped_usd_sol);
    filtered
}

fn group_by_time(swaps: &[Swap]) -> Vec<Vec<&Swap>> {
    const MAX_GROUP_SPAN: u64 = 6 * 3600; // 6 hours in seconds

    let mut sorted = swaps.iter().collect::<Vec<_>>();
    if sorted.is_empty() {
        return vec![];
    }

    sorted.sort_by_key(|s| s.timestamp);

    let mut groups = vec![];
    let mut current_group = vec![sorted[0]];
    let mut group_start = sorted[0].timestamp;

    for s in sorted.iter().skip(1) {
        let span = s.timestamp - group_start;

        if span > MAX_GROUP_SPAN {
            groups.push(current_group);
            current_group = vec![*s];
            group_start = s.timestamp;
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

fn enrich_swaps_with_pricing(swaps_path: &str, priced_swaps: &[PricedSwap]) {
    let content = fs::read_to_string(swaps_path).expect("failed to read raw swap file");
    let mut swaps: Vec<Value> = serde_json::from_str(&content).expect("failed to parse JSON");

    let price_map: HashMap<String, &PricedSwap> = priced_swaps
        .iter()
        .map(|p| (p.signature.clone(), p))
        .collect();

    for swap in swaps.iter_mut() {
        let sig = swap.get("signature").and_then(|s| s.as_str());
        let sold_name = swap.get("sold_token_name").and_then(|s| s.as_str()).unwrap_or("");
        let bought_name = swap.get("bought_token_name").and_then(|s| s.as_str()).unwrap_or("");
        let sold_mint = swap.get("sold_mint").and_then(|s| s.as_str()).unwrap_or("");
        let bought_mint = swap.get("bought_mint").and_then(|s| s.as_str()).unwrap_or("");
        let sold_amount = swap.get("sold_amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let bought_amount = swap.get("bought_amount").and_then(|v| v.as_f64()).unwrap_or(0.0);

        let has_sol = sold_mint == SOLANA_MINT || bought_mint == SOLANA_MINT;
        let is_usd = is_usd_token(sold_name) || is_usd_token(bought_name);

        if let Some(sig) = sig {
            if let Some(p) = price_map.get(sig) {
                let sol_amount = if p.sold_mint == SOLANA_MINT {
                    p.sold_amount
                } else {
                    p.bought_amount
                };
                let sol_price = p.usd_value / sol_amount;

                swap["binance_sol_usd_price"] = serde_json::json!(sol_price);
                swap["pricing_method"] = serde_json::json!(p.pricing_method);
                continue;
            }

            if has_sol && is_usd {
                let (sol_amount, usd_amount) = if sold_mint == SOLANA_MINT {
                    (sold_amount, bought_amount)
                } else {
                    (bought_amount, sold_amount)
                };

                if sol_amount > 0.0 {
                    let price = usd_amount / sol_amount;
                    swap["binance_sol_usd_price"] = serde_json::json!(price);
                    swap["pricing_method"] = serde_json::json!("usd_direct");
                } else {
                    swap["pricing_method"] = serde_json::json!("unverified");
                }
                continue;
            }

            if !has_sol && is_usd {
                swap["pricing_method"] = serde_json::json!("usd_direct");
                continue;
            }

            swap["pricing_method"] = serde_json::json!("unverified");
        } else {
            swap["pricing_method"] = serde_json::json!("unverified");
        }
    }

    let mut count_binance = 0;
    let mut count_usd_direct = 0;
    let mut count_unverified = 0;

    for swap in &swaps {
        match swap.get("pricing_method").and_then(|v| v.as_str()) {
            Some("binance_1m") => count_binance += 1,
            Some("usd_direct") => count_usd_direct += 1,
            Some("unverified") => count_unverified += 1,
            _ => {}
        }
    }

    println!(
        "Pricing summary: binance_1m: {}, usd_direct: {}, unverified: {}",
        count_binance, count_usd_direct, count_unverified
    );
    let json = serde_json::to_string_pretty(&swaps).unwrap();
    fs::write(swaps_path, json).unwrap();
    println!("✅ Overwrote {} with enriched pricing fields", swaps_path);

}

pub fn run_prices() -> Result<(), Box<dyn std::error::Error>> {
    let root = get_project_root();

    let settings = Config::builder()
        .add_source(config::File::with_name(root.join("config/config").to_str().unwrap()))
        .build()?;

    let wallet_address: String = settings.get("wallet_address")?;
    let swaps_path = get_swaps_path(&wallet_address);

    let swaps = load_sol_swaps(&swaps_path);
    let groups = group_by_time(&swaps);

    println!("{:<6} | {:<20} | {:<20} | {}", "Group", "Start Time", "End Time", "Swaps");
    println!("{}", "-".repeat(65));

    for (i, group) in groups.iter().enumerate() {
        let start_ts = group.first().unwrap().timestamp.saturating_sub(300);
        let end_ts = group.last().unwrap().timestamp + 300;
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
        let start_ts = group.first().unwrap().timestamp.saturating_sub(120);
        let end_ts = group.last().unwrap().timestamp + 60;
        let price_map = fetch_price_map_for_range(&client, start_ts, end_ts);

        for swap in group {
            // USD direct pricing logic
            if swap.sold_token_name.contains("USD") {
                results.push(PricedSwap {
                    timestamp: swap.timestamp,
                    signature: swap.signature.clone(),
                    sold_mint: swap.sold_mint.clone(),
                    sold_token_name: swap.sold_token_name.clone(),
                    sold_amount: swap.sold_amount,
                    bought_mint: swap.bought_mint.clone(),
                    bought_token_name: swap.bought_token_name.clone(),
                    bought_amount: swap.bought_amount,
                    usd_value: swap.sold_amount,
                    pricing_method: "usd_direct".to_string(),
                });
                continue;
            } else if swap.bought_token_name.contains("USD") {
                results.push(PricedSwap {
                    timestamp: swap.timestamp,
                    signature: swap.signature.clone(),
                    sold_mint: swap.sold_mint.clone(),
                    sold_token_name: swap.sold_token_name.clone(),
                    sold_amount: swap.sold_amount,
                    bought_mint: swap.bought_mint.clone(),
                    bought_token_name: swap.bought_token_name.clone(),
                    bought_amount: swap.bought_amount,
                    usd_value: swap.bought_amount,
                    pricing_method: "usd_direct".to_string(),
                });
                continue;
            }

            // fall back to binance pricing
            let mut matched_price = None;

            let mut timestamps: Vec<_> = price_map.keys().cloned().collect();
            timestamps.sort_unstable();

            let mut min_diff = u64::MAX;
            let mut best_ts = None;

            for ts in &timestamps {
                let diff = swap.timestamp.abs_diff(*ts);
                if diff < min_diff {
                    min_diff = diff;
                    best_ts = Some(*ts);
                }
            }

            if let Some(ts) = best_ts {
                if min_diff <= 90 {
                    matched_price = price_map.get(&ts).cloned();
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
                    sold_token_name: swap.sold_token_name.clone(),
                    sold_amount: swap.sold_amount,
                    bought_mint: swap.bought_mint.clone(),
                    bought_token_name: swap.bought_token_name.clone(),
                    bought_amount: swap.bought_amount,
                    usd_value,
                    pricing_method: "binance_1m".to_string(),
                });
            } else {
                println!(
                    "No price found for swap at ts={} (sig={})",
                    swap.timestamp, swap.signature
                );
            }
        }
    }

    println!("✅ Overwrote {} with enriched swaps.", swaps_path);

    enrich_swaps_with_pricing(&swaps_path, &results);

    Ok(())
}
