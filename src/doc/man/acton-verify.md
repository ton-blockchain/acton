# acton-verify(1)

## Name

acton-verify --- Verify contract source code on the TON verifier service

## Synopsis

`acton verify` [_options_] [_contract-name_]

## Description

Verify local contract source code with the TON verifier on TON testnet.

The command compiles the local sources, requests a verification ticket, sends
the required testnet payment, and uploads the source bundle. An optional
deployed contract address can be used to check the compiled code hash before
payment.

## Options

### Verify Options

{{#options}}

{{#option "_contract-name_" }}
Contract name to verify.

If omitted, Acton prompts when the project contains multiple contracts.
{{/option}}

{{#option "`--address` _address_" }}
Deployed contract address to verify.

If omitted, Acton verifies the compiled code hash without a separate deployed
address check.
{{/option}}

{{#option "`--wallet` _wallet_" }}
Testnet wallet to use for the verification payment.

If omitted, Acton auto-selects the only configured wallet or prompts when
multiple wallets are available.
{{/option}}

{{#option "`--tonconnect`" }}
Use TON Connect wallet approval for the verification payment.

Acton prints a native TON Connect QR code and a `tc://` link.

Conflicts with `--wallet`.
{{/option}}

{{#option "`--compiler-version` _version_" }}
Tolk compiler version to request on the verifier side.

Currently defaults to `1.4.2`.
{{/option}}

{{#option "`--dry-run`" }}
Prepare verification without sending the payment or uploading sources.
{{/option}}

{{#option "`--payment-tx-hash` _payment-tx-hash_" }}
Reuse a finalized testnet payment transaction.

The transaction must contain the code hash from the current verification.
{{/option}}

{{/options}}

## TON Center API Keys

Testnet requests read `TONCENTER_TESTNET_API_KEY`.

Acton loads `.env` automatically, so the simplest setup during project work is
usually to keep these keys there and use shell environment variables only for
one-off overrides or CI.

### Display Options

{{> options-display }}

### Project Options

{{> options-project-resolved }}

## Process

1. Compile the local contract and compute its code hash.
2. Request a ticket from `/api/v1/take_ticket`.
3. Stop successfully if the code hash is already verified.
4. If `--address` is set, compare its deployed code hash with the compiled code.
5. Get wallet approval for the returned testnet amount and address.
6. Send the payment with the returned code-hash comment.
7. Wait for the finalized recipient transaction.
8. Upload the sources and recipient transaction hash to `/api/v1/verify`.

## Prerequisites

- a `.tolk` contract source in the current project
- testnet funds when `--dry-run` is not used
- TON verifier availability
- a configured wallet or TON Connect wallet, funded when not using `--dry-run`
- reproducible compiler settings that match the deployed contract

## Contract And Wallet Selection

- if `_contract-name_` is omitted and exactly one contract is configured, Acton
  selects it automatically
- if multiple contracts are configured, Acton prompts for the contract
- if `--wallet` is omitted and exactly one wallet is configured, Acton selects
  it automatically
- if multiple wallets are configured, Acton prompts for the wallet
- if `--tonconnect` is used, Acton skips local wallet selection and uses the
  wallet selected in the TON Connect page

## Requirements And Limitations

- only `.tolk` sources can be verified
- precompiled `.boc` contracts cannot be verified
- verification always uses TON testnet
- each verification payment contains the code hash in its comment
- one payment transaction can authorize only one verification attempt
- verification requires a funded local or TON Connect wallet when not using
  `--dry-run`
- if a contract with the same code hash is already verified, the backend can
  skip the final transaction

## Cost And Backend Notes

- the ticket defines the minimum testnet payment amount
- if the verifier backend reports that the contract is already verified, Acton
  exits successfully without sending another transaction
- on successful verification, Acton prints a verifier link for the contract

## Environment Overrides

The verification flow also supports backend/debug environment overrides:

- `ACTON_VERIFY_BACKEND` overrides the TON verifier backend

Backend override values are trimmed and normalized, including removal of a
trailing `/`.

Example: use a local verifier backend:

```acton-cli
ACTON_VERIFY_BACKEND=http://127.0.0.1:8080 \
acton verify Counter --dry-run
```

## Dry Run

`--dry-run` requests a ticket and prepares the source request. It
does not send a payment or upload sources.

## TON Connect

Use `--tonconnect` to approve the verification payment through a TON
Connect wallet instead of a wallet configured in `wallets.toml`:

```bash
acton verify Counter --tonconnect
```

Acton prints a native TON Connect QR code and a `tc://` link.

## Retries And Failure Hints

- source upload is attempted up to 8 times total for transient transport
  failures and explicitly retryable verifier errors
- retry backoff grows from 1 second to 7 seconds between attempts
- backend error responses are printed with the response body when available
- only an explicitly retryable source storage failure permits reuse of the
  same payment
- other results after the payment claim consume the payment, including generic
  internal failures

## Exit Status

- `0`: Verification completed successfully, including successful dry runs and
  flows where the backend decides that no final transaction is needed.
- `1`: Compilation failed, the verifier rejected the request, wallet resolution
  failed, or the payment transaction failed.

## Examples

1. Verify with a configured testnet wallet:

   ```bash
   acton verify Counter --wallet deployer
   ```

2. Check a deployed contract address before payment:

   ```bash
   acton verify Counter --address EQDt7LL...
   ```

3. Prepare verification without sending payment:

   ```bash
   acton verify Counter --address EQDt7LL... --dry-run
   ```

4. Verify with an explicit compiler version:

   ```bash
   acton verify Counter --address EQDt7LL... --compiler-version 1.2.0
   ```

5. Retry an upload with an existing finalized payment:

   ```bash
   acton verify Counter --payment-tx-hash '<transaction-hash>'
   ```

## See Also

- [Contract verification guide](https://ton-blockchain.github.io/acton/docs/verify)
- [Wallet management guide](https://ton-blockchain.github.io/acton/docs/wallets)
