//! Run with
//!
//! ```not_rust
//! cargo run -p rpc-node
//! ```
//!
//! This installs an additional RPC method `myrpcExt_customMethod` that can queried via [cast](https://github.com/foundry-rs/foundry)
//!
//! ```sh
//! cast rpc myrpcExt_customMethod
//! ```

use std::{path::Path, sync::Arc};

use reth::{
    api::NodeTypesWithDBAdapter,
    beacon_consensus::EthBeaconConsensus,
    providers::{
        providers::{BlockchainProvider, StaticFileProvider},
        ProviderFactory,
    },
    rpc::eth::EthApi,
    utils::open_db_read_only,
};
use reth_chainspec::ChainSpecBuilder;
use reth_db::{redis::DatabaseArguments, ClientVersion, DatabaseEnv};

// Bringing up the RPC
use reth::rpc::builder::{
    RethRpcModule, RpcModuleBuilder, RpcServerConfig, TransportRpcModuleConfig,
};
use reth::{blockchain_tree::noop::NoopBlockchainTree, tasks::TokioTaskExecutor};
use reth_node_ethereum::{EthEvmConfig, EthExecutorProvider, EthereumNode};
use reth_node_ethereum::node::EthereumEngineValidator;
use reth_provider::{test_utils::TestCanonStateSubscriptions, ChainSpecProvider};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // 1. Setup the DB
    let db_path = std::env::var("RETH_DB_PATH")?;
    let db_path = Path::new(&db_path);
    let redis_url = "redis://chainnodetestdt.psi2zx.ng.0001.use1.cache.amazonaws.com:6379";
    let db = Arc::new(open_db_read_only(
        &redis_url.to_string(),
        db_path.join("db").as_path(),
        DatabaseArguments::new(ClientVersion::default()),
    )?);
    let spec = Arc::new(ChainSpecBuilder::mainnet().build());
    let factory = ProviderFactory::<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>::new(
        db.clone(),
        spec.clone(),
        StaticFileProvider::read_only(db_path.join("static_files"), true)?,
    );

    // 2. Setup the blockchain provider using only the database provider and a noop for the tree to
    //    satisfy trait bounds. Tree is not used in this example since we are only operating on the
    //    disk and don't handle new blocks/live sync etc, which is done by the blockchain tree.
    let provider = BlockchainProvider::new(factory, Arc::new(NoopBlockchainTree::default()))?;

    let rpc_builder = RpcModuleBuilder::default()
        .with_provider(provider.clone())
        // Rest is just noops that do nothing
        .with_noop_pool()
        .with_noop_network()
        .with_executor(TokioTaskExecutor::default())
        .with_evm_config(EthEvmConfig::new(spec.clone()))
        .with_events(TestCanonStateSubscriptions::default())
        .with_block_executor(EthExecutorProvider::ethereum(provider.chain_spec()))
        .with_consensus(EthBeaconConsensus::new(spec.clone()));

    // Pick which namespaces to expose.
    let config = TransportRpcModuleConfig::default().with_http([RethRpcModule::Eth]);
    let mut server = rpc_builder.build(
        config,
        Box::new(EthApi::with_spawner),
        Arc::new(EthereumEngineValidator::new(spec)),
    );

    // Start the server & keep it alive
    let server_args =
        RpcServerConfig::http(Default::default()).with_http_address("0.0.0.0:9090".parse()?);
    let _handle = server_args.start(&server).await?;
    futures::future::pending::<()>().await;

    Ok(())
}
