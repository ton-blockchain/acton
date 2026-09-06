These BoCs come from Acton's Jetton and NFT templates in
`src/commands/new/templates`.

`NftCollection.patch` changes only `get_nft_content`: generated items return
complete on-chain TEP-64 metadata instead of concatenating URL fragments.
This lets NFT names and inline SVG images work without a metadata server.

To regenerate from the repository root:

```sh
cargo build --bin acton
python3 crates/acton-localnet/src/activity/contracts/generate.py
```
