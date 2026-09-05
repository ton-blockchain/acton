//! CLI arguments stay separate from service ownership and HTTP request handling.

use clap::{Args, Subcommand};
use std::path::PathBuf;

/// Command-line entry to the independent real-network runtime.
/// State is scoped to the project unless an explicit directory is selected.
#[derive(Args)]
pub struct LocalnetArgs {
    #[arg(
        long,
        global = true,
        help = "Localnet service data directory (default: <project>/.acton-localnet)"
    )]
    pub state_dir: Option<PathBuf>,

    #[arg(long, global = true, help = "Print machine-readable JSON")]
    pub json: bool,

    #[command(subcommand)]
    pub command: LocalnetCommand,
}

/// User-facing operations. Network deletion requires confirmation or `--yes`;
/// stopping a network always preserves its blockchain and snapshot volumes.
#[derive(Subcommand)]
pub enum LocalnetCommand {
    #[command(about = "Run the localnet HTTP service in the foreground")]
    Serve {
        network: Option<String>,

        #[arg(
            long,
            default_value_t = 0,
            help = "Control API port (0 chooses an available port)"
        )]
        port: u16,
    },

    #[command(about = "Start a real TON network and wait for its APIs")]
    Start {
        #[command(flatten)]
        options: CreateOptions,

        #[arg(long, help = "Leave a newly started service running in the background")]
        detach: bool,
    },

    #[command(about = "Create a stopped network definition")]
    Create {
        #[command(flatten)]
        options: CreateOptions,
    },

    #[command(about = "List local network definitions")]
    List,

    #[command(about = "Show network endpoints, nodes, and the latest operation")]
    Status { network: Option<String> },

    #[command(about = "Gracefully stop a network and keep its data")]
    Stop { network: Option<String> },

    #[command(about = "Delete the network containers, blockchain, and snapshots")]
    Delete {
        #[arg(help = "Network name or ID (select interactively when omitted)")]
        network: Option<String>,

        #[arg(long, help = "Confirm deletion of all network volumes")]
        yes: bool,
    },

    #[command(about = "Show the bounded tail of the network operation log")]
    Logs {
        network: Option<String>,

        #[arg(long, default_value_t = 100)]
        tail: usize,
    },

    #[command(about = "Manage full nodes and validator participation")]
    Node {
        network: Option<String>,

        #[command(subcommand)]
        command: NodeCommand,
    },

    #[command(about = "Manage cold network snapshots")]
    Snapshot {
        network: Option<String>,

        #[command(subcommand)]
        command: SnapshotCommand,
    },

    #[command(about = "Inspect or wait for an accepted operation")]
    Operation {
        id: String,

        #[arg(long)]
        network: Option<String>,

        #[arg(long)]
        wait: bool,
    },

    #[command(about = "Gracefully stop a network and its HTTP service")]
    Shutdown { network: Option<String> },
}

/// Genesis inputs apply only when creating a network. Reusing a name with changed
/// genesis options is rejected so `start` cannot silently ignore user settings.
#[derive(Args)]
pub struct CreateOptions {
    pub name: Option<String>,

    #[arg(
        long,
        help = "First of five consecutive host ports for Config, Admin, V2, V3, and observability"
    )]
    pub port_base: Option<u16>,

    #[arg(long)]
    pub block_time_ms: Option<u32>,

    #[arg(long)]
    pub election_time_seconds: Option<u32>,

    #[arg(
        long,
        help = "JSON file containing an array of hexadecimal ShardAccount BoCs"
    )]
    pub accounts_file: Option<PathBuf>,
}

/// Topology mutations follow the network's normal TON election lifecycle.
#[derive(Subcommand)]
pub enum NodeCommand {
    Add {
        name: String,

        #[arg(long)]
        validator: bool,
    },
    Remove {
        id: String,

        #[arg(long, help = "Allow deleting a validator still in an elected set")]
        force: bool,

        #[arg(long, required = true)]
        yes: bool,
    },
    EnterValidation {
        id: String,
    },
    LeaveValidation {
        id: String,
    },
}

/// Cold archives capture the blockchain state; restoration rebuilds indexing.
#[derive(Subcommand)]
pub enum SnapshotCommand {
    List,
    Create {
        name: Option<String>,
    },
    Restore {
        id: String,

        #[arg(long, required = true)]
        yes: bool,
    },
    Delete {
        id: String,

        #[arg(long, required = true)]
        yes: bool,
    },
}
