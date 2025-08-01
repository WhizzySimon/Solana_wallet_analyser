use crate::modules::types::{Settings, TokenPnl, PricedSwap, BuyPart, SellPart};
use std::fs::File;
use std::io::Write;

fn is_stable(token: &str) -> bool {
    matches!(token, "USDC" | "USDT" | "USD" | "USDL" | "PAI" | "UXD")
}

pub fn calculate_direct_token_pnl(swaps: &[PricedSwap]) -> Vec<TokenPnl> {
    use std::collections::{HashMap, VecDeque};

    let mut token_map: HashMap<String, (VecDeque<BuyPart>, Vec<SellPart>, f64)> = HashMap::new();

    for swap in swaps {
        if swap.usd_value.is_none() {
            continue;
        }
        let usd_value = swap.usd_value.unwrap();

        // === Grouping: determine which token to attribute PnL to ===
        let group_token = match (
            swap.sold_token_name.as_str(),
            swap.bought_token_name.as_str(),
        ) {
            // Ignore swaps between stables or WSOL
            (a, b) if is_stable(a) && is_stable(b) => continue,
            (a, b) if a == "Wrapped SOL" && b == "Wrapped SOL" => continue,

            // If buying a token with WSOL, group by the bought token
            ("Wrapped SOL", other) if !is_stable(other) => Some(other.to_string()),
            // If selling a token into WSOL, group by the sold token
            (other, "Wrapped SOL") if !is_stable(other) => Some(other.to_string()),

            // Skip swaps like WSOL <-> stable
            ("Wrapped SOL", other) if is_stable(other) => continue,
            (other, "Wrapped SOL") if is_stable(other) => continue,

            // Fallback: group by sold token
            _ => Some(swap.sold_token_name.clone()),
        };

        if let Some(token) = group_token {
            let entry = token_map
                .entry(token.clone())
                .or_insert_with(|| (VecDeque::new(), Vec::new(), 0.0));

            // Track buys for all non-stable tokens (including Wrapped SOL if bought)
            if !is_stable(&swap.bought_token_name) {
                entry.0.push_back(BuyPart {
                    timestamp: swap.timestamp,
                    amount: swap.bought_amount,
                    cost_usd: usd_value,
                });
            }

            // SELL logic
            let mut remaining = swap.sold_amount;
            let mut cost_basis = 0.0;

            while remaining > 0.0 {
                if let Some(mut buy) = entry.0.pop_front() {
                    let used = remaining.min(buy.amount);
                    let ratio = used / buy.amount;
                    cost_basis += buy.cost_usd * ratio;
                    remaining -= used;
                    buy.amount -= used;
                    if buy.amount > 0.0 {
                        entry.0.push_front(buy);
                        break;
                    }
                } else {
                    break;
                }
            }

            let sold_amount = swap.sold_amount - remaining;

            entry.1.push(SellPart {
                timestamp: swap.timestamp,
                amount: sold_amount,
                proceeds_usd: usd_value,
            });

            entry.2 += usd_value - cost_basis;
        }
    }

    token_map
        .into_iter()
        .map(|(token, (buys, sells, realized_pnl))| {
            if token == "Lamine Yamal" {
                println!("--- DEBUG: Lamine Yamal PnL Breakdown ---");
                println!("Total Bought: {:.2}, Total Cost: {:.2}", 
                        buys.iter().map(|b| b.amount).sum::<f64>(), 
                        buys.iter().map(|b| b.cost_usd).sum::<f64>());
                for (i, b) in buys.iter().enumerate() {
                    println!("Buy #{:>2}: {:.4} tokens for ${:.4} at {}", i+1, b.amount, b.cost_usd, b.timestamp);
                }

                println!("Total Sold: {:.2}", sells.iter().map(|s| s.amount).sum::<f64>());
                for (i, s) in sells.iter().enumerate() {
                    println!("Sell #{:>2}: {:.4} tokens for ${:.4} at {}", i+1, s.amount, s.proceeds_usd, s.timestamp);
                }

                println!("Realized PnL: {:.2}\n", realized_pnl);
            }


            let total_bought: f64 = buys.iter().map(|b| b.amount).sum();
            let total_cost: f64 = buys.iter().map(|b| b.cost_usd).sum();
            let average_cost = if total_bought > 0.0 {
                total_cost / total_bought
            } else {
                0.0
            };
            let total_sold: f64 = sells.iter().map(|s| s.amount).sum();

            TokenPnl {
                token,
                buys: buys.into(),
                sells,
                realized_pnl,
                total_bought,
                total_sold,
                remaining_amount: total_bought,
                average_cost_usd: average_cost,
            }
        })
        .collect()
}

pub async fn calc_pnl(
    priced_swaps: &[PricedSwap],
    settings: &Settings,
) -> Result<Vec<TokenPnl>, Box<dyn std::error::Error>> {
    let write_cache_files = settings.config.write_cache_files.unwrap_or(false);

    let mut swaps_sorted = priced_swaps.to_vec();

    for swap in &swaps_sorted {
        if swap.sold_token_name.contains("Yamal") || swap.bought_token_name.contains("Yamal") {
            println!(
                "🧪 DEBUG normalized: {} -> sold: {} (dec: {:?}) bought: {} (dec: {:?})",
                swap.signature,
                swap.sold_amount, swap.sold_decimals,
                swap.bought_amount, swap.bought_decimals
            );
        }
    }

    swaps_sorted.sort_by(|a, b| {
        a.timestamp
            .cmp(&b.timestamp)
            .then(a.signature.cmp(&b.signature))
    });

    let trades = calculate_direct_token_pnl(&swaps_sorted);

    if write_cache_files {
        let out_path = format!("cache/trades_{}.json", settings.wallet_address);
        let json = serde_json::to_string_pretty(&trades)?;
        let mut file = File::create(&out_path)?;
        file.write_all(json.as_bytes())?;
        println!("💰 Wrote {} trades to {}", trades.len(), out_path);
    } else {
        println!("Found {} trades.", trades.len());
    }

    Ok(trades)
}
