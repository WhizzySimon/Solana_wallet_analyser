use std::collections::HashMap;

use crate::modules::types::{InventoryEntry, TradeWithPnl, PricedSwap};
use std::fs::File;
use std::io::Write;

fn match_and_consume_inventory(
    inventory: &mut HashMap<String, Vec<InventoryEntry>>,
    mint: &str,
    amount_to_sell: f64,
    use_fifo: bool,
) -> (f64, bool) {
    let mut entries = inventory.get(mint).cloned().unwrap_or_default();
    if !use_fifo {
        entries.reverse();
    }

    let mut remaining = amount_to_sell;
    let mut cost_basis = 0.0;

    while remaining > 0.0 && !entries.is_empty() {
        let mut entry = entries[0].clone();
        let used = remaining.min(entry.amount);
        cost_basis += used * entry.price_per_token;
        entry.amount -= used;
        remaining -= used;

        if entry.amount > 0.0 {
            entries[0] = entry;
        } else {
            entries.remove(0);
        }
    }

    // Write back updated entries
    inventory.insert(mint.to_string(), if use_fifo { entries } else { entries.into_iter().rev().collect() });

    (cost_basis, remaining == amount_to_sell)
}


pub fn calculate_direct_usd_pnl(
    swaps: &[PricedSwap],
    use_fifo: bool,
    inventory: &mut HashMap<String, Vec<InventoryEntry>>,

) -> Vec<TradeWithPnl> {
    let mut trades: Vec<TradeWithPnl> = Vec::new();

    for swap in swaps {
        if swap.pricing_method != "usd_direct" {
            continue;
        }

        let usd_value = match swap.usd_value {
            Some(v) => v,
            None => continue,
        };

        // Add to inventory (buy)
        if swap.bought_amount > 0.0 {
            let entry = InventoryEntry {
                amount: swap.bought_amount,
                price_per_token: usd_value / swap.bought_amount,
                total_usd: usd_value,
                timestamp: swap.timestamp,
                signature: swap.signature.clone(),
            };
            inventory.entry(swap.bought_mint.clone()).or_default().push(entry);
        }

        let (cost_basis, no_inventory) =
            match_and_consume_inventory(inventory, &swap.sold_mint, swap.sold_amount, use_fifo);

        if no_inventory {
            eprintln!(
                "⚠️  No inventory for {}, treating as full profit: {}",
                swap.sold_token_name, usd_value
            );
        }

        if swap.sold_amount == 0.0 {
            continue;
        }

        let trade = TradeWithPnl {
            sold_token: swap.sold_token_name.clone(),
            sold_amount: swap.sold_amount,
            received_usd: usd_value,
            cost_basis_usd: cost_basis,
            profit_loss: usd_value - cost_basis,
            timestamp: swap.timestamp,
            signature: swap.signature.clone(),
        };

        trades.push(trade);
        /* 
        println!(
            "🧾 {} | {}: sold {} for ${} (cost: {}, pnl: {})",
            swap.signature,
            swap.sold_token_name,
            swap.sold_amount,
            usd_value,
            cost_basis,
            usd_value - cost_basis
        ); */

    }

    trades
}

pub fn calculate_sol_indirect_pnl(
    swaps: &[PricedSwap],
    use_fifo: bool,
    inventory: &mut HashMap<String, Vec<InventoryEntry>>,
) -> Vec<TradeWithPnl> {
    let mut trades: Vec<TradeWithPnl> = Vec::new();

    for swap in swaps {
        if swap.pricing_method != "binance_1m" {
            continue;
        }

        let sol_usd_price = match swap.binance_sol_usd_price {
            Some(v) => v,
            None => continue,
        };

        let usd_value = swap.usd_value.unwrap_or(swap.sold_amount * sol_usd_price);

        // Add to inventory (buy)
        if swap.bought_amount > 0.0 {
            let entry = InventoryEntry {
                amount: swap.bought_amount,
                price_per_token: usd_value / swap.bought_amount,
                total_usd: usd_value,
                timestamp: swap.timestamp,
                signature: swap.signature.clone(),
            };
            inventory.entry(swap.bought_mint.clone()).or_default().push(entry);
        }

        
        let (cost_basis, no_inventory) =
            match_and_consume_inventory(inventory, &swap.sold_mint, swap.sold_amount, use_fifo);

        if no_inventory {
            eprintln!(
                "⚠️  No inventory for {}, treating as full profit: {}",
                swap.sold_token_name, usd_value
            );
        }


        if swap.sold_amount == 0.0 {
            continue;
        }

        let trade = TradeWithPnl {
            sold_token: swap.sold_token_name.clone(),
            sold_amount: swap.sold_amount,
            received_usd: usd_value,
            cost_basis_usd: cost_basis,
            profit_loss: usd_value - cost_basis,
            timestamp: swap.timestamp,
            signature: swap.signature.clone(),
        };

        trades.push(trade);
    }

    trades
}

pub fn calc_pnl(priced_swaps: &[PricedSwap], wallet_address: &str) 
    -> Result<Vec<TradeWithPnl>, Box<dyn std::error::Error>> {
    
    let settings = crate::modules::utils::load_config()?;
    let use_fifo = settings.fifo.unwrap_or(true);
    let write_cache_files = settings.write_cache_files.unwrap_or(false);

    let mut inventory: HashMap<String, Vec<InventoryEntry>> = HashMap::new();
    let mut swaps_sorted = priced_swaps.to_vec();
    swaps_sorted.sort_by(|a, b| {
        a.timestamp.cmp(&b.timestamp)
            .then(a.signature.cmp(&b.signature))
    });


    let mut trades = calculate_direct_usd_pnl(&swaps_sorted, use_fifo, &mut inventory);
    let mut sol_trades = calculate_sol_indirect_pnl(&swaps_sorted, use_fifo, &mut inventory);

    trades.append(&mut sol_trades);
    
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
