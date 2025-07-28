use reqwest::Client;
use std::collections::{HashMap};
use std::fs;
use std::time::Duration;
use crate::modules::utils::{get_priced_swaps_path};
use crate::modules::types::{NamedSwap, PricedSwap, Settings};

const SOLANA_MINT: &str = "So11111111111111111111111111111111111111112";
const BINANCE_SYMBOL: &str = "SOLUSDT";

fn group_by_time(swaps_with_token_names: &[NamedSwap]) -> Vec<Vec<&NamedSwap>> {
    const MAX_GROUP_SPAN: u64 = 6 * 3600; // 6 hours in seconds

    let mut sorted = swaps_with_token_names.iter().collect::<Vec<_>>();
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

async fn fetch_price_map_for_range(client: &Client, start_ts: u64, end_ts: u64) -> Result<HashMap<u64, f64>, Box<dyn std::error::Error>> {
    let url = format!(
        "https://api.binance.com/api/v3/klines?symbol={}&interval=1m&startTime={}&endTime={}",
        BINANCE_SYMBOL,
        start_ts * 1000,
        end_ts * 1000
    );

    let resp = client
        .get(&url)
        .timeout(Duration::from_secs(10))
        .send().await?
        .json::<Vec<Vec<serde_json::Value>>>().await?;

    let mut map = HashMap::new();
    for entry in resp {
        if let (Some(open_time), Some(close_price)) = (
            entry.get(0).and_then(|v| v.as_u64()),
            entry.get(4).and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()),
        ) {
            map.insert(open_time / 1000, close_price);
        }
    }

    Ok(map)

}

pub async fn get_or_load_swaps_with_prices(swaps_with_token_names:&Vec<NamedSwap>, settings: &Settings) 
    -> Result<Vec<PricedSwap>, Box<dyn std::error::Error>> {

    let wallet_address = settings.wallet_address.to_lowercase();
    let priced_swaps_path = get_priced_swaps_path(&wallet_address);
    let use_cached_priced_swaps = settings.config.use_cached_priced_swaps.unwrap_or(true);
    let write_cache_files = settings.config.write_cache_files.unwrap_or(false);

    let priced_swaps: Vec<PricedSwap> = if use_cached_priced_swaps {
        println!("♻️  Using cached enriched swaps from {}", priced_swaps_path);
        let content = fs::read_to_string(&priced_swaps_path).expect("Failed to read cached enriched swaps");
        serde_json::from_str(&content).expect("Failed to parse cached enriched swaps as PricedSwap")
    } else {
        let groups = group_by_time(swaps_with_token_names);

        println!("{:<6} | {:<20} | {:<20} | {}", "Group", "Start Time", "End Time", "Swaps");
        println!("{}", "-".repeat(65));
    
        for (i, group) in groups.iter().enumerate() {
            let start_ts = group.first().unwrap().timestamp.saturating_sub(120);
            let end_ts = group.last().unwrap().timestamp + 60;
            println!("{:<6} | {:<20} | {:<20} | {}", i + 1, start_ts, end_ts, group.len());
        }

        let client = Client::new();
        let mut results = vec![];

        for group in groups {
            let start_ts = group.first().unwrap().timestamp.saturating_sub(120);
            let end_ts = group.last().unwrap().timestamp + 60;
            let price_map = fetch_price_map_for_range(&client, start_ts, end_ts).await?;

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
                        usd_value: Some(swap.sold_amount),
                        pricing_method: "usd_direct".to_string(),
                        binance_sol_usd_price: None,
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
                        usd_value: Some(swap.bought_amount),
                        pricing_method: "usd_direct".to_string(),
                        binance_sol_usd_price: None,
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
                        usd_value: Some(usd_value),
                        pricing_method: "binance_1m".to_string(),
                        binance_sol_usd_price: Some(price),
                    });
                } else {
                    println!(
                        "No price found for swap at ts={} (sig={})",
                        swap.timestamp, swap.signature
                    );
                }
            }
        }

        let mut count_binance = 0;
        let mut count_usd_direct = 0;
        let mut count_unverified = 0;

        for swap in &results {
            match swap.pricing_method.as_str() {
                "binance_1m" => count_binance += 1,
                "usd_direct" => count_usd_direct += 1,
                "unverified" => count_unverified += 1,
                _ => {}
            }
        }

        println!(
            "Pricing summary: binance_1m: {}, usd_direct: {}, unverified: {}",
            count_binance, count_usd_direct, count_unverified
        );

        if write_cache_files {
            let json = serde_json::to_string_pretty(&results)?;
            fs::write(&priced_swaps_path, json)?;
            println!("✅ Saved enriched swaps to {}", priced_swaps_path);
        }
        else {
            println!("Priced {} swaps.", results.len());
        }
        results
    };
    Ok(priced_swaps)

}
