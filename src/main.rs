use std::{str::FromStr, sync::Arc};

use constants::{ETH, USDC};
use dotenv::dotenv;
use ethers::{
    providers::{Http, Provider},
    types::{H160, U256},
};

pub mod abi;
pub mod config;
pub mod constants;
pub mod error;
pub mod markets;
pub mod routing;
pub mod tests;
pub mod transactions;

use crate::transactions::swap_transaction_calldata;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenv().ok();

    tracing_subscriber::fmt::init();

    let rpc_endpoint: String = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let ws_endpoint: String = std::env::var("ETHEREUM_WS_ENDPOINT")?;

    // Initialize a new configuration
    let configuration = config::Config::new(rpc_endpoint, ws_endpoint);
    let middleware = Arc::new(Provider::<Http>::try_from(
        configuration.http_endpoint.clone(),
    )?);

    let token_in = H160::from_str("0x0000000000000000000000000000000000000000")?;
    let token_out = H160::from_str("0xdBdb4d16EdA451D0503b854CF79D55697F90c8DF").unwrap();
    let token_x = H160::from_str("0x0000000000000000000000000000000000000000").unwrap();
    let amount_in = U256::from_dec_str("1000000").unwrap();
    let slippage: u32 = 10 * 100; // Should be permyriad value
    let receiver = H160::from_str("0x0000000000000000000000000000000000000000").unwrap();

    let (swap_data, swap_multicall) = swap_transaction_calldata(
        &configuration,
        token_in,
        token_out,
        token_x,
        amount_in,
        slippage,
        receiver,
        middleware,
    )
    .await?;

    println!(
        "SwapData: {:?}\n\nSwapMultiCall: {:?}",
        swap_data, swap_multicall
    );

    Ok(())
}
