# acton-wallet(1)

## NAME

acton-wallet --- Create, import, inspect, and use development wallets

## SYNOPSIS

`acton wallet` [_options_] _command_

## DESCRIPTION

Manage wallets used for Acton development workflows.

The wallet subsystem can create new wallets, import mnemonic-based wallets,
list available wallets, export mnemonics interactively, sign external message
bodies, request faucet funds, and remove stored wallet entries.

Wallets created by this workflow are intended for development and testing. The
default network context is testnet.

## SUBCOMMANDS

### acton wallet new

Generate a new wallet.

#### Synopsis

`acton wallet new` [_options_]

#### Options

{{#options}}

{{#option "`--name` _name_" }}
Wallet name.

If omitted, Acton prompts for it.
{{/option}}

{{#option "`--version` _version_" }}
Wallet version.

If omitted, Acton prompts for it.
{{/option}}

{{#option "`--local`" }}
Save the wallet to local `wallets.toml`.
{{/option}}

{{#option "`--global`" }}
Save the wallet to global `global.wallets.toml`.
{{/option}}

{{#option "`--secure` [`true`|`false`]" }}
Use a secure native store for mnemonic storage when available.
{{/option}}

{{#option "`--airdrop`" }}
Request testnet TON after wallet creation.
{{/option}}

{{#option "`--json`" }}
Emit JSON output.
{{/option}}

{{/options}}

Wallet names are normalized to lowercase, spaces become `-`, and unsupported
characters are removed.

### acton wallet import

Import an existing mnemonic-based wallet.

#### Synopsis

`acton wallet import` [_options_] [_mnemonics_...]

#### Options

{{#options}}

{{#option "_mnemonics_..." }}
Mnemonic words for the wallet.

If omitted, Acton prompts interactively.
{{/option}}

{{#option "`--name` _name_" }}
Wallet name.
{{/option}}

{{#option "`--version` _version_" }}
Wallet version.
{{/option}}

{{#option "`--local`" }}
Save the wallet to local `wallets.toml`.
{{/option}}

{{#option "`--global`" }}
Save the wallet to global `global.wallets.toml`.
{{/option}}

{{#option "`--secure` [`true`|`false`]" }}
Use a secure native store for mnemonic storage when available.
{{/option}}

{{#option "`--json`" }}
Emit JSON output.
{{/option}}

{{/options}}

### acton wallet list

List configured wallets.

#### Options

{{#options}}

{{#option "`-b`, `--balance`" }}
Fetch and print wallet balances.
{{/option}}

{{#option "`--api-key` _key_" }}
TonCenter API key for balance lookups.
{{/option}}

{{#option "`--json`" }}
Emit JSON output.
{{/option}}

{{/options}}

### acton wallet export-mnemonic

Export a wallet mnemonic with interactive confirmation.

#### Synopsis

`acton wallet export-mnemonic` [_options_] [_name_]

This command is interactive-only and asks for confirmation before showing the
mnemonic.

### acton wallet sign

Sign an external message body with the selected wallet.

#### Synopsis

`acton wallet sign` [_options_] [_name_]

#### Options

{{#options}}

{{#option "_name_" }}
Wallet name.
{{/option}}

{{#option "`--body` _boc_" }}
External body BoC to sign in hex or base64.

If omitted, Acton reads from stdin or prompts interactively.
{{/option}}

{{#option "`--json`" }}
Emit JSON output.
{{/option}}

{{/options}}

Output is always a signed external-body BoC in hex.

### acton wallet airdrop

Request faucet funds for a wallet.

#### Synopsis

`acton wallet airdrop` [_options_] [_name_]

#### Options

{{#options}}

{{#option "_name_" }}
Wallet name.

If omitted, Acton auto-selects or prompts.
{{/option}}

{{#option "`--net` _network_" }}
Airdrop backend.

Possible values: `testnet`, `localnet`
{{/option}}

{{#option "`--faucet-url` _url_" }}
Custom faucet URL for the testnet backend.
{{/option}}

{{#option "`--json`" }}
Emit JSON output.
{{/option}}

{{/options}}

With `--net localnet`, Acton uses the local LiteNode faucet instead of the
testnet PoW flow.

### acton wallet remove

Remove a wallet from configuration.

#### Synopsis

`acton wallet remove` [_options_] [_name_]

#### Options

{{#options}}

{{#option "_name_" }}
Wallet name.
{{/option}}

{{#option "`-y`, `--yes`" }}
Skip the confirmation prompt.
{{/option}}

{{#option "`--json`" }}
Emit JSON output.
{{/option}}

{{/options}}

If the wallet uses keyring storage, Acton also removes the mnemonic from the
secure store.

## NEW AND IMPORT WORKFLOW

`wallet new` and `wallet import` share the same storage choices and most of the
same prompts.

- if `--name` is omitted, Acton prompts for the wallet name
- if `--version` is omitted, Acton prompts for the wallet version
- local wallets are stored in project `wallets.toml`
- global wallets are stored in `global.wallets.toml`
- common wallet versions include `v5r1`, `v4r2`, `v3r2`, and highload variants

When secure native storage is available, Acton prefers storing mnemonics in the
system keyring instead of plain-text config.

- if `--secure=true` is requested explicitly but secure storage is unavailable,
  the command fails
- if secure storage is unavailable and `--secure` is omitted, Acton falls back
  to plain-text config

When `wallet new` is used interactively without `--airdrop`, Acton can offer to
request testnet funds automatically after creating the wallet.

## LISTING, SIGNING, AND EXPORT

- `wallet list --balance` resolves balances through TonCenter and also respects
  the `TONCENTER_API_KEY` environment variable
- `wallet sign` auto-detects hex and base64 input, preferring hex when a payload
  could be interpreted as both
- `wallet export-mnemonic` is interactive-only and asks for confirmation before
  showing the mnemonic

## AIRDROP DETAILS

For `wallet airdrop`, the selected backend depends on `--net`.

- `testnet` requests a PoW challenge from the faucet, solves it locally, and
  submits a claim
- `localnet` sends a fixed faucet transfer through LiteNode

`--faucet-url` is only valid for `testnet`, must use `http` or `https`, and may
not include query parameters or fragments.

When `--json` is used, successful responses include the wallet address and may
also include PoW metadata such as difficulty, nonce, and solve time for the
testnet flow.

## REMOVAL NOTES

- if both local and global wallets share the same name, local config takes
  precedence when removing
- in non-interactive mode, `wallet remove` requires `-y` or `--yes`
- removing a wallet from config does not delete external files or environment
  variables referenced through other mnemonic sources

## STORAGE

Wallets can be stored:

- locally in project `wallets.toml`
- globally in `global.wallets.toml`
- with mnemonic values in plain text, secure keyring storage, environment
  variables, or external files depending on configuration

For local wallets, keyring IDs usually include a project prefix. For global
wallets, the keyring ID usually matches the wallet name.

## SECURITY

- secure keyring storage is recommended when available
- plain-text mnemonic storage is for development only
- do not commit wallet files with real secrets to version control

## DISPLAY OPTIONS

{{> options-display }}

## PROJECT OPTIONS

{{> options-project-resolved }}

## EXAMPLES

1. Create a local deployer wallet:

   ```bash
   acton wallet new --name deployer --version v5r1 --local
   ```

2. Import an existing mnemonic:

   ```bash
   acton wallet import --name deployer --local "word1 word2 ... word24"
   ```

3. Show balances:

   ```bash
   acton wallet list --balance
   ```

4. Request faucet funds:

   ```bash
   acton wallet airdrop deployer --net testnet
   acton wallet airdrop deployer --net localnet
   ```

## SEE ALSO

- `acton help litenode`
- [Wallet command guide](https://ton-blockchain.github.io/acton/docs/commands/wallet)
- [Wallet setup guide](https://ton-blockchain.github.io/acton/docs/setup-wallets)
