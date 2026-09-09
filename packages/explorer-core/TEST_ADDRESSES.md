# Explorer test addresses

Short registry of live-chain accounts that are useful for manual Explorer checks

These addresses are not fixtures and their balances or recent history can change. Keep each entry focused on behavior that is expected to remain useful and update the last-checked date after verifying it

## Mainnet

| Address | What to verify | Last checked |
| --- | --- | --- |
| [`EQA9S5qRZTa0GcH5VZqDJ_KxtfVvGzYgZtdon7Rhg82zLi-N`](https://actonscan.com/address/EQA9S5qRZTa0GcH5VZqDJ_KxtfVvGzYgZtdon7Rhg82zLi-N?network=mainnet) | Fragment username NFT: Username and both Telegram aliases are shown as external links in the account summary | 2026-09-09 |
| [`EQCkQlDs452x_vspIKq_PTlUl8B0S427RELr_62Dbro7g3om`](https://actonscan.com/address/EQCkQlDs452x_vspIKq_PTlUl8B0S427RELr_62Dbro7g3om?network=mainnet) | Fragment anonymous number NFT: Number is shown as a Telegram link in the account summary | 2026-09-09 |
| [`EQAAAFVppd1Pip_zOxJIezgVnOzy7Yn_Mob56acg4MhcCuwi`](https://actonscan.com/address/EQAAAFVppd1Pip_zOxJIezgVnOzy7Yn_Mob56acg4MhcCuwi?network=mainnet) | Standard NftCollection ABI fallback; get_collection_data decodes nextItemIndex = -1 for a non-sequential collection | 2026-09-09 |
| [`EQAABbDZyUG5vO-lhhpNPMzdHmv5Ub5NILsKgJj_51kZDAQ6`](https://actonscan.com/address/EQAABbDZyUG5vO-lhhpNPMzdHmv5Ub5NILsKgJj_51kZDAQ6?network=mainnet) | Standard NftCollection ABI fallback; get_nft_address_by_index(0) matches the item below, and get_nft_content combines its individual content with the collection URI | 2026-09-09 |
| [`EQDreUU_JJiOt-5cUX8Q3xFrtOFqQX-ESYXtVeFncgTq0MaJ`](https://actonscan.com/address/EQDreUU_JJiOt-5cUX8Q3xFrtOFqQX-ESYXtVeFncgTq0MaJ?network=mainnet) | Standard NftItem ABI fallback shows five TEP-62 messages; get_nft_data decodes initialization, index, collection, owner and content without assuming a storage layout | 2026-09-09 |
| [`EQAVUSshxwidTs6F1kckyH4OxkM9fl2aoZG-Lh3x5DiZ8s7C`](https://actonscan.com/address/EQAVUSshxwidTs6F1kckyH4OxkM9fl2aoZG-Lh3x5DiZ8s7C?network=mainnet) | STONCAT master: generic TEP-74 ABI fallback, successful get_jetton_data decoding and raw storage without an assumed layout | 2026-09-09 |
| [`EQBzU2fioouNYWoD3f064TwSM3C1pNnO47dW1VUhWGZOhwm8`](https://actonscan.com/address/EQBzU2fioouNYWoD3f064TwSM3C1pNnO47dW1VUhWGZOhwm8?network=mainnet) | STONCAT wallet: generic TEP-74 ABI fallback and successful get_wallet_data decoding with owner and master addresses | 2026-09-09 |
| [`UQA9Xxv6Ig-SvxM_j_Xde6XTOG3ftJfEYUFFLauvq5wyHMru`](https://actonscan.com/address/UQA9Xxv6Ig-SvxM_j_Xde6XTOG3ftJfEYUFFLauvq5wyHMru?network=mainnet) | Wallet V4 R1 Simulator: automatic wallet ID and seqno, signed external simple send with a 0.1 GRAM self-message and comment under Ignore CHKSIG; compute and action exit codes are 0 and the message remains editable after emulation | 2026-09-08 |
| [`UQAtPMPoGXJzm6zvqeRcK6IzgJa8RpISp0xpMPgOgj5ggKaV`](https://actonscan.com/address/UQAtPMPoGXJzm6zvqeRcK6IzgJa8RpISp0xpMPgOgj5ggKaV?network=mainnet) | Wallet V4 R2 Simulator: automatic wallet ID and seqno through the bits264 plugin-dictionary fallback, 0.1 GRAM self-message with comment, compute and action exit codes 0, lossless raw/builder switching and four-message limit | 2026-09-08 |
| [`EQDV9A9W0GpbnFhiI6hJkGUSNIqU7Nxx-rn5FVQsAc7ZkfZB`](https://actonscan.com/address/EQDV9A9W0GpbnFhiI6hJkGUSNIqU7Nxx-rn5FVQsAc7ZkfZB?network=mainnet) | Frozen account state, no Contract type row, Unfreezer link next to Tonscan | 2026-07-30 |
| [`EQCBMyAieemf3vF3umY0lCaQxLhwvbTFuL8eQxPYrpeZ8O4O`](https://actonscan.com/address/EQCBMyAieemf3vF3umY0lCaQxLhwvbTFuL8eQxPYrpeZ8O4O?network=mainnet) | Active account that is also suspended, suspended overview must not depend on account state | 2026-07-30 |
| [`Ef9mDsqzIg2i8fdw0Bb7UGafA3Gc1qX5IYjp6AOZwGlfvim2`](https://actonscan.com/address/Ef9mDsqzIg2i8fdw0Bb7UGafA3Gc1qX5IYjp6AOZwGlfvim2?network=mainnet) | Suspended account with a long resolved name, name ellipsis and spacing between QR and edit controls | 2026-07-30 |
| [`EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c`](https://actonscan.com/address/EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c?network=mainnet) | Zero Address, Uninit state, suspended overview, large mixed action history | 2026-07-30 |
| [`EQAlL9ItlyCN7VbZyDV3lLxoTcwPCl3zUT62xB9VAwJ_USDC`](https://actonscan.com/address/EQAlL9ItlyCN7VbZyDV3lLxoTcwPCl3zUT62xB9VAwJ_USDC?network=mainnet) | Mobile account header, address suffix preservation, QR, favorite, edit, and copy action layout | 2026-07-30 |
| [`EQDXgkYbrxDpRZD6PUZd0jwdjZmYYQd7l5YOE2UeXunLD8Wm`](https://actonscan.com/address/EQDXgkYbrxDpRZD6PUZd0jwdjZmYYQd7l5YOE2UeXunLD8Wm?network=mainnet) | Wallet with many Renew DNS actions, useful for checking that domains such as `bybit.ton` are rendered as NFT chips | 2026-07-31 |
| [`EQD5MEMHNCHR-zkATB3elGUvRfNiZAlK_luIj0matDI1xCIC`](https://actonscan.com/address/EQD5MEMHNCHR-zkATB3elGUvRfNiZAlK_luIj0matDI1xCIC?network=mainnet) | Wallet with a Change DNS action in transaction `e9bffd9d…aad9a0`, useful for checking that the affected domain is rendered as an NFT chip instead of the raw DNS record-category hash | 2026-09-08 |
| [`EQDoxOcDo0EkHBNVL6tFfH5K-BAWI9PSO44zlgWdOwgeqw7m`](https://actonscan.com/address/EQDoxOcDo0EkHBNVL6tFfH5K-BAWI9PSO44zlgWdOwgeqw7m?network=mainnet) | Wallet V5 with repeated 250-action bulk sends, useful for action pagination; Simulator: fetch wallet ID and seqno, build a 0.1 GRAM self-message with a comment, emulate with Ignore CHKSIG and verify compute/action exit code 0 | 2026-09-08 |
| [`EQDYzZmfsrGzhObKJUw4gzdeIxEai3jAFbiGKGwxvxHinaPP`](https://actonscan.com/address/EQDYzZmfsrGzhObKJUw4gzdeIxEai3jAFbiGKGwxvxHinaPP?network=mainnet) | Wallet with more than 1,000 NFTs and safety-filtered items in early batches, useful for checking uninterrupted incremental loading in the NFTs tab | 2026-07-31 |
| [`EQBazGjn2UgiOom47yy_fGdRoxU1tWNBNqWnkil5T9fP-xuK`](https://actonscan.com/address/EQBazGjn2UgiOom47yy_fGdRoxU1tWNBNqWnkil5T9fP-xuK?network=mainnet) | Contract with very large code, useful for checking that the account page and Code tab load without errors | 2026-08-02 |
| [`EQCeTFSYKmcPZIQ-0Hvi98bXvXygrxru56LzUllC-Jup2727`](https://actonscan.com/address/EQCeTFSYKmcPZIQ-0Hvi98bXvXygrxru56LzUllC-Jup2727?network=mainnet) | Multisig orders with expired entries, useful for checking the red Expired status and cross icon | 2026-07-31 |
| [`kQB1tqrLMLJZk0YnmU_1UKr-r90QBnUubm0So09b39LPk3rZ`](https://actonscan.com/address/kQB1tqrLMLJZk0YnmU_1UKr-r90QBnUubm0So09b39LPk3rZ?network=mainnet) | Multisig with 1,487 orders, useful for checking incremental loading while scrolling the Orders table | 2026-07-31 |
| [`EQDKHZ7e70CzqdvZCC83Z4WVR8POC_ZB0J1Y4zo88G-zCXmC`](https://actonscan.com/address/EQDKHZ7e70CzqdvZCC83Z4WVR8POC_ZB0J1Y4zo88G-zCXmC?network=mainnet) | Binance account with many jetton balances, useful for checking token metadata, balances, and incremental loading in the Tokens tab | 2026-08-02 |
| [`EQCxE6mUtQJKFnGfaROTKOt1lZbDiiX1kCixRv7Nw2Id_sDs`](https://actonscan.com/address/EQCxE6mUtQJKFnGfaROTKOt1lZbDiiX1kCixRv7Nw2Id_sDs?network=mainnet) | USD₮ jetton master with many holders and a URI-only `metadataUri` storage cell, useful for checking Holders loading and opening metadata in Cell Inspector | 2026-09-08 |
| [`EQAHpIxxJEMzoHfqHU_8c7G1qXtIgd9xCkaBF3QeTzLv3rJU`](https://actonscan.com/address/EQAHpIxxJEMzoHfqHU_8c7G1qXtIgd9xCkaBF3QeTzLv3rJU?network=mainnet) | Jetton master with on-chain TEP-64 `jettonContent`, useful for checking Cell Inspector decoding of name, symbol, decimals, description, and image | 2026-09-08 |
| [`EQDgR7jZy9D88IBC3dK4qEJjb9D8WmfCbL9v-4Fjsjzm7cpq`](https://actonscan.com/address/EQDgR7jZy9D88IBC3dK4qEJjb9D8WmfCbL9v-4Fjsjzm7cpq?network=mainnet) | Account with more than 400 NFTs, useful for checking uninterrupted incremental loading in the NFTs tab | 2026-08-02 |
| [`EQCA14o1-VWhS2efqoh_9M1b_A9DtKTuoqfmkn83AbJzwnPi`](https://actonscan.com/address/EQCA14o1-VWhS2efqoh_9M1b_A9DtKTuoqfmkn83AbJzwnPi?network=mainnet) | NFT collection with more than 800 items, useful for checking collection metadata and uninterrupted incremental item loading | 2026-08-02 |

## Mainnet blocks

| Block | What to verify | Last checked |
| --- | --- | --- |
| [`0:8000000000000000:86786720`](https://actonscan.com/block/0/8000000000000000/86786720?network=mainnet) | Block with 533 transactions, useful for checking the v2 fallback and uninterrupted incremental transaction loading | 2026-08-06 |

## Mainnet configuration blocks

| Block | What to verify | Last checked |
| --- | --- | --- |
| [`-1:8000000000000000:84965023`](https://actonscan.com/block/-1/8000000000000000/84965023?network=mainnet) | Masterchain block whose configuration contains ConfigParam 36 (`Next validator set`), useful for checking historical validator set rendering | 2026-08-10 |

## Mainnet ConfigParam 8 history

`ConfigParam 8` contains the network's global protocol version and a bitmask of enabled capabilities. Closed ranges below are inclusive; an open end means that the value is still active.

| From seqno | To seqno | Version | Capabilities |
| ---: | ---: | ---: | ---: |
| [1](https://actonscan.com/block/-1/8000000000000000/1?network=mainnet) | [2,908,198](https://actonscan.com/block/-1/8000000000000000/2908198?network=mainnet) | 0 | `0x002` |
| [2,908,199](https://actonscan.com/block/-1/8000000000000000/2908199?network=mainnet) | [2,908,414](https://actonscan.com/block/-1/8000000000000000/2908414?network=mainnet) | 1 | `0x00e` |
| [2,908,415](https://actonscan.com/block/-1/8000000000000000/2908415?network=mainnet) | [2,908,450](https://actonscan.com/block/-1/8000000000000000/2908450?network=mainnet) | 0 | `0x002` |
| [2,908,451](https://actonscan.com/block/-1/8000000000000000/2908451?network=mainnet) | [3,127,941](https://actonscan.com/block/-1/8000000000000000/3127941?network=mainnet) | 1 | `0x006` |
| [3,127,942](https://actonscan.com/block/-1/8000000000000000/3127942?network=mainnet) | [34,875,662](https://actonscan.com/block/-1/8000000000000000/34875662?network=mainnet) | 2 | `0x02e` |
| [34,875,663](https://actonscan.com/block/-1/8000000000000000/34875663?network=mainnet) | [35,865,386](https://actonscan.com/block/-1/8000000000000000/35865386?network=mainnet) | 4 | `0x02e` |
| [35,865,387](https://actonscan.com/block/-1/8000000000000000/35865387?network=mainnet) | [36,746,857](https://actonscan.com/block/-1/8000000000000000/36746857?network=mainnet) | 5 | `0x02e` |
| [36,746,858](https://actonscan.com/block/-1/8000000000000000/36746858?network=mainnet) | [37,375,728](https://actonscan.com/block/-1/8000000000000000/37375728?network=mainnet) | 6 | `0x02e` |
| [37,375,729](https://actonscan.com/block/-1/8000000000000000/37375729?network=mainnet) | [39,939,168](https://actonscan.com/block/-1/8000000000000000/39939168?network=mainnet) | 7 | `0x02e` |
| [39,939,169](https://actonscan.com/block/-1/8000000000000000/39939169?network=mainnet) | [44,891,368](https://actonscan.com/block/-1/8000000000000000/44891368?network=mainnet) | 8 | `0x1ee` |
| [44,891,369](https://actonscan.com/block/-1/8000000000000000/44891369?network=mainnet) | [47,557,456](https://actonscan.com/block/-1/8000000000000000/47557456?network=mainnet) | 9 | `0x1ee` |
| [47,557,457](https://actonscan.com/block/-1/8000000000000000/47557457?network=mainnet) | [49,524,025](https://actonscan.com/block/-1/8000000000000000/49524025?network=mainnet) | 10 | `0x1ee` |
| [49,524,026](https://actonscan.com/block/-1/8000000000000000/49524026?network=mainnet) | [53,939,516](https://actonscan.com/block/-1/8000000000000000/53939516?network=mainnet) | 11 | `0x1ee` |
| [53,939,517](https://actonscan.com/block/-1/8000000000000000/53939517?network=mainnet) | [59,015,495](https://actonscan.com/block/-1/8000000000000000/59015495?network=mainnet) | 12 | `0x1ee` |
| [59,015,496](https://actonscan.com/block/-1/8000000000000000/59015496?network=mainnet) | [71,304,030](https://actonscan.com/block/-1/8000000000000000/71304030?network=mainnet) | 13 | `0x1ee` |
| [71,304,031](https://actonscan.com/block/-1/8000000000000000/71304031?network=mainnet) | [81,421,435](https://actonscan.com/block/-1/8000000000000000/81421435?network=mainnet) | 14 | `0x3ee` |
| [81,421,436](https://actonscan.com/block/-1/8000000000000000/81421436?network=mainnet) | — | 15 | `0x3ee` |

Capability masks used in this history:

| Mask | Enabled capabilities |
| ---: | --- |
| `0x002` | `capCreateStatsEnabled` |
| `0x006` | `capCreateStatsEnabled`, `capBounceMsgBody` |
| `0x00e` | `capCreateStatsEnabled`, `capBounceMsgBody`, `capReportVersion` |
| `0x02e` | `capCreateStatsEnabled`, `capBounceMsgBody`, `capReportVersion`, `capShortDequeue` |
| `0x1ee` | `capCreateStatsEnabled`, `capBounceMsgBody`, `capReportVersion`, `capShortDequeue`, `capStoreOutMsgQueueSize`, `capMsgMetadata`, `capDeferMessages` |
| `0x3ee` | `capCreateStatsEnabled`, `capBounceMsgBody`, `capReportVersion`, `capShortDequeue`, `capStoreOutMsgQueueSize`, `capMsgMetadata`, `capDeferMessages`, `capFullCollatedData` |

The early `v0 -> v1 -> v0 -> v1` rollback means this history cannot be found safely with a binary search that assumes monotonic values. Version 3 was not observed in mainnet. Equal capability masks across multiple versions do not mean equal protocol behavior.

## Mainnet transactions

| Transaction | What to verify | Last checked |
| --- | --- | --- |
| [`37bf9a98ff1355d179dd7b8f95a45aeb6566cbeccb2d97b2585d4e5924ad3302`](https://actonscan.com/tx/37bf9a98ff1355d179dd7b8f95a45aeb6566cbeccb2d97b2585d4e5924ad3302?network=mainnet) | STONCAT transfer trace: standard Jetton ABI names in the tree and parsed transfer/internal-transfer bodies, including referenced forward payload; no storage loading for an interface-only ABI; after opening State Changes, Details shows wrapped send modes without reserving empty horizontal space | 2026-09-09 |
| [`8199e63661cc9db0764d81403d17679e154aa24a725b632f6936402a2442319e`](https://actonscan.com/tx/8199e63661cc9db0764d81403d17679e154aa24a725b632f6936402a2442319e?network=mainnet) | Large trace with 240 actions and 241 transactions, useful for checking that Actions and Value Flow start with 10 full rows plus one faded preview row, remain responsive, and reveal the full table and Value Flow total on demand | 2026-08-10 |
| [`2a88c08d3411efa6dc08acfdce87389c98ea03a52f696a00882f71cba3682fde`](https://actonscan.com/tx/2a88c08d3411efa6dc08acfdce87389c98ea03a52f696a00882f71cba3682fde?network=mainnet) | `DnsDomainItemChangeDnsRecord` with a referenced value slice containing a `dns_smc_address`, useful for checking Slice handoff to Cell Inspector and TON DNS domain parsing | 2026-09-08 |
| [`1d0aac90eae0914432710b859d6d4c6bb42aa36141f29fe00165cc240cc17a87`](https://actonscan.com/tx/1d0aac90eae0914432710b859d6d4c6bb42aa36141f29fe00165cc240cc17a87?network=mainnet) | Opening Actions in the transaction details reproduces `Cannot parse stack:` | 2026-08-02 |

## Testnet

| Address | What to verify | Last checked |
| --- | --- | --- |
| [`kQAgO7g7m2763OuP-AaTVOZVhEjg5zYyCKDF660QzJp71KLB`](https://actonscan.com/address/kQAgO7g7m2763OuP-AaTVOZVhEjg5zYyCKDF660QzJp71KLB?network=testnet) | Alternating incoming and outgoing transfers around `0.01 GRAM`, useful for the small-transfer spam filter and for confirming outgoing transfers remain visible | 2026-07-30 |
| [`kQB6XGzpO7rglhK1tR9A4l2QQu6yaYE6ALUp1vAOHMaGAfGD`](https://actonscan.com/address/kQB6XGzpO7rglhK1tR9A4l2QQu6yaYE6ALUp1vAOHMaGAfGD?network=testnet) | Repeated one-nano outgoing self-transfers, useful for confirming the spam filter never hides outgoing actions | 2026-07-30 |

## Full localnet

These accounts belong to an isolated local test environment, not a public network. Recreate the imported account from the fixture in `crates/acton-studio/src/local_process/imports/tests.rs` when the environment is no longer available

| Address | What to verify | Last checked |
| --- | --- | --- |
| `kQBFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRdNV` | After importing into a running full localnet, Explorer shows an active account with 1 GRAM and no transaction history; Contracts shows its imported name. Verified on two running nodes, with code and data matching the source and an unrelated account preserved | 2026-09-07 |

## Adding an entry

- Use a full user-friendly URL-safe address and include the network in the link
- Describe the observable behavior rather than the implementation that currently renders it
- Do not add an address until the scenario has been verified manually
- Update or remove entries when live-chain data no longer demonstrates the described behavior
