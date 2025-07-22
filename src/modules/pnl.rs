use std::collections::HashMap;
use std::fs;
use crate::modules::types::{Trade, PricedSwap};


pub fn calculate_pnl(priced_swaps: Vec<PricedSwap>) ->Result<Vec<Trade>, Box<dyn std::error::Error>> {
    let mut trades: Vec<Trade> = Vec::new();
    let mut inventory: HashMap<String, Vec<PricedSwap>> = HashMap::new();

    let settings = crate::modules::utils::load_config()?;
    let fifo = settings.fifo.unwrap_or(true);

    for swap in priced_swaps {
        let (token_mint, token_name, is_buy, amount, usd_value) = if is_stablecoin(&swap.bought_token_name) {
            // Selling token_mint for stablecoin
            (
                swap.sold_mint.clone(),
                swap.sold_token_name.clone(),
                false,
                swap.sold_amount,
                swap.usd_value.unwrap_or(0.0),
            )
        } else if is_stablecoin(&swap.sold_token_name) {
            // Buying token_mint with stablecoin
            (
                swap.bought_mint.clone(),
                swap.bought_token_name.clone(),
                true,
                swap.bought_amount,
                swap.usd_value.unwrap_or(0.0),
            )
        } else {
            continue;
        };


        if is_buy {
            inventory.entry(token_mint.clone()).or_default().push(swap.clone());
        } else {
            let mut remaining = amount;
            let queue = inventory.entry(token_mint.clone()).or_default();

            while remaining > 0.0 && !queue.is_empty() {
                let idx = if fifo { 0 } else { queue.len() - 1 };
                let buy = &mut queue[idx];

                let available = if is_stablecoin(&buy.sold_token_name) {
                    buy.bought_amount
                } else {
                    buy.sold_amount
                };

                let matched_amount = remaining.min(available);

                let cost_usd = matched_amount / amount * usd_value;
                let pnl = usd_value - cost_usd;

                trades.push(Trade {
                    token_mint: token_mint.clone(),
                    token_name: token_name.clone(),
                    buy_signature: buy.signature.clone(),
                    sell_signature: swap.signature.clone(),
                    buy_timestamp: buy.timestamp,
                    sell_timestamp: swap.timestamp,
                    amount: matched_amount,
                    cost_usd,
                    proceeds_usd: usd_value,
                    pnl_usd: pnl,
                    holding_period_secs: swap.timestamp.saturating_sub(buy.timestamp),
                });

                remaining -= matched_amount;

                if available <= matched_amount {
                    queue.remove(idx);
                } else {
                    if is_stablecoin(&buy.sold_token_name) {
                        buy.bought_amount -= matched_amount;
                    } else {
                        buy.sold_amount -= matched_amount;
                    }
                }
            }
        }
    }

    let json = serde_json::to_string_pretty(&trades).unwrap();
    fs::create_dir_all("output").unwrap();
    fs::write("output/trades.json", json).unwrap();
    println!("✅ Wrote {} trades to output/trades.json", trades.len());
    Ok(trades)
}

fn is_stablecoin(token_name: &str) -> bool {
    matches!(
        token_name.to_lowercase().as_str(),
        "usdc" | "usdt" | "usd coin" | "tether"
    )
}