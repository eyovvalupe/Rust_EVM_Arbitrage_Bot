use amms::amm::{uniswap_v2::UniswapV2Pool, AutomatedMarketMaker};
use ethers::{
    providers::{Http, Provider},
    types::{H160, U256},
};
use std::{str::FromStr, sync::Arc};

use crate::constants::*;

pub async fn try_sample_swap_simulate(rpc_endpoint: String) -> eyre::Result<()> {
    let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);

    // Initialize the pool
    let pool_address = H160::from_str(WETH_USDC_V2)?; // WETH/USDC
    let pool = UniswapV2Pool::new_from_address(pool_address, 300, middleware.clone()).await?;

    // Simulate a swap
    let token_in = H160::from_str(WETH)?;
    let amount_out = pool.simulate_swap(token_in, U256::from_dec_str("1000000000000000000")?)?;

    println!("Amount out: {amount_out}");

    Ok(())
}
