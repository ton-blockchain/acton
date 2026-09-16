// we're working in basechain, but theoretically, a jetton might even work in masterchain
const MY_WORKCHAIN = BASECHAIN

// it's a sharded jetton (a jetton wallet is in the same shard as owner's wallet)
// (example: if owner's address is `01010101xxx...xxx`, jetton wallet will be `01010101yyy...yyy`)
const SHARD_DEPTH = 8
