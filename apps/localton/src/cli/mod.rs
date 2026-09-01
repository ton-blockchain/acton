//! Command-line interface and dispatch helpers for the Localton executable.
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
    about = "Bootstrap and operate a complete headless local TON development network"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Create or resume a network and run its genesis node
    Bootstrap(BootstrapArgs),
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
    /// Inspect full nodes owned by one local state directory.
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
pub struct BootstrapArgs {
    /// Persistent network state. It is created on the first run and reused.
    #[arg(long, default_value = ".localton")]
    pub state_dir: PathBuf,

    /// Use an existing directory with official TON binaries instead of downloading them.
    #[arg(long, env = "TON_BIN_DIR")]
    pub ton_bin_dir: Option<PathBuf>,

    /// Maximum bootstrap/readiness wait in seconds.
    #[arg(long, default_value_t = 180)]
    pub startup_timeout: u64,

    /// Target interval between Simplex blocks, in milliseconds
    ///
    /// Localton writes this value as noncritical key 0 (target_rate) in TON config
    /// parameter 30 when it creates the network zerostate. It controls the target
    /// block-production pace, but actual intervals can be longer when a slot is
    /// skipped or consensus is delayed
    ///
    /// The value is part of zerostate. To change it, create a new network state
    #[arg(long, value_name = "MILLISECONDS", value_parser = clap::value_parser!(u32).range(1..))]
    pub block_time: Option<u32>,

    /// Validator round duration for a new network, in seconds
    ///
    /// Localton writes the duration and its derived election windows to TON config
    /// parameter 15 when it creates the network zerostate. Elections open three
    /// quarters of a round before the validator set changes and close one quarter
    /// before the change
    ///
    /// For example, --election-time 120 creates a two-minute round with elections
    /// open from 90 to 30 seconds before the validator set changes. The stake freeze
    /// period is also 30 seconds, and the first election can begin immediately
    ///
    /// Without this flag the round duration is 120 seconds
    ///
    /// Minimum: 4 seconds
    ///
    /// To change the value, create a new network state
    #[arg(long, value_name = "SECONDS", value_parser = clap::value_parser!(u32).range(4..))]
    pub election_time: Option<u32>,

    /// IPv4 address advertised by the genesis DHT and liteserver.
    ///
    /// Set this before first bootstrap when nodes on other hosts must join.
    #[arg(long, env = "LOCALTON_ADVERTISE_IP")]
    pub advertise_ip: Option<Ipv4Addr>,

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

impl Default for BootstrapArgs {
    fn default() -> Self {
        Self {
            state_dir: PathBuf::from(".localton"),
            ton_bin_dir: None,
            startup_timeout: 180,
            block_time: None,
            election_time: None,
            advertise_ip: None,
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
        /// Integer TVM stack arguments in decimal or `0x` hexadecimal form.
        params: Vec<String>,
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
    /// Print the on-chain validator election schedule and sets.
    Elections {
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
    /// List host-local nodes and their live status.
    List {
        #[command(flatten)]
        state: StateArgs,
    },
    /// Print validator-engine-console getstats output.
    Stats {
        #[command(flatten)]
        state: StateArgs,
        #[arg(default_value = "genesis")]
        name: String,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum ValidatorCommand {
    /// Print elections and validator sets.
    Status {
        #[command(flatten)]
        state: StateArgs,
    },
    /// Participate in future elections without restarting the full node.
    Enable {
        #[command(flatten)]
        state: StateArgs,
        node: Option<String>,
    },
    /// Stop participating in future elections and remain a full node.
    Disable {
        #[command(flatten)]
        state: StateArgs,
        node: Option<String>,
    },
    /// Create keys and submit an election participation request.
    Participate {
        #[command(flatten)]
        state: StateArgs,
        node: Option<String>,
        #[arg(long)]
        election_id: Option<u32>,
    },
    /// Recover an unfrozen stake and rewards.
    Reap {
        #[command(flatten)]
        state: StateArgs,
        node: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_accepts_block_time_in_milliseconds() {
        let cli = Cli::try_parse_from(["localton", "bootstrap", "--block-time", "750"]).unwrap();
        let Command::Bootstrap(args) = cli.command else {
            panic!("expected bootstrap command");
        };
        assert_eq!(args.block_time, Some(750));
        assert_eq!(
            Cli::try_parse_from(["localton", "bootstrap", "--block-time", "0"])
                .unwrap_err()
                .kind(),
            clap::error::ErrorKind::ValueValidation
        );
    }

    #[test]
    fn bootstrap_accepts_election_time_in_seconds() {
        let cli = Cli::try_parse_from(["localton", "bootstrap", "--election-time", "240"]).unwrap();
        let Command::Bootstrap(args) = cli.command else {
            panic!("expected bootstrap command");
        };
        assert_eq!(args.election_time, Some(240));
        assert_eq!(
            Cli::try_parse_from(["localton", "bootstrap", "--election-time", "3"])
                .unwrap_err()
                .kind(),
            clap::error::ErrorKind::ValueValidation
        );
    }

    #[test]
    fn lifecycle_command_is_explicit_without_legacy_aliases() {
        assert_eq!(
            Cli::try_parse_from(["localton"]).unwrap_err().kind(),
            clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );

        for legacy_command in ["run", "agent"] {
            assert_eq!(
                Cli::try_parse_from(["localton", legacy_command])
                    .unwrap_err()
                    .kind(),
                clap::error::ErrorKind::InvalidSubcommand
            );
        }
    }
}
