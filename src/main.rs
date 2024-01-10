use dotenv::dotenv;

pub mod constants;
pub mod tests;

use crate::tests::{
    simulate_swap,
    discover_factories,
};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenv().ok();

    tracing_subscriber::fmt::init();

    let rpc_endpoint: String = std::env::var("ETHEREUM_RPC_ENDPOINT")?;

    simulate_swap::try_sample_swap_simulate(rpc_endpoint.clone()).await?;
    discover_factories::try_discorver_factories(rpc_endpoint.clone()).await?;

    Ok(())
}
