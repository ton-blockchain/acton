//! Command-line interface and dispatch helpers for the launcher executable.
//!
//! This module defines the Clap command tree and keeps command handlers in the
//! adjacent [`commands`] module. Blockchain operations themselves live in their
//! respective domain modules and are called from `main` after argument parsing.

pub(crate) mod commands;

use std::{net::Ipv4Addr, path::PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "localton",
    version,
    about = "Run and operate a complete headless local TON development network"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Options for the default `run` command.
    #[command(flatten)]
    pub run: RunArgs,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Start the network and all enabled headless services.
    Run(RunArgs),
    /// Inspect persisted and live network status.
    Status(StatusArgs),
    /// Read or update persistent network configuration.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Query the liteserver or send an external message.
    Lite {
        #[command(subcommand)]
        command: LiteCommand,
    },
    /// Create and use local wallets.
    Wallet {
        #[command(subcommand)]
        command: WalletCommand,
    },
    /// Prepare local chain state for indexer services.
    Indexer {
        #[command(subcommand)]
        command: IndexerCommand,
    },
    /// Manage full nodes and validators.
    Node {
        #[command(subcommand)]
        command: NodeCommand,
    },
    /// Save and restore persistent network state.
    Snapshot {
        #[command(subcommand)]
        command: SnapshotCommand,
    },
    /// Manage validator elections, keys, stakes, and rewards.
    Validator {
        #[command(subcommand)]
        command: ValidatorCommand,
    },
    /// Create and inspect a hardfork configuration.
    Hardfork(HardforkArgs),
}

#[derive(Debug, Clone, Args)]
pub struct StateArgs {
    /// Persistent network state.
    #[arg(long, default_value = ".localton", global = true)]
    pub state_dir: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct SnapshotArgs {
    #[command(flatten)]
    pub state: StateArgs,

    /// Snapshot storage. Defaults to a sibling of the state directory.
    #[arg(long, global = true)]
    pub snapshot_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SnapshotCommand {
    /// Save a compressed cold snapshot.
    Create {
        #[command(flatten)]
        paths: SnapshotArgs,
        /// Optional label shown in snapshot listings.
        #[arg(long)]
        name: Option<String>,
    },
    /// List saved snapshots as JSON.
    List {
        #[command(flatten)]
        paths: SnapshotArgs,
    },
    /// Restore a snapshot into the state directory.
    Restore {
        #[command(flatten)]
        paths: SnapshotArgs,
        id: String,
    },
    /// Delete a saved snapshot.
    Delete {
        #[command(flatten)]
        paths: SnapshotArgs,
        id: String,
    },
}

#[derive(Debug, Clone, Args)]
pub struct RunArgs {
    /// Persistent network state. It is created on the first run and reused.
    #[arg(long, default_value = ".localton")]
    pub state_dir: PathBuf,

    /// Use an existing directory with official TON binaries instead of downloading them.
    #[arg(long, env = "TON_BIN_DIR")]
    pub ton_bin_dir: Option<PathBuf>,

    /// Maximum bootstrap/readiness wait in seconds.
    #[arg(long, default_value_t = 180)]
    pub startup_timeout: u64,

    /// Total number of validators to enable, including genesis.
    #[arg(long, value_parser = clap::value_parser!(usize))]
    pub validators: Option<usize>,

    /// Add an active basechain account from a hex-encoded ShardAccount BoC.
    ///
    /// May be specified more than once. Only used when creating a new state.
    #[arg(long, value_name = "SHARD_ACCOUNT_HEX")]
    pub add_account: Vec<String>,

    /// Start the compatible ton-http-api bridge.
    #[arg(long)]
    pub ton_http_api: bool,

    /// Address for the browser-facing TON HTTP API proxy.
    #[arg(
        long,
        env = "LOCALTON_HTTP_API_BIND",
        default_value_t = Ipv4Addr::LOCALHOST
    )]
    pub ton_http_api_bind: Ipv4Addr,

    /// Runtime-only bind address for the configuration HTTP API.
    #[arg(long, env = "LOCALTON_CONFIG_HTTP_BIND")]
    pub config_http_bind: Option<Ipv4Addr>,

    /// Runtime-only bind address for the administrative HTTP API.
    #[arg(long, env = "LOCALTON_ADMIN_HTTP_BIND")]
    pub admin_http_bind: Option<Ipv4Addr>,

    /// Runtime-only TON HTTP API V2 executable override.
    #[arg(long, env = "LOCALTON_HTTP_API_COMMAND")]
    pub ton_http_api_command: Option<PathBuf>,

    /// Runtime-only TON HTTP API static configuration override.
    #[arg(long, env = "LOCALTON_HTTP_API_STATIC_CONFIG")]
    pub ton_http_api_static_config: Option<PathBuf>,

    /// Do not serve global config and liveness endpoints.
    #[arg(long)]
    pub no_config_http: bool,

    /// Do not start the local administrative HTTP API.
    #[arg(long)]
    pub no_admin_http: bool,
}

impl Default for RunArgs {
    fn default() -> Self {
        Self {
            state_dir: PathBuf::from(".localton"),
            ton_bin_dir: None,
            startup_timeout: 180,
            validators: None,
            add_account: Vec::new(),
            ton_http_api: false,
            ton_http_api_bind: Ipv4Addr::LOCALHOST,
            config_http_bind: None,
            admin_http_bind: None,
            ton_http_api_command: None,
            ton_http_api_static_config: None,
            no_config_http: false,
            no_admin_http: false,
        }
    }
}

#[derive(Debug, Clone, Args)]
pub struct StatusArgs {
    #[command(flatten)]
    pub state: StateArgs,

    /// Emit a machine-readable JSON document.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigCommand {
    /// Create settings.json if it does not exist and print its path.
    Init(StateArgs),
    /// Print the effective settings.
    Show {
        #[command(flatten)]
        state: StateArgs,
    },
    /// Validate settings.json without starting the network.
    Validate(StateArgs),
    /// Enable a total number of validators in settings.json.
    Validators {
        #[command(flatten)]
        state: StateArgs,
        #[arg(value_parser = clap::value_parser!(usize))]
        count: usize,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum LiteCommand {
    /// Print the latest masterchain block.
    Last {
        #[command(flatten)]
        state: StateArgs,
    },
    /// Print decoded account state.
    Account {
        #[command(flatten)]
        state: StateArgs,
        address: String,
    },
    /// Execute a get method and print its TVM stack.
    RunMethod {
        #[command(flatten)]
        state: StateArgs,
        address: String,
        method: String,
        /// TVM stack in lite-client syntax, for example `0` or `[ 1 2 ]`.
        #[arg(default_value = "")]
        params: String,
    },
    /// Send an external message BoC.
    Send {
        #[command(flatten)]
        state: StateArgs,
        boc: PathBuf,
    },
    /// Print a block by workchain, shard, and seqno.
    Block {
        #[command(flatten)]
        state: StateArgs,
        workchain: i32,
        shard: String,
        seqno: u32,
    },
    /// List transaction identifiers from a block.
    Transactions {
        #[command(flatten)]
        state: StateArgs,
        workchain: i32,
        shard: String,
        seqno: u32,
        #[arg(long, default_value_t = 256)]
        count: u32,
    },
    /// Print all shards at the latest masterchain block.
    Shards {
        #[command(flatten)]
        state: StateArgs,
    },
    /// Print blockchain configuration parameters.
    Config {
        #[command(flatten)]
        state: StateArgs,
        params: Vec<i32>,
    },
    /// Execute a raw official lite-client command.
    Exec {
        #[command(flatten)]
        state: StateArgs,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum WalletVersion {
    V1,
    V2,
    V3,
    V4r2,
    V5r1,
    Highload,
}

#[derive(Debug, Clone, Subcommand)]
pub enum WalletCommand {
    /// List genesis and user-created wallets without revealing private keys.
    List {
        #[command(flatten)]
        state: StateArgs,
    },
    /// Create a wallet and persist its key only under the state directory.
    Create {
        #[command(flatten)]
        state: StateArgs,
        name: String,
        #[arg(long, value_enum, default_value_t = WalletVersion::V3)]
        version: WalletVersion,
        #[arg(long, default_value_t = 0)]
        workchain: i32,
        #[arg(long, default_value_t = 42)]
        wallet_id: u32,
    },
    /// Transfer local Grams from a managed wallet.
    Send {
        #[command(flatten)]
        state: StateArgs,
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        /// Decimal Gram amount, for example 1.25.
        #[arg(long)]
        amount: String,
        #[arg(long)]
        comment: Option<String>,
        #[arg(long)]
        body: Option<PathBuf>,
        #[arg(long)]
        state_init: Option<PathBuf>,
        #[arg(long, default_value_t = 3)]
        mode: u8,
        #[arg(long)]
        no_bounce: bool,
    },
    /// Fund a wallet from the genesis faucet.
    Fund {
        #[command(flatten)]
        state: StateArgs,
        wallet: String,
        /// Decimal Gram amount, for example 100.
        amount: String,
    },
    /// Print a wallet balance and seqno.
    Info {
        #[command(flatten)]
        state: StateArgs,
        wallet: String,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum IndexerCommand {
    /// Ensure the basechain has a block and save a masterchain seqno for account scanning.
    BootstrapBasechain {
        #[command(flatten)]
        state: StateArgs,
        /// TON HTTP API V2 endpoint.
        #[arg(long, default_value = "http://127.0.0.1:18002/api/v2")]
        endpoint: String,
        /// Managed workchain 0 wallet used only to create the first basechain block.
        #[arg(long, default_value = "studio-indexer-bootstrap")]
        wallet: String,
        /// Grams transferred from the genesis faucet when the basechain is empty.
        #[arg(long, default_value = "1")]
        amount: String,
        /// File that receives the indexable masterchain seqno.
        #[arg(long)]
        seqno_file: Option<PathBuf>,
        /// Maximum time to wait for the first basechain block.
        #[arg(long, default_value_t = 120)]
        timeout: u64,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum NodeCommand {
    /// List configured nodes and their live status.
    List {
        #[command(flatten)]
        state: StateArgs,
    },
    /// Enable and start a predefined node.
    Add {
        #[command(flatten)]
        state: StateArgs,
        name: String,
        #[arg(long)]
        fullnode_only: bool,
        #[arg(long, default_value_t = true)]
        liteserver: bool,
    },
    /// Start an enabled node.
    Start {
        #[command(flatten)]
        state: StateArgs,
        name: String,
    },
    /// Stop a node while leaving its state intact.
    Stop {
        #[command(flatten)]
        state: StateArgs,
        name: String,
    },
    /// Disable a node and optionally delete only that node's generated state.
    Remove {
        #[command(flatten)]
        state: StateArgs,
        name: String,
        #[arg(long)]
        delete_state: bool,
    },
    /// Print validator-engine-console getstats output.
    Stats {
        #[command(flatten)]
        state: StateArgs,
        #[arg(default_value = "genesis")]
        name: String,
    },
    /// Execute a raw validator-engine-console command.
    Console {
        #[command(flatten)]
        state: StateArgs,
        name: String,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum ValidatorCommand {
    /// Print elections and validator sets.
    Status {
        #[command(flatten)]
        state: StateArgs,
    },
    /// Create keys and submit an election participation request.
    Participate {
        #[command(flatten)]
        state: StateArgs,
        #[arg(default_value = "genesis")]
        node: String,
        #[arg(long)]
        election_id: Option<u32>,
    },
    /// Recover an unfrozen stake and rewards.
    Reap {
        #[command(flatten)]
        state: StateArgs,
        #[arg(default_value = "genesis")]
        node: String,
    },
    /// Run participation for every enabled validator.
    ParticipateAll {
        #[command(flatten)]
        state: StateArgs,
    },
    /// Recover stakes for every enabled validator.
    ReapAll {
        #[command(flatten)]
        state: StateArgs,
    },
}

#[derive(Debug, Clone, Args)]
pub struct HardforkArgs {
    #[command(flatten)]
    pub state: StateArgs,
    /// Source node whose latest block becomes the hardfork anchor.
    #[arg(long, default_value = "genesis")]
    pub node: String,
    /// Existing external message BoC to include while creating the fork block.
    #[arg(long)]
    pub external_message: Option<PathBuf>,
    /// Output global configuration. Defaults under the state directory.
    #[arg(long)]
    pub output: Option<PathBuf>,
}
