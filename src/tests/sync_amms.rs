use amms::{
    amm::{
        factory::Factory, uniswap_v2::factory::UniswapV2Factory,
        uniswap_v3::factory::UniswapV3Factory, AMM,
    },
    sync,
};
use ethers::{
    providers::{Http, Provider},
    types::H160,
};
use std::{env, str::FromStr, sync::Arc};

use crate::constants::*;

pub async fn try_sync_amms(rpc_endpoint: String) -> eyre::Result<()> {
    let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);

    let factories = vec![
        //Add UniswapV2
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            H160::from_str(UNISWAP_V2)?,
            UNISWAP_V2_CREATION_BLOCK,
            UNISWAP_V2_FEE,
        )),
        //Add Sushiswap
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            H160::from_str(SUSHISWAP)?,
            SUSHISWAP_CREATION_BLOCK,
            SUSHISWAP_FEE,
        )),
        //Add UniswapV3
        Factory::UniswapV3Factory(UniswapV3Factory::new(
            H160::from_str(UNISWAP_V3)?,
            UNISWAP_V3_CREATION_BLOCK,
        )),
    ];

    let current_dir = env::current_dir()?;
    let sushi_cp_path = current_dir.join("checkpoints/sushiswap.json");
    let uni2_cp_path = current_dir.join("checkpoints/uniswap2.json");
    let uni3_cp_path = current_dir.join("checkpoints/uniswap3.json");
    println!(
        "Checkpoint paths: {:?}",
        vec![
            sushi_cp_path.clone(),
            uni2_cp_path.clone(),
            uni3_cp_path.clone()
        ]
    );

    //Sync uniswap v2 pairs
    let (amms, last_sync_block): (Vec<AMM>, u64) = sync::sync_amms(
        factories[..1].to_vec(),
        middleware.clone(),
        uni2_cp_path.to_str(),
        500,
    )
    .await?;

    println!(
        "Sync uniswap v2 again and found {:?} amms at {:?}",
        amms.len(),
        last_sync_block
    );

    // //Sync sushiswap pairs
    // let (amms, last_sync_block): (Vec<AMM>, u64) = sync::sync_amms(
    //     factories[1..2].to_vec(),
    //     middleware.clone(),
    //     sushi_cp_path.to_str(),
    //     500,
    // )
    // .await?;

    // println!(
    //     "Sync sushiswap again and found {:?} amms at {:?}",
    //     amms.len(),
    //     last_sync_block
    // );

    // //Sync uniswap v3 pairs
    // let (amms, last_sync_block): (Vec<AMM>, u64) = sync::sync_amms(
    //     factories[2..].to_vec(),
    //     middleware.clone(),
    //     uni3_cp_path.to_str(),
    //     500,
    // )
    // .await?;

    // println!(
    //     "Sync uniswap v3 again and found {:?} amms at {:?}",
    //     amms.len(),
    //     last_sync_block
    // );

    Ok(())
}
