use std::sync::Arc;

use ethers::{providers::Middleware, types::Bytes};

use crate::{
    abi::{self},
    config::{self},
    error::ExecutorError,
    execution,
};

//Construct a limit order execution transaction calldata
pub async fn lo_execution_calldata<M: Middleware>(
    configuration: &config::Config,
    order_ids: Vec<[u8; 32]>,
    middleware: Arc<M>,
) -> Result<Bytes, ExecutorError<M>> {
    let calldata = abi::ILimitOrderRouter::new(configuration.limit_order_book, middleware.clone())
        .execute_limit_orders(order_ids)
        .calldata()
        .unwrap();

    Ok(calldata)
}

//Construct a limit order execution transaction calldata
pub async fn slo_execution_calldata<M: Middleware>(
    configuration: &config::Config,
    slo_bundle: execution::sandbox_limit_order::SandboxLimitOrderExecutionBundle,
    middleware: Arc<M>,
) -> Result<Bytes, ExecutorError<M>> {
    let sandbox_limit_order_router = abi::ISandboxLimitOrderRouter::new(
        configuration.sandbox_limit_order_router,
        middleware.clone(),
    );

    let calldata = sandbox_limit_order_router
        .execute_sandbox_multicall(slo_bundle.to_sandbox_multicall())
        .calldata()
        .unwrap();

    Ok(calldata)
}
