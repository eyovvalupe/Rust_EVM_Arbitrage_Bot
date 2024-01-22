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
pub mod execution;
pub mod markets;
pub mod order;
pub mod routing;
pub mod tests;
pub mod transactions;

use crate::transactions::swap_transaction_calldata;
// use crate::tests::{
//     discover_erc_4626_vaults, discover_factories, simulate_swap, swap_calldata, sync_amms,
// };

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

    let token_in = H160::from_str(ETH)?;
    let token_out = H160::from_str(USDC).unwrap();
    let amount_in = U256::from_dec_str("100000000000000000").unwrap();
    let slippage: u32 = 1 * 100; // Should be permyriad value
    let receiver = H160::from_str("0x0000000000000000000000000000000000000000").unwrap();

    let (swap_data, swap_multicall) = swap_transaction_calldata(
        &configuration,
        token_in,
        token_out,
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

    // simulate_swap::try_sample_swap_simulate(rpc_endpoint.clone()).await?;
    // swap_calldata::try_swap_calldata(configuration.http_endpoint.clone()).await?;
    // discover_erc_4626_vaults::try_discorver_erc_4626_vaults(rpc_endpoint.clone()).await?;
    // discover_factories::try_discorver_factories(rpc_endpoint.clone()).await?;

    // sync_amms::try_sync_amms(configuration.http_endpoint.clone()).await?;

    Ok(())
}
