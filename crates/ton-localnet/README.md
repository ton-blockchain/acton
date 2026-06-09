# ton-localnet API Notes

## TonCenter v3 response format

TonCenter v3 endpoints in local node return direct JSON payloads (no `ok/result/@extra` wrapper).

Errors are returned with HTTP status and request-error payload:

```json
{
  "code": 400,
  "error": "..."
}
```

## `/acton_setShardAccount`

`/acton_setShardAccount` is a local control endpoint for replacing one account
state with a serialized `ShardAccount`.

Request body:

```json
{
  "address": "<ADDR>",
  "shard_account": "<BASE64_BOC>"
}
```

`shard_account` must be a base64-encoded `ShardAccount` BOC string.

## `/acton_sendInternalMessage`

`/acton_sendInternalMessage` is a local control endpoint for injecting a raw
internal message into the local node. It accepts the same BOC payload shape as
TonCenter message endpoints:

```json
{
  "boc": "<BASE64_BOC>"
}
```

The BOC must decode to `MsgInfo::Int`. TonCenter-compatible endpoints such as
`/api/v2/sendBoc` and `/api/v3/message` accept external-in messages only.

## `/api/v3/addressInformation`

`/api/v3/addressInformation` is implemented as a v3-compatible view over
account state retrieval, based on the existing `/api/v2/getAddressInformation`
flow.

Differences vs `/api/v2/getAddressInformation`:

- Query params:
  - v2: `address`, optional `seqno`
  - v3: `address`, optional `use_v2` (defaults to `true` per OpenAPI)
- Payload shape:
  - v2 returns `raw.fullAccountState` (extended structure)
  - v3 returns `V2AddressInformation` fields:
    `balance`, `code`, `data`, `frozen_hash`,
    `last_transaction_hash`, `last_transaction_lt`, `status`

For non-existing accounts, `status` is returned as `uninitialized` (v2-compatible).

`use_v2` query parameter is accepted for compatibility with TonCenter v3 schema.

## `/api/v3/accountStates`

`/api/v3/accountStates` is implemented with TonCenter v3-compatible repeated
`address` query params and optional `include_boc` flag.

The response includes:

- `accounts` in `AccountStateFull` shape
- `accounts[].status` using TonCenter v3-style values such as `active`,
  `uninit`, `frozen`, `nonexist`
- optional fields such as `code_boc`, `data_boc`, `code_hash`, `data_hash`,
  and `frozen_hash` omitted when unavailable; `code_boc` / `data_boc` are also
  omitted when `include_boc=false`
- `address_book` rows for requested addresses with `user_friendly`, `domain`,
  and detected `interfaces`
- `metadata` entries for detected token contracts, with `token_info[].type`
  including:
  - `jetton_wallets`
  - `jetton_masters`
  - `nft_items`
  - `nft_collections`

## `/api/v3/nft/items`

`/api/v3/nft/items` is supported with TonCenter v3-compatible request filters:

- `address`
- `owner_address`
- `collection_address`
- `index` (requires `collection_address`)
- `sort_by_last_transaction_lt`
- `limit`, `offset`

The response includes:

- `nft_items` list
- `address_book` (empty object in local node)
- `metadata` with `token_info` for:
  - NFT item addresses (`type: "nft_items"`, includes `nft_index`, optional `name/description/image/symbol`, `extra`)
  - collection addresses (`type: "nft_collections"`)

## `/api/streaming/v2/sse` and `/api/streaming/v2/ws`

The local node exposes a TonCenter Streaming API v2-compatible subset for
real-time local tooling. Use the official TON Center Streaming API
documentation for protocol details:
https://docs.ton.org/applications/api/toncenter/streaming/overview

- `POST /api/streaming/v2/sse` opens an SSE subscription and returns
  `text/event-stream`.
- `GET /api/streaming/v2/ws` opens a WebSocket connection.

Localnet emits transaction, account-state, jetton-state, and trace updates.
Action extraction is not implemented yet.

## `/api/emulate/v1/emulateTrace`

`/api/emulate/v1/emulateTrace` is implemented to mimic `ton-emulate-go` contract.

- Success response is returned as a direct JSON payload (without `ok/result/@extra`) with
  top-level fields such as `mc_block_seqno`, `trace`, `transactions`, optional `actions`,
  optional `code_cells`/`data_cells`, `rand_seed`, and `is_incomplete`.
- For `include_code_data=true`, `code_cells` / `data_cells` are filled with collected BOCs
  keyed by base64 hash.
- Validation/runtime errors are returned with HTTP status `400`/`500` and body:

```json
{
  "error": "..."
}
```

- `include_address_book` and `include_metadata` are currently unavailable in local node and
  produce HTTP `400` (`invalid request: address book and metadata are not available`).
