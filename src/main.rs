use std::{str::FromStr, sync::Arc};

use amms::amm::uniswap_v2::IUNISWAPV2PAIR_ABI;
use cfmms::pool::{Pool, UniswapV2Pool};
use constants::{ETH, FIFTH_WEB_MULTICALL, UNISWAP_V2_FEE, USDC, WETH};
use dotenv::dotenv;
use ethabi::Token;
use ethers::{
    abi::AbiEncode,
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

use crate::abi::IERC20_ABI;
// use crate::tests::{
//     discover_erc_4626_vaults, discover_factories, simulate_swap, swap_calldata, sync_amms,
// };

#[derive(Debug)]
struct SwapData {
    token_in: Option<H160>,
    token_out: Option<H160>,
    amount_in: Option<U256>,
    amount_out_min: Option<U256>,
    protocol_fee: Option<U256>,
    bribe: U256,
    affiliate: U256,
    referrer: U256,
}
#[derive(Debug)]
struct SwapMultiCall {
    token_in_destination: H160,
    calls: Vec<(H160, String)>,
}

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
    let slippage: u32 = 1 * 100;
    let receiver = H160::from_str("0x0000000000000000000000000000000000000000").unwrap();

    // TODO: Make this as module

    let mut amount_fixed_for_fee = amount_in;
    let mut protocol_fee = U256::zero();
    let bribe = U256::zero();

    if token_in.is_zero() {
        amount_fixed_for_fee -= amount_fixed_for_fee / 100;
        protocol_fee = amount_in - amount_fixed_for_fee;
    }

    let markets = markets::get_market(
        match token_in.is_zero() {
            true => H160::from_str(WETH).unwrap(),
            false => token_in,
        },
        token_out,
        &configuration.dexes,
        middleware.clone(),
    )
    .await?;

    match markets {
        Some(markets) => {
            println!("Found markets: {:?}", markets.keys());

            // TODO: find best route for this pair
            let mut best_amount_out = U256::zero();
            let mut best_pool = Pool::UniswapV2(UniswapV2Pool::default());
            // let mut handles = vec![];

            let to = H160::from_str(FIFTH_WEB_MULTICALL).unwrap();

            for pool in markets.values() {
                let pool = *pool;
                match pool {
                    Pool::UniswapV2(_) => {
                        let swap_amount_out = pool
                            .simulate_swap(token_in, amount_fixed_for_fee, middleware.clone())
                            .await?;
                        if swap_amount_out > best_amount_out {
                            best_amount_out = swap_amount_out;
                            best_pool = pool;
                        }
                    }

                    Pool::UniswapV3(_uniswap_v3_pool) => {
                        //     let uniswap_v3_quoter = IUniswapV3Quoter::new(
                        //         H160::from_str(V3_QUOTER_ADDRESS).unwrap(),
                        //         middleware.clone(),
                        //     );

                        //     let (token_in, token_out) = if token_in == uniswap_v3_pool.token_a {
                        //         (uniswap_v3_pool.token_a, uniswap_v3_pool.token_b)
                        //     } else {
                        //         (uniswap_v3_pool.token_b, uniswap_v3_pool.token_a)
                        //     };

                        //     handles.push(tokio::spawn(async move {
                        //         let swap_amount_out = uniswap_v3_quoter
                        //             .quote_exact_input_single(
                        //                 token_in,
                        //                 token_out,
                        //                 pool.fee(),
                        //                 amount_in,
                        //                 U256::zero(),
                        //             )
                        //             .call()
                        //             .await?;
                        //         Result::<(U256, Pool), ExecutorError<M>>::Ok((swap_amount_out, pool))
                        //     }))
                    }
                };
            }

            // Construct SwapCallData
            let mut swap_data: SwapData = SwapData {
                token_in: None,
                token_out: None,
                amount_in: None,
                amount_out_min: None,
                protocol_fee: None,
                bribe,
                affiliate: U256::zero(),
                referrer: U256::zero(),
            };

            if token_in.is_zero() {
                swap_data.token_out = Some(token_out);
                swap_data.amount_out_min = Some(best_amount_out);
                swap_data.protocol_fee = Some(protocol_fee);
            } else if token_out.is_zero() {
                swap_data.token_in = Some(token_in);
                swap_data.amount_in = Some(amount_fixed_for_fee);
                swap_data.amount_out_min = Some(best_amount_out);
            } else {
                swap_data.token_in = Some(token_in);
                swap_data.token_out = Some(token_out);
                swap_data.amount_in = Some(amount_fixed_for_fee);
                swap_data.amount_out_min = Some(best_amount_out);
            };

            // Construct SwapMultiCall
            let mut swap_multicall: SwapMultiCall = SwapMultiCall {
                token_in_destination: to,
                calls: vec![],
            };

            match best_pool {
                Pool::UniswapV2(uniswapv2_pool) => {
                    let mut swap_bytes: Vec<u8> = vec![];

                    if token_in.is_zero() {
                        swap_bytes.extend(&H160::from_str(WETH).unwrap().encode());
                        swap_bytes.extend(&U256::from(UNISWAP_V2_FEE).encode());
                    } else if token_out.is_zero() {
                        swap_bytes.push(0);
                    } else {
                        swap_bytes.push(0);
                    }

                    let input_tokens = vec![
                        Token::Uint(amount_fixed_for_fee),
                        Token::Uint(U256::zero()),
                        Token::Address(to),
                        Token::Bytes(swap_bytes),
                    ];

                    let swap_calldata = IUNISWAPV2PAIR_ABI
                        .function("swap")?
                        .encode_input(&input_tokens)
                        .unwrap();

                    let mut hex_calldata = hex::encode(swap_calldata.clone());
                    hex_calldata.insert_str(0, "0x");

                    // Push V2 swap call
                    swap_multicall
                        .calls
                        .push((uniswapv2_pool.address, hex_calldata));

                    // Push Erc20 transfer call
                    let transfer_input =
                        vec![Token::Address(receiver), Token::Uint(best_amount_out)];
                    let transfer_calldata = IERC20_ABI
                        .function("transfer")?
                        .encode_input(&transfer_input)
                        .unwrap();
                    let mut hex_calldata = hex::encode(transfer_calldata);
                    hex_calldata.insert_str(0, "0x");
                    swap_multicall.calls.push((token_out, hex_calldata))
                }
                Pool::UniswapV3(_uniswapv3_pool) => {}
            }

            println!(
                "SwapData: {:?}\n, SwapMultiCall: {:?}",
                swap_data, swap_multicall
            );
        }
        None => {
            println!("No markets found!");
        }
    }

    // simulate_swap::try_sample_swap_simulate(rpc_endpoint.clone()).await?;
    // swap_calldata::try_swap_calldata(configuration.http_endpoint.clone()).await?;
    // discover_erc_4626_vaults::try_discorver_erc_4626_vaults(rpc_endpoint.clone()).await?;
    // discover_factories::try_discorver_factories(rpc_endpoint.clone()).await?;

    // sync_amms::try_sync_amms(configuration.http_endpoint.clone()).await?;

    Ok(())
}
