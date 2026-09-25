# Wallet V5 accounts with installed plugins

This is a live-chain manual test case with a saved storage snapshot. The on-chain
state can change; use the snapshot below for deterministic decoder checks.

## Mainnet: Tonkeeper 2FA installed, signature authorization disabled

- Wallet: [`UQAh1U-WPuJthcG6fAN-lmoYfebMkJPoXK0MyBzdjzRQskEB`](https://actonscan.com/address/UQAh1U-WPuJthcG6fAN-lmoYfebMkJPoXK0MyBzdjzRQskEB?network=mainnet#plugins)
- Raw wallet: `0:21d54f963ee26d85c1ba7c037e966a187de6cc9093e85cad0cc81cdd8f3450b2`
- Plugin: [`EQAhPLqGHIVPVYg5-lbvo3B6tSFG3qtOz1MHIew3YdF1oP_w`](https://actonscan.com/address/EQAhPLqGHIVPVYg5-lbvo3B6tSFG3qtOz1MHIew3YdF1oP_w?network=mainnet)
- Raw plugin: `0:213cba861c854f558839fa56efa3707ab52146deab4ecf530721ec3761d175a0`
- Checked: `2026-09-25T09:47:11.935Z` using public Toncenter mainnet account states
- Both accounts were active
- Manually checked in the local Explorer on 2026-09-25: disabled signature auth,
  one installed plugin, and resolved type `Tonkeeper2fa`

### Registry provenance

The plugin is an existing `knownAddresses[0]` entry of
`tonkeeper_2fa.Tonkeeper2fa` in
[`crates/acton-abi-catalog/data/data-abis.json`](../../crates/acton-abi-catalog/data/data-abis.json).
Its registered display name is `Tonkeeper 2FA Extension`, and its ABI contract
name is `Tonkeeper2fa`.

The wallet was found by decoding the plugin's `Tonkeeper2faStorage.wallet` field
(the address immediately after `seqno:uint32`). The wallet's own extension
dictionary contains that same plugin address. This checks the association in both
directions without inferring an owner from the address.

The wallet is **not** a named entry in `packages/address-registry`. On 2026-09-25,
all 901 mainnet and 33 testnet entries of that registry were checked: the ten
mainnet and two testnet Wallet V5 R1 accounts found there all had empty extension
dictionaries. The useful nonempty case above therefore comes from the separate
bundled ABI catalog's known plugin address. No address-registry entries were added
or changed to create this provenance.

### Expected UI

- The wallet is recognized as Wallet V5 R1
- The top of the `Plugins` tab displays `Signature auth: Disabled`, reflecting the stored
  authorization bit
- The installed plugins table contains exactly one row
- The plugin row shows type `Tonkeeper2fa` and its address with navigation and copy
  actions; the type is resolved from the plugin's exact code hash
- The plugin address opens the extension account

This case has a nonempty extension dictionary, so disabling signature
authorization leaves the installed extension as the wallet's authorized caller.
The displayed flag describes the stored setting. In the separate edge case with
a false bit and an empty dictionary, a valid signed request can enable signature
authorization; that exception does not change the stored bit shown before the
request. The UI then appends `(auto-enables on signed request)` to `Disabled`.
That hint must be absent for this fixture because its plugin dictionary is nonempty.
The exception and the storage update are defined in the
[official Wallet V5 signed-request handler](https://github.com/ton-blockchain/wallet-contract-v5/blob/main/contracts/wallet_v5.fc#L164-L184).

### Wallet snapshot

| Field | Observed value |
| --- | --- |
| Code hash (hex) | `20834b7b72b112147e1b2fb457b84e74d1a30f04f737d4f62a668e9552d2b72f` |
| Account state hash (base64) | `Sd1roO2tX/FplrJ3kIVPDcc/PSkawk16dkLZ4dyva9w=` |
| Data hash (base64) | `ZqVFzuHizMjFePT2zXL6HvEdiuejgFgttgrxzDRJJT4=` |
| Last transaction hash (base64) | `jFMefsTgDNWSG9GezB+alTxvvhY96TtqMOVP4GCAnwU=` |
| Last transaction logical time | `79771211000005` |
| `isSignatureAllowed` | `false` |
| `seqno` | `15` |
| `walletId` | `2147483409` |
| Extension count | `1` |

Wallet data BoC (base64):

```text
te6ccgEBAgEAUAABUQAAAAe///+IoOG6AWmjA85+sWwmCJZ0efoExLTqhxgtqtXPry5bGkLgAQBDoAQnl1DDkKnqsQc/St30bg9WpCjb1WnZ6mDkPYbsOi60GA==
```

Expected extension addresses after decoding the snapshot with the wallet's
workchain:

```json
[
  "0:213cba861c854f558839fa56efa3707ab52146deab4ecf530721ec3761d175a0"
]
```

### Plugin snapshot

| Field | Observed value |
| --- | --- |
| Exact code hash (hex) | `c5ef19df22aee8b707bd7a181174e400a4225223c5ae40d8320f5ddd707d34a1` |
| ABI catalog id | `tonkeeper_2fa.Tonkeeper2fa` |
| Data hash (base64) | `WIqSa1nkFuk0AarF3Th+O07W0OirL2UTtpToWV0n42E=` |
| Stored `wallet` | `0:21d54f963ee26d85c1ba7c037e966a187de6cc9093e85cad0cc81cdd8f3450b2` |

Plugin data BoC (base64), retained to reproduce the wallet association:

```text
te6ccgEBAQEAcAAA2wAAABKABDqp8sfcTbC4N0+Ab9LNQw+82ZISfQuVoZkDm7HmihZW++Z4OyFBxPRPz1Cc73qlck96yVzKL2owAH0/pGSfnAg4boBaaMDzn6xbCYIlnR5+gTEtOqHGC2q1c+vLlsaQoAAAAAagJGx4
```

### Read-only recheck

Use Toncenter's account-state endpoints for the
[wallet](https://toncenter.com/api/v3/accountStates?address=UQAh1U-WPuJthcG6fAN-lmoYfebMkJPoXK0MyBzdjzRQskEB&include_boc=true)
and the
[plugin](https://toncenter.com/api/v3/accountStates?address=EQAhPLqGHIVPVYg5-lbvo3B6tSFG3qtOz1MHIew3YdF1oP_w&include_boc=true).
Recheck the code hashes, decode the current wallet data, and confirm that the
plugin's stored wallet still points back to this account before changing the
last-checked date. Public API rate limits may require spacing requests.

To decode the saved wallet snapshot from the repository root:

```sh
cd packages/explorer-core
bun -e 'import {parseWalletV5Storage} from "./src/components/walletV5"; console.log(parseWalletV5Storage("te6ccgEBAgEAUAABUQAAAAe///+IoOG6AWmjA85+sWwmCJZ0efoExLTqhxgtqtXPry5bGkLgAQBDoAQnl1DDkKnqsQc/St30bg9WpCjb1WnZ6mDkPYbsOi60GA==", "UQAh1U-WPuJthcG6fAN-lmoYfebMkJPoXK0MyBzdjzRQskEB"))'
```
