/// Module for public API to be used in scripts (interacting with real blockchain).
///
/// You write scripts to deploy and manage your contracts in a real blockchain.
/// Use methods `scripts.xxx` in combination with `net.xxx` in `scripts/some-file.tolk`.
///
/// Writing scripts that operate testnet/mainnet differs from writing tests.
/// In scripts, you connect your real wallet to send transactions.
/// In scripts, you are not able to set blockchain config or create an unlimited treasury.
/// In scripts, you wait for network responses, which takes time.
///
/// Any script can be run in either in emulation mode (without `--net`) or in a real blockchain (with `--net`).
///

import "_impl_emul"
import "config"
import "network"
import "testing"
import "../crypto"
import "../ffi"
import "../impl"
import "../testing/assert"
import "../types/transaction"

/// Root module for public API to be used in scripts along with `net.xxx` methods.
struct scripts

/// `ActonWallet` is a wallet created or registered by `acton wallet` CLI command.
/// Use `scripts.wallet("name")` to "connect" a wallet by name.
/// Available wallets are stored in `Acton.toml` either project-wise (local) or system-wise (global).
struct ActonWallet {
    address: address
}

/// Returns the key pair for a wallet configured in wallets.toml or global.wallets.toml.
///
/// The `privateKey` field is the 32-byte Ed25519 seed used by [crypto.sign]
/// and [crypto.rawSign]. This is available only for wallets opened by
/// [scripts.wallet] when the script was started with `--net`.
fun ActonWallet.toKeyPair(self): KeyPair {
    val keyPair = ffi.getWalletKeyPair(self.address);
    if (keyPair == null) {
        Assert.fail("Wallet keys are available only for configured wallets opened with `scripts.wallet()` in scripts started with `--net`");
    }

    val [privateKey, publicKey] = keyPair;
    return KeyPair { privateKey, publicKey };
}

/// Returns the wallet private key as a 32-byte Ed25519 seed.
fun ActonWallet.privateKey(self): int {
    return self.toKeyPair().privateKey;
}

/// Returns the wallet public key.
fun ActonWallet.publicKey(self): int {
    return self.toKeyPair().publicKey;
}

/// Returns the wallet id used by the opened wallet contract.
///
/// For wallet v5 this is the wallet_id stored in wallet data and used for
/// signed requests.
fun ActonWallet.walletId(self): int {
    val walletId = ffi.getWalletId(self.address);
    if (walletId == null) {
        Assert.fail("Wallet id is available only for configured wallets opened with `scripts.wallet()` in scripts started with `--net`");
    }

    return walletId;
}

/// Hashes and signs a cell with the wallet private key.
fun ActonWallet.sign(self, data: cell): slice {
    return crypto.sign(self.privateKey(), data);
}

/// Signs an already computed 256-bit hash with the wallet private key.
fun ActonWallet.rawSign(self, data: int): slice {
    return crypto.rawSign(self.privateKey(), data);
}

/// Creates a new wallet for use in scripts. Unlike `testing.treasury`, the wallet
/// can refer to a real wallet described in Acton.toml, and therefore can be used
/// in scripts launched with `--net` to execute transactions on the real blockchain.
///
/// Without `--net`, the wallet refers to an internal wallet, created in the
/// same way as in `testing.treasury`.
///
/// Note: in tests, use [testing.treasury].
///
/// Example:
/// ```tolk
/// /// Script that deploys a counter contract using a wallet to real blockchain.
/// fun main() {
///     val deployer = scripts.wallet("deployer");
///     val counter = Counter.fromStorage({
///         id: 8,
///         counter: 0,
///     });
///
///     val result = counter.deploy(deployer.address, ton("0.05"));
///     result.waitForFirstTransaction();
///
///     println("Deployed counter to {}", counter.address);
/// }
/// ```
fun scripts.wallet(name: string): ActonWallet {
    // for "broadcast to testnet"
    if (ffi.isBroadcasting()) {
        val addressCell = ffi.getWalletByName(name);
        if (addressCell == null) {
            ffi.failWalletNotFound(name, "");
            throw ASSERTION_FAILED;
        }
        val address = addressCell.beginParse().loadAddress();
        return ActonWallet { address };
    }

    // for "emulate" script without `--net`
    return ActonWallet { address: testing.treasury(name).address };
}

/// Returns current account state for address or null if no information available or
/// contract is not deployed.
///
/// The account state includes balance, storage information, and other account data.
/// See [TlbAccountInfo] type for more information.
///
/// Example:
/// ```tolk
/// val state = scripts.fetchAccountState(contract.address);
/// if (state != null) {
///     println("Balance: {:ton}", state.storage.balance.grams);
/// } else {
///     println("Account not found");
/// }
/// ```
fun scripts.fetchAccountState(addr: address): TlbAccountInfo? {
    return impl.getAccountState(addr);
}

/// Returns storage fee for address for specified amount of seconds.
///
/// Returns `null` if the contract with given address is not deployed.
///
/// Example:
/// ```tolk
/// val storageFee = scripts.fetchAccountStorageFee(contract.address, 86400); // 1 day
/// if (storageFee != null) {
///     println("Daily storage fee: {:ton}", storageFee);
/// }
/// ```
fun scripts.fetchAccountStorageFee(addr: address, seconds: int): coins? {
    return impl.getAccountStorageFee(addr, seconds);
}

/// Get the balance of an address.
///
/// Returns 0 if the address is not deployed.
///
/// Example:
/// ```tolk
/// val balance = scripts.fetchAccountBalance(address);
/// println("{:ton}", balance);
/// ```
fun scripts.fetchAccountBalance(of: address): coins {
    return impl.getAccountBalance(of);
}

/// Returns the shard account state for an address.
///
/// The returned shard account can represent an uninitialized account.
///
/// Example:
/// ```tolk
/// val shard = scripts.fetchShardAccount(addr);
/// if (shard != null) {
///     println("Last LT: {}", shard.lastTransLt);
/// }
/// ```
fun scripts.fetchShardAccount(addr: address): TlbShardAccount? {
    return impl.getShardAccount(addr);
}

/// Retrieves the current blockchain configuration.
///
/// Returns a map of all blockchain configuration parameters indexed from 0-90.
/// The configuration includes gas prices, storage costs, validator settings, and other
/// network parameters. Use specialized getter functions like [BlockchainConfigMap.getGasPrices],
/// [BlockchainConfigMap.getStoragePrices], or [BlockchainConfigMap.getGlobalVersion]
/// to access specific parameters.
///
/// For detailed information about blockchain configuration parameters, see the
/// [TON Blockchain Configuration Reference](https://docs.ton.org/foundations/config).
///
/// Returns: A [BlockchainConfigMap] containing all current blockchain configuration parameters.
///
/// Example:
/// ```tolk
/// val config = scripts.fetchConfig();
///
/// // Access specific configuration
/// val gasPrice = config.getGasPrices(BASECHAIN);
/// val storagePrices = config.getStoragePrices();
/// val globalVersion = config.getGlobalVersion();
///
/// println("Network version: {}", globalVersion.version);
/// ```
fun scripts.fetchConfig(): BlockchainConfigMap {
    return createMapFromLowLevelDict(ffi.getConfig());
}

/// Check if a contract with a specified address is deployed.
///
/// Returns `true` if the contract is deployed, `false` otherwise.
///
/// Example:
/// ```tolk
/// fun JettonWallet.getJettonBalance(self): coins {
///     if (!scripts.isContractDeployed(self.address)) {
///         return 0;
///     }
///
///     // if deployed, actually run get method
///     val result = net.runGetMethod<JettonWalletDataReply>(self.address, "get_wallet_data");
///     return result.jettonBalance;
/// }
/// ```
fun scripts.isContractDeployed(addr: address): bool {
    return ffi.isDeployed(addr.toCell());
}

fun ffi.getWalletByName(name: string): cell?
    asm "24 EXTCALL"

fun ffi.getWalletKeyPair(addr: address): [int, int]?
    asm "45 EXTCALL"

fun ffi.getWalletId(addr: address): int?
    asm "46 EXTCALL"

fun ffi.failWalletNotFound(walletName: string, location: string): bool
    asm "105 EXTCALL"
