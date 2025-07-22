
use wallet_analyzer::modules::transactions::get_transactions;
use wallet_analyzer::modules::swaps::filter_and_name_swaps;
use wallet_analyzer::modules::prices::get_or_load_swaps_with_prices;



fn main() -> Result<(), Box<dyn std::error::Error>> {

    let transactions = get_transactions()?;
    println!("Total transactions fetched/loaded: {}", transactions.len());

    let named_swaps = filter_and_name_swaps(&transactions)?;
    println!("Total swaps with token names filtered/loaded: {}", named_swaps.len());

    let _priced_swaps = get_or_load_swaps_with_prices(&named_swaps);

    Ok(())
}
