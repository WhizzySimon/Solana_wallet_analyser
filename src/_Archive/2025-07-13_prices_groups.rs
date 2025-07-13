use serde::Deserialize;
use std::{
    fs,
    time::{Duration, UNIX_EPOCH},
};

const INPUT_FILE: &str = "cache/swaps_KzNxNJvcieTvAF4bnfsuH1YEAXLHcB1cs468JA4K4QE.json";
const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
const MAX_SPAN: u64 = 86400; // 1 day

#[derive(Debug, Deserialize)]
struct SwapSummary {
    timestamp: u64,
    signature: String,
    sold_mint: String,
    sold_amount: f64,
    bought_mint: String,
    bought_amount: f64,
}

fn format_utc(ts: u64) -> String {
    let dt = UNIX_EPOCH + Duration::from_secs(ts);
    let datetime: chrono::DateTime<chrono::Utc> = dt.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw = fs::read_to_string(INPUT_FILE)?;
    let mut swaps: Vec<SwapSummary> = serde_json::from_str(&raw)?;

    swaps.retain(|s| s.sold_mint == SOL_MINT || s.bought_mint == SOL_MINT);
    if swaps.is_empty() {
        println!("No SOL-related swaps found.");
        return Ok(());
    }

    swaps.sort_by_key(|s| s.timestamp);

    let mut groups: Vec<Vec<&SwapSummary>> = vec![];
    let mut group_ranges: Vec<(u64, u64)> = vec![];

    let mut current_group: Vec<&SwapSummary> = vec![];
    let mut group_start_ts = swaps[0].timestamp;

    for swap in &swaps {
        if current_group.is_empty() {
            group_start_ts = swap.timestamp;
            current_group.push(swap);
        } else if swap.timestamp - group_start_ts <= MAX_SPAN {
            current_group.push(swap);
        } else {
            let from = current_group.first().unwrap().timestamp;
            let to = current_group.last().unwrap().timestamp;
            groups.push(current_group);
            group_ranges.push((from, to));

            current_group = vec![swap];
            group_start_ts = swap.timestamp;
        }
    }

    if !current_group.is_empty() {
        let from = current_group.first().unwrap().timestamp;
        let to = current_group.last().unwrap().timestamp;
        groups.push(current_group);
        group_ranges.push((from, to));
    }

    println!("Grouped {} SOL swaps into {} CoinGecko-safe buckets:", swaps.len(), groups.len());
    println!("{:<6} {:<20} {:<20} {:<6} {:<10}", "Group", "From (UTC)", "To (UTC)", "Count", "Span(min)");

    for (i, group) in groups.iter().enumerate() {
        let from = group.first().unwrap().timestamp;
        let to = group.last().unwrap().timestamp;
        let span_min = (to - from) as f64 / 60.0;
        println!(
            "{:<6} {:<20} {:<20} {:<6} {:.1}",
            i + 1,
            format_utc(from),
            format_utc(to),
            group.len(),
            span_min
        );
    }

    // ✅ This vector will be used for API calls in the next step:
    println!("\nAPI call time ranges (Unix timestamps):");
    for (i, (from_ts, to_ts)) in group_ranges.iter().enumerate() {
        println!("Group {}: from={} to={}", i + 1, from_ts, to_ts);
    }

    Ok(())
}
