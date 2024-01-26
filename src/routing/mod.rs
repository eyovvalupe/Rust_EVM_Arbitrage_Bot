use cfmms::pool::{Pool, UniswapV2Pool};
use ethers::providers::Middleware;
use ethers::types::{H160, U256};
use std::collections::HashMap;
use std::{str::FromStr, sync::Arc};

use crate::config::Config;
use crate::constants::WETH;
use crate::error::ExecutorError;
use crate::markets;

pub async fn find_markets_and_route<M: 'static + Middleware>(
    token_in: H160,
    token_out: H160,
    configuration: &Config,
    middleware: Arc<M>,
) -> Result<HashMap<H160, Pool>, ExecutorError<M>> {
    let markets = markets::get_market(
        match token_in.is_zero() {
            true => H160::from_str(WETH).unwrap(),
            false => token_in,
        },
        match token_out.is_zero() {
            true => H160::from_str(WETH).unwrap(),
            false => token_out,
        },
        &configuration.dexes,
        middleware,
    )
    .await?;

    match markets {
        Some(markets) => {
            println!("Found markets: {:?}", markets.keys());
            Ok(markets)
        }
        None => {
            println!("No markets found!");
            Err(ExecutorError::MarketDoesNotExistForPair(
                token_in, token_out,
            ))
        }
    }
}

pub async fn find_best_route<M: 'static + Middleware>(
    markets: HashMap<H160, Pool>,
    token_in: H160,
    amount: U256,
    middleware: Arc<M>,
) -> Result<(Pool, U256, Vec<Pool>), ExecutorError<M>> {
    let mut best_amount_out = U256::zero();
    let mut best_pool = Pool::UniswapV2(UniswapV2Pool::default());
    let mut handles: Vec<Pool> = vec![];

    for pool in markets.values() {
        let pool = *pool;
        match pool {
            Pool::UniswapV2(_) => {
                let swap_amount_out = pool
                    .simulate_swap(token_in, amount, middleware.clone())
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

    Ok((best_pool, best_amount_out, handles))
}
