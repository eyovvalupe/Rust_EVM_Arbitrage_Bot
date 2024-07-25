use cfmms::pool::{Pool, UniswapV2Pool};
use ethers::providers::Middleware;
use ethers::types::{H160, U256};
use std::collections::HashMap;
use std::hash::RandomState;
use std::{str::FromStr, sync::Arc};
use crate::{
    config::Config,
    constants::WETH,
    markets::{self, Market},
    abi::IUniswapV3Quoter,
    error::ExecutorError,
};
use futures::future::join_all;

pub const V3_QUOTER_ADDRESS: H160 = H160([
    178, 115, 8, 249, 249, 13, 96, 116, 99, 187, 51, 234, 27, 235, 180, 28, 39, 206, 90, 182,
]);

fn merge_option_hashmaps<K, V>(
    map1: Option<HashMap<K, V>>, 
    map2: Option<HashMap<K, V>>
) -> Option<HashMap<K, V>> where K: std::hash::Hash + Eq, V: Clone, {
    match (map1, map2) {
        (None, None) => None,
        (Some(map), None) | (None, Some(map)) => Some(map),
        (Some(mut map1), Some(map2)) => {
            map1.extend(map2);
            Some(map1)
        }
    }
}

pub async fn find_a_to_b_markets_and_route<M: 'static + Middleware>(
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
            println!("Found markets in A-B: {:?}", markets.keys());
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

pub async fn find_best_a_to_b_route<M: 'static + Middleware>(
    markets: HashMap<H160, Pool>,
    token_in: H160,
    amount: U256,
    middleware: Arc<M>,
) -> Result<(Pool, U256), ExecutorError<M>> {
    let mut best_amount_out = U256::zero();
    let mut best_pool = Pool::UniswapV2(UniswapV2Pool::default());
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

            Pool::UniswapV3(uniswap_v3_pool) => {
                let uniswap_v3_quoter =
                    IUniswapV3Quoter::new(V3_QUOTER_ADDRESS, middleware.clone());

                let (token_in, token_out) = if token_in == uniswap_v3_pool.token_a {
                    (uniswap_v3_pool.token_a, uniswap_v3_pool.token_b)
                } else {
                    (uniswap_v3_pool.token_b, uniswap_v3_pool.token_a)
                };

                let swap_amount_out = uniswap_v3_quoter
                    .quote_exact_input_single(
                        token_in,
                        token_out,
                        pool.fee(),
                        amount,
                        U256::zero(),
                    )
                    .call()
                    .await?;

                if swap_amount_out > best_amount_out {
                    best_amount_out = swap_amount_out;
                    best_pool = pool;
                }
            }
        };
    }

    Ok((best_pool, best_amount_out))
}

pub async fn find_a_to_x_to_b_markets_and_route<M: 'static + Middleware>(
    token_in: H160,
    token_out: H160,
    token_x: H160,
    configuration: &Config,
    middleware: Arc<M>,
) -> Result<HashMap<U256, markets::Market>, ExecutorError<M>> {
    let markets = markets::get_market_x(
        match token_in.is_zero() {
            true => H160::from_str(WETH).unwrap(),
            false => token_in,
        },
        match token_x.is_zero() {
            true => H160::from_str(WETH).unwrap(),
            false => token_x,
        },
        &configuration.dexes,
        middleware.clone(),
    )
    .await?;
    
    let temp_markets = markets::get_market_x(
        match token_x.is_zero() {
            true => H160::from_str(WETH).unwrap(),
            false => token_x,
        },
        match token_out.is_zero() {
            true => H160::from_str(WETH).unwrap(),
            false => token_out,
        },
        &configuration.dexes,
        middleware.clone(),
    )
    .await?;

    let result = merge_option_hashmaps(markets, temp_markets);
    
    match result {
        Some(result) => {
            println!("Found markets in A-X-B: {:?}", result.keys());
            Ok(result)
        }
        None => {
            println!("No markets found!");
            Err(ExecutorError::MarketDoesNotExistForPair(
                token_in, token_out,
            ))
        }
    }
}

pub async fn find_best_a_to_x_to_b_route<M: 'static + Middleware>(
    token_in: H160,
    token_out: H160,
    token_x: H160,
    amount_in: U256,
    simulated_markets: &HashMap<U256, HashMap<H160, Pool>>,
    middleware: Arc<M>,
) -> Result<(Vec<U256>, Vec<U256>, Vec<Pool>), ExecutorError<M>> {
    let markets_in_route: Vec<&Market> = {
        // Simulate order along route for token_a -> weth -> token_b
        let a_to_x_market = simulated_markets.get(&markets::get_market_id(token_in, token_x));
        let x_to_b_market = simulated_markets.get(&markets::get_market_id(token_x, token_out));

        if a_to_x_market.is_some() && x_to_b_market.is_some() {
            let a_to_x_market = a_to_x_market.unwrap();
            let x_to_b_market = x_to_b_market.unwrap();

            vec![a_to_x_market, x_to_b_market]
        } else if a_to_x_market.is_none() {
            return Err(ExecutorError::MarketDoesNotExistForPair(token_in, token_x));
        } else {
            //x to b market is none
            return Err(ExecutorError::MarketDoesNotExistForPair(token_x, token_out));
        }
    };

    find_best_route_across_markets(amount_in, token_in, markets_in_route, middleware.clone()).await
}

//Returns the amounts in, amount out and a reference to the pools that it took through the route
pub async fn find_best_route_across_markets<M: 'static + Middleware>(
    amount_in: U256,
    mut token_in: H160,
    markets: Vec<&Market>,
    middleware: Arc<M>,
) -> Result<(Vec<U256>, Vec<U256>, Vec<Pool>), ExecutorError<M>> {
    let mut amount_in = amount_in;
    let mut amounts_in: Vec<U256> = vec![];
    let mut amounts_out: Vec<U256> = vec![];
    let mut route: Vec<Pool> = vec![];
    for market in markets {
        //TODO: apply tax to amount in
        let mut best_amount_out = U256::zero();
        let mut best_pool = Pool::UniswapV2(UniswapV2Pool::default());

        amounts_in.push(amount_in);

        let mut handles = vec![];

        for pool in market.values() {
            let pool = *pool;
            match pool {
                Pool::UniswapV2(_) => {
                    let swap_amount_out = pool
                        .simulate_swap(token_in, amount_in, middleware.clone())
                        .await?;
                    if swap_amount_out > best_amount_out {
                        best_amount_out = swap_amount_out;
                        best_pool = pool;
                    }
                }

                Pool::UniswapV3(uniswap_v3_pool) => {
                    let uniswap_v3_quoter =
                        IUniswapV3Quoter::new(V3_QUOTER_ADDRESS, middleware.clone());

                    let (token_in, token_out) = if token_in == uniswap_v3_pool.token_a {
                        (uniswap_v3_pool.token_a, uniswap_v3_pool.token_b)
                    } else {
                        (uniswap_v3_pool.token_b, uniswap_v3_pool.token_a)
                    };

                    handles.push(tokio::spawn(async move {
                        let swap_amount_out = uniswap_v3_quoter
                            .quote_exact_input_single(
                                token_in,
                                token_out,
                                pool.fee(),
                                amount_in,
                                U256::zero(),
                            )
                            .call()
                            .await?;
                        Result::<(U256, Pool), ExecutorError<M>>::Ok((swap_amount_out, pool))
                    }))
                }
            };
        }

        for join_result in join_all(handles).await {
            match join_result {
                Ok(ok) => {
                    if let Ok((swap_amount_out, pool)) = ok {
                        if swap_amount_out > best_amount_out {
                            best_amount_out = swap_amount_out;
                            best_pool = pool;
                        }
                    }
                }
                Err(_err) => {}
            }
        }

        amount_in = best_amount_out;
        amounts_out.push(best_amount_out);
        route.push(best_pool);

        //update token in
        //Get the token out from the market to set as the new token in, we can use any pool in the market since the token out and token in for each pool in the market are the same.
        // Have the same token in and out to be in the same market.
        token_in = match market.values().next().unwrap() {
            Pool::UniswapV2(uniswap_v2_pool) => {
                if uniswap_v2_pool.token_a == token_in {
                    uniswap_v2_pool.token_b
                } else {
                    uniswap_v2_pool.token_a
                }
            }
            Pool::UniswapV3(uniswap_v3_pool) => {
                if uniswap_v3_pool.token_a == token_in {
                    uniswap_v3_pool.token_b
                } else {
                    uniswap_v3_pool.token_a
                }
            }
        };
    }

    Ok((amounts_in, amounts_out, route))
}

pub async fn find_all_markets<M: 'static + Middleware>(
    token_in: H160,
    token_out: H160,
    configuration: &Config,
    middleware: Arc<M>,
) -> Result<Vec<Pool>, ExecutorError<M>> {
    println!("this is the position before call the get_all_markets function\n");
    let markets = markets::get_all_markets(
        configuration.dexes.clone(),
        middleware,
    )
    .await?;

    println!("this is the all markets =================> {:?}\n",markets);
// Ok(markets);
    match markets {
        (markets) => {
            // println!("Found markets: {:?}", markets.keys());
            Ok(markets)
        }
        // None => {
        //     println!("No markets found!");
        //     Err(ExecutorError::MarketDoesNotExistForPair(
        //         token_in, token_out,
        //     ))
        // }
    }
}