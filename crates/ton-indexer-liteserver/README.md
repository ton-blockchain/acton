# TON indexer LiteServer source

Canonical masterchain and shard traversal for `ton-indexer-core`, backed by a
direct TON LiteAPI connection.

The source validates block identities and BoCs before producing complete
indexing batches. Product-specific projections and persistence belong in
`Sink` implementations outside this crate.
