use std::{path::{Path, PathBuf}, sync::Arc};
use clap::Parser;
use reth::{
    api::NodeTypesWithDBAdapter,
    beacon_consensus::EthBeaconConsensus,
    providers::{
        providers::{BlockchainProvider, StaticFileProvider},
        ProviderFactory,
    },
    rpc::eth::EthApi,
    utils::open_db_read_only,
    rpc::builder::{
        RethRpcModule, RpcModuleBuilder, RpcServerConfig, TransportRpcModuleConfig,
    },
    blockchain_tree::noop::NoopBlockchainTree,
    tasks::TokioTaskExecutor,
};
use reth_chainspec::{ChainSpec, MAINNET, SEPOLIA, HOLESKY};
use reth_db::{redis::DatabaseArguments, ClientVersion, DatabaseEnv};
use reth_node_ethereum::{EthEvmConfig, EthExecutorProvider, EthereumNode};
use reth_node_ethereum::node::EthereumEngineValidator;
use reth_provider::{test_utils::TestCanonStateSubscriptions, ChainSpecProvider};

/// A helper function to pick a chain spec based on a user-supplied string.
/// Defaults to MAINNET if the chain name is unrecognized.
fn get_chain_spec(chain_name: &str) -> Arc<ChainSpec> {
    match chain_name.to_lowercase().as_str() {
        "mainnet" => MAINNET.clone(),
        "sepolia" => SEPOLIA.clone(),
        "holesky" => HOLESKY.clone(),
        _ => {
            eprintln!("Unsupported chain name: {chain_name}. Defaulting to MAINNET.");
            MAINNET.clone()
        }
    }
}

/// Command line options
#[derive(Parser, Debug)]
#[command(name = "rpc-node", author, version, about = "A sample node with a custom RPC method")]
struct Cli {
    /// Data directory for the node (contains the DB)
    #[arg(long, default_value = "./data")]
    datadir: PathBuf,

    /// Redis URL to connect to
    #[arg(long, default_value = "redis://127.0.0.1:6379")]
    redis_url: String,

    /// The HTTP port for the RPC server to listen on
    #[arg(long, default_value = "9090")]
    http_port: u16,

    /// Name of the chain to use (e.g., mainnet, sepolia, goerli, holesky)
    #[arg(long, default_value = "mainnet")]
    chain: String,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();

    // Choose the chain spec based on user input
    let spec = get_chain_spec(&cli.chain);

    // 1. Setup the DB
    let db_path = cli.datadir.join("db");
    let db = Arc::new(open_db_read_only(
        &cli.redis_url,
        Path::new(&db_path),
        DatabaseArguments::new(ClientVersion::default()),
    )?);

    let factory = ProviderFactory::<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>::new(
        db.clone(),
        spec.clone(),
        StaticFileProvider::read_only(cli.datadir.join("static_files"), true)?,
    );

    // 2. Setup the blockchain provider
    let provider = BlockchainProvider::new(factory, Arc::new(NoopBlockchainTree::default()))?;

    let rpc_builder = RpcModuleBuilder::default()
        .with_provider(provider.clone())
        // The following methods are placeholders for a minimal example
        .with_noop_pool()
        .with_noop_network()
        .with_executor(TokioTaskExecutor::default())
        .with_evm_config(EthEvmConfig::new(spec.clone()))
        .with_events(TestCanonStateSubscriptions::default())
        .with_block_executor(EthExecutorProvider::ethereum(provider.chain_spec()))
        .with_consensus(EthBeaconConsensus::new(spec.clone()));

    // Pick which namespaces to expose
    let config = TransportRpcModuleConfig::default().with_http([RethRpcModule::Eth]);
    let mut server = rpc_builder.build(
        config,
        Box::new(EthApi::with_spawner),
        Arc::new(EthereumEngineValidator::new(spec)),
    );

    // Start the server & keep it alive on the user-specified port
    let server_args = RpcServerConfig::http(Default::default())
        .with_http_address(format!("0.0.0.0:{}", cli.http_port).parse()?);
    println!("Starting RPC server with:");
    println!("  Chain: {}", cli.chain);
    println!("  Redis URL: {}", cli.redis_url);
    println!("  HTTP Port: {}", cli.http_port);
    let _handle = server_args.start(&server).await?;
    println!("RPC server started on port {}", cli.http_port);
    futures::future::pending::<()>().await;

    Ok(())
}
