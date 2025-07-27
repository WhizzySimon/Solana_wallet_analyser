use crate::modules::types::{TokenPnl, PricedSwap, BuyPart, SellPart};
use std::fs::File;
use std::io::Write;

fn is_stable(token: &str) -> bool {
    matches!(token, "USDC" | "USDT" | "USD" | "USDL" | "PAI" | "UXD")
}

pub fn calculate_direct_token_pnl(
    swaps: &[PricedSwap],
) -> Vec<TokenPnl> {
    use std::collections::{HashMap, VecDeque};

    let mut token_map: HashMap<String, (VecDeque<BuyPart>, Vec<SellPart>, f64)> = HashMap::new();

    for swap in swaps {
        if swap.usd_value.is_none() {continue;}

        let usd_value = swap.usd_value.unwrap();

        // GROUPING BY SOLD TOKEN (the one the user is trading)
        let token = if is_stable(&swap.sold_token_name) && !is_stable(&swap.bought_token_name) {
            &swap.bought_token_name
        } else {
            &swap.sold_token_name
        };

        // insert BuyPart for bought_token (must be separate borrow)
        if !is_stable(&swap.bought_token_name) {
            token_map
                .entry(swap.bought_token_name.clone())
                .or_insert_with(|| (VecDeque::new(), Vec::new(), 0.0))
                .0
                .push_back(BuyPart {
                    timestamp: swap.timestamp,
                    amount: swap.bought_amount,
                    cost_usd: usd_value,
                });
        }

        // safe second borrow for SELL logic
        let entry = token_map.entry(token.clone()).or_insert_with(|| (VecDeque::new(), Vec::new(), 0.0));

        // SELL
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

    // Create TokenPnl results
    token_map
        .into_iter()
        .map(|(token, (buys, sells, realized_pnl))| {
            let total_bought: f64 = buys.iter().map(|b| b.amount).sum();
            let total_cost: f64 = buys.iter().map(|b| b.cost_usd).sum();
            let average_cost = if total_bought > 0.0 { total_cost / total_bought } else { 0.0 };
            let total_sold: f64 = sells.iter().map(|s| s.amount).sum();

            TokenPnl {
                token,
                buys: buys.into(),
                sells, // now moved safely
                realized_pnl,
                total_bought,
                total_sold,
                remaining_amount: total_bought,
                average_cost_usd: average_cost,
            }

        })
        .collect()
}

pub async fn calc_pnl(priced_swaps: &[PricedSwap], wallet_address: &str) 
    -> Result<Vec<TokenPnl>, Box<dyn std::error::Error>> {
    
    let settings = crate::modules::utils::load_config()?;
    //let use_fifo = settings.fifo.unwrap_or(true);
    let write_cache_files = settings.write_cache_files.unwrap_or(false);

    // let inventory: HashMap<String, Vec<InventoryEntry>> = HashMap::new();
    let mut swaps_sorted = priced_swaps.to_vec();
    swaps_sorted.sort_by(|a, b| {
        a.timestamp.cmp(&b.timestamp)
            .then(a.signature.cmp(&b.signature))
    });
    let trades = calculate_direct_token_pnl (&swaps_sorted);

/*     let mut trades = calculate_direct_usd_pnl(&swaps_sorted, use_fifo, &mut inventory);
    let mut sol_trades = calculate_sol_indirect_pnl(&swaps_sorted, use_fifo, &mut inventory);

    trades.append(&mut sol_trades); */
    
    if write_cache_files {
        let out_path = format!("cache/trades_{}.json", wallet_address);
        let json = serde_json::to_string_pretty(&trades)?;
        let mut file = File::create(&out_path)?;
        file.write_all(json.as_bytes())?;
        println!("💰 Wrote {} trades to {}", trades.len(), out_path);
    }
    else {
        println!("Found {} trades.", trades.len());
    }
    Ok(trades)
}
