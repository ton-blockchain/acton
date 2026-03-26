# acton-verify(1)

## NAME

acton-verify --- Verify contract source code on the TON verifier service

## SYNOPSIS

`acton verify` [_options_] [_contract-id_]

## DESCRIPTION

Verify that a deployed contract address matches the local source code for a
contract from your project.

The verification flow compiles local sources, prepares data for the verifier
backend, collects the required signatures, and optionally submits the final
verification transaction to the blockchain.

## OPTIONS

### Verify Options

{{#options}}

{{#option "_contract-id_" }}
Contract ID to verify.

If omitted, Acton prompts when the project contains multiple contracts.
{{/option}}

{{#option "`--address` _address_" }}
Deployed contract address to verify.

If omitted, Acton prompts for it.
{{/option}}

{{#option "`--wallet` _wallet_" }}
Wallet to use for the verification transaction.
{{/option}}

{{#option "`--compiler-version` _version_" }}
Tolk compiler version to request on the verifier side.
{{/option}}

{{#option "`--dry-run`" }}
Run verification without submitting the final blockchain transaction.
{{/option}}

{{#option "`--api-key` _key_" }}
TonCenter API key for blockchain queries.
{{/option}}

{{/options}}

### Network Options

{{#options}}

{{#option "`--net` _network_" }}
Network to verify against.

Defaults to `testnet`.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-resolved }}

## PROCESS

Verification usually consists of:

1. compiling the local contract
2. calculating the resulting code hash
3. sending sources to a verifier backend
4. collecting the required signatures
5. optionally sending the final verification transaction

## REQUIREMENTS AND LIMITATIONS

- only `.tolk` sources can be verified
- precompiled `.boc` contracts cannot be verified
- `localnet` and `custom:<name>` are not supported by verifier backends
- verification requires a funded wallet when not using `--dry-run`
- if a contract with the same code hash is already verified, the backend may
  skip the final transaction

## EXAMPLES

1. Verify on testnet:

   ```bash
   acton verify counter --address EQDt7LL...
   ```

2. Verify on mainnet:

   ```bash
   acton verify counter --address UQDt7LL... --net mainnet
   ```

3. Use a specific wallet:

   ```bash
   acton verify counter --address EQDt7LL... --wallet deployer
   ```

4. Test the flow without sending the final transaction:

   ```bash
   acton verify counter --address EQDt7LL... --dry-run
   ```

## SEE ALSO

- [Contract verification guide](https://ton-blockchain.github.io/acton/docs/contract-verification)
- [Wallet setup guide](https://ton-blockchain.github.io/acton/docs/setup-wallets)
