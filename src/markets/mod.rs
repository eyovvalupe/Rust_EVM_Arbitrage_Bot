use std::{collections::HashMap, sync::{Arc, Mutex}, panic::resume_unwind};

use cfmms::{checkpoint, dex::Dex, errors::CFMMError, pool::Pool, throttle::RequestThrottle};
use ethers::{
    providers::Middleware,
    types::{H160, U256},
    utils::keccak256,
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
// pub(crate) mod AllPools;
// use futures::io::AllowStdIo;
// use AllPools::get_pools;

use crate::{
    error::ExecutorError,
    markets
};

pub type Market = HashMap<H160, Pool>;

pub fn get_market_id(token_a: H160, token_b: H160) -> U256 {
    if token_a > token_b {
        U256::from_little_endian(&keccak256(
            vec![token_a.as_bytes(), token_b.as_bytes()].concat(),
        ))
    } else {
        U256::from_little_endian(&keccak256(
            vec![token_b.as_bytes(), token_a.as_bytes()].concat(),
        ))
    }
}

pub async fn get_market_x<M: 'static + Middleware>(
    token_a: H160,
    token_b: H160,
    dexes: &[Dex],
    middleware: Arc<M>,
) -> Result<Option<HashMap<U256, markets::Market>>, ExecutorError<M>> {
    let mut market = HashMap::new();
    let mut simulated_markets: HashMap<U256, markets::Market> = HashMap::new();

    for dex in dexes {
        if let Some(pools) = dex
            .get_all_pools_for_pair(token_a, token_b, middleware.clone())
            .await?
        {
            let market_id = markets::get_market_id(token_a, token_b);
            for pool in pools {
                market.insert(pool.address(), pool);
            }
            simulated_markets.insert(market_id, market.clone());
        }
    }

    if !simulated_markets.is_empty() {
        Ok(Some(simulated_markets))
    } else {
        Ok(None)
    }
}

pub async fn get_market<M: 'static + Middleware>(
    token_a: H160,
    token_b: H160,
    dexes: &[Dex],
    middleware: Arc<M>,
) -> Result<Option<HashMap<H160, Pool>>, ExecutorError<M>> {
    let mut market = HashMap::new();
    println!("this is the first step of find route");

    for dex in dexes {
        if let Some(pools) = dex
            .get_all_pools_for_pair(token_a, token_b, middleware.clone())
            .await?
        {
            for pool in pools {
                market.insert(pool.address(), pool);
            }
        }
    }
            println!("this is the first step of find route");

    if !market.is_empty() {
        Ok(Some(market))
    } else {
        Ok(None)
    }
}

pub fn get_best_market_price(
    buy: bool,
    base_token: H160,
    quote_token: H160,
    markets: &HashMap<U256, HashMap<H160, Pool>>,
) -> f64 {
    let mut best_price = if buy { f64::MAX } else { 0.0 };

    let market_id = get_market_id(base_token, quote_token);
    if let Some(market) = markets.get(&market_id) {
        for (_, pool) in market {
            let price = pool.calculate_price(base_token).unwrap_or(0.0);

            if buy {
                if price < best_price {
                    best_price = price;
                }
            } else if price > best_price {
                best_price = price;
            }
        }
    }

    best_price
}

pub async fn get_all_markets<M: 'static + Middleware>(
    dexes: Vec<Dex>,
    middleware: Arc<M>,
) -> Result<Vec<Pool>, CFMMError<M>> {
    // let mut market = HashMap::new();

    // let pools = AllowStdIo::get_pools(token_a, token_b);

    println!("THIS IS THE START OF THE GETTING ALL MARKETS");

    let current_block = middleware
                                .get_block_number()
                                .await
                                .map_err(CFMMError::MiddlewareError)?;
    //Initialize a new request throttle
    let request_throttle = Arc::new(Mutex::new(RequestThrottle::new(10)));

    //Aggregate the populated pools from each thread
    let mut aggregated_pools: Vec<Pool> = vec![];
    let mut handles = vec![];

    //Initialize multi progress bar
    let multi_progress_bar = MultiProgress::new();

    //For each dex supplied, get all pair created events and get reserve values
    for dex in dexes.clone() {
        let async_provider = middleware.clone();
        let request_throttle = request_throttle.clone();
        let progress_bar = multi_progress_bar.add(ProgressBar::new(0));

        handles.push(tokio::spawn(async move {
            progress_bar.set_style(
                ProgressStyle::with_template("{msg} {bar:40.cyan/blue} {pos:>7}/{len:7} Pairs")
                    .unwrap()
                    .progress_chars("##-"),
            );

            let mut pools = dex
                .get_all_pools(
                    request_throttle.clone(),
                    100000000,
                    progress_bar.clone(),
                    async_provider.clone(),
                )
                .await?;
            // println!("this is the all pools of specific dex ==============> {:?}\n", pools);

            progress_bar.reset();
            progress_bar.set_style(
                ProgressStyle::with_template("{msg} {bar:40.cyan/blue} {pos:>7}/{len:7} Block")
                    .unwrap()
                    .progress_chars("##-"),
            );

            dex.get_all_pool_data(
                &mut pools,
                request_throttle.clone(),
                progress_bar.clone(),
                async_provider.clone(),
            )
            .await?;

            progress_bar.finish_and_clear();
            progress_bar.set_message(format!(
                "Finished syncing pools for {} ✅",
                dex.factory_address()
            ));

            progress_bar.finish();

            Ok::<_, CFMMError<M>>(pools)
        }));
    }

    for handle in handles {
        match handle.await {
            Ok(sync_result) => aggregated_pools.extend(sync_result?),
            Err(err) => {
                {
                    if err.is_panic() {
                        // Resume the panic on the main task
                        resume_unwind(err.into_panic());
                    }
                }
            }
        }
    }
    // let checkpoint_path = Some("");
    // //Save a checkpoint if a path is provided
    // // if checkpoint_path.is_some() {
    //     let checkpoint_path = checkpoint_path.unwrap();

    //     checkpoint::construct_checkpoint(
    //         dexes,
    //         &aggregated_pools,
    //         current_block.as_u64(),
    //         checkpoint_path,
    //     );
    // // }

    // //Return the populated aggregated pools vec
    println!("THIS IS THE END OF THE GETTING ALL MARKETS");


    Ok(aggregated_pools)    
}