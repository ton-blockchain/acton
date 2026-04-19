# acton-wallet(1)

## Name

acton-wallet --- Create, import, inspect, and use development wallets

## Synopsis

`acton wallet` [_options_] _command_

## Description

Manage wallets used for Acton development workflows.

The wallet subsystem can create new wallets, import mnemonic-based wallets,
list available wallets, export mnemonics interactively, sign external message
bodies, request faucet funds, and remove stored wallet entries.

Wallets created by this workflow are intended for development and testing. The
default network context is testnet.

## Common Lifecycle

The wallet workflow is not limited to `wallet new`.

Typical follow-up commands are:

- `acton wallet import` to reuse an existing mnemonic
- `acton wallet list --balance` to inspect configured wallets and balances
- `acton wallet airdrop --net localnet` to fund a wallet from a localnet
  faucet
- `acton wallet sign` to sign an external message body without writing a whole
  script
- `acton wallet export-mnemonic` for interactive export
- `acton wallet remove -y` to remove a wallet non-interactively

Acton's testnet TonCenter client reads `TONCENTER_API_KEY` for balance-backed
flows such as `wallet list --balance` and the interactive post-airdrop balance
confirmation after `wallet new`. `wallet list --balance` also accepts
`--api-key`.

## Subcommands

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

{{#option "`--faucet-url` _url_" }}
Faucet URL for automatic testnet airdrop.
{{/option}}

{{#option "`--no-wait-airdrop`" }}
Do not wait for testnet funds to appear after a successful automatic airdrop.
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

{{#option "`--body`, `--message` _boc_" }}
External body BoC to sign in hex or base64.

If omitted, Acton reads from stdin when stdin is piped; otherwise it prompts
interactively.
{{/option}}

{{#option "`--json`" }}
Emit JSON output.
{{/option}}

{{/options}}

Without `--json`, Acton prints the signed external-body BoC in hex. With
`--json`, it prints a JSON object containing the signed hex body.

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

{{#option "`--no-wait-airdrop`" }}
Do not wait for testnet funds to appear after a successful testnet airdrop.
{{/option}}

{{#option "`--json`" }}
Emit JSON output.
{{/option}}

{{/options}}

With `--net localnet`, Acton uses the localnet faucet instead of the
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

## Resolution Rules

- wallet definitions are merged from `global.wallets.toml` first and then
  local `wallets.toml`
- when both files define the same wallet name, the local entry wins
- commands that need a wallet auto-select it only when exactly one merged
  wallet is available
- otherwise Acton prompts on a TTY; in non-interactive contexts, provide the
  wallet name explicitly

## New And Import Workflow

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
After an interactive auto-airdrop succeeds, Acton briefly waits for the balance
to appear on testnet and lets you skip that wait by pressing `Enter`, unless
`--no-wait-airdrop` is passed.

## Listing, Signing, And Export

- `wallet list --balance` resolves balances through TonCenter and also respects
  the `TONCENTER_API_KEY` environment variable; the same environment fallback
  is used when `wallet new` waits for testnet funds after an interactive
  auto-airdrop
- `wallet sign` auto-detects hex and base64 input, preferring hex when a payload
  could be interpreted as both
- surrounding stdin whitespace is trimmed before decoding the message body
- `wallet export-mnemonic` is interactive-only and asks for confirmation before
  showing the mnemonic

## Airdrop Details

For `wallet airdrop`, the selected backend depends on `--net`.

- `testnet` requests a PoW challenge from the faucet, solves it locally, and
  submits a claim
- `localnet` sends a fixed faucet transfer through Localnet

After a successful interactive testnet `wallet airdrop`, Acton briefly waits
for the balance to appear and lets you skip that wait by pressing `Enter`,
unless `--no-wait-airdrop` is passed.

`--faucet-url` is only valid for `testnet`, must use `http` or `https`, and may
not include query parameters or fragments.

When `--json` is used, successful responses include the wallet address and may
also include PoW metadata such as difficulty, nonce, and solve time for the
testnet flow.

## Removal Notes

- if both local and global wallets share the same name, local config takes
  precedence when removing
- in non-interactive mode, `wallet remove` requires `-y` or `--yes`
- removing a wallet from config does not delete external files or environment
  variables referenced through other mnemonic sources

## Storage

Wallets can be stored:

- locally in project `wallets.toml`
- globally in `global.wallets.toml`
- with mnemonic values in plain text (`mnemonic`), secure keyring storage
  (`mnemonic-keyring`), environment variables (`mnemonic-env`), or external
  files (`mnemonic-file`) depending on configuration

For local wallets, keyring IDs usually include a project prefix. For global
wallets, the keyring ID usually matches the wallet name.

## Security

- secure keyring storage is recommended when available
- plain-text mnemonic storage is for development only
- do not commit wallet files with real secrets to version control

## Exit Status

- `0`: The selected wallet subcommand completed successfully.
- `1`: Input validation failed, a required wallet could not be resolved, a
  prompt could not run non-interactively, secure storage failed, or a network
  call such as balance lookup or faucet funding failed.

## Display Options

{{> options-display }}

## Project Options

{{> options-project-resolved }}

## Examples

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

5. Sign an external body read from stdin:

   ```bash
   cat body.boc.base64 | acton wallet sign deployer
   ```

## See Also

- `acton help localnet`
- [Wallet command guide](https://ton-blockchain.github.io/acton/docs/commands/wallet)
- [Wallet setup guide](https://ton-blockchain.github.io/acton/docs/setup-wallets)
