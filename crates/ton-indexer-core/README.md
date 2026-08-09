# TON indexer core

Reusable building blocks for full-fidelity TON indexing.

The crate separates block ingestion from product-specific storage and
classification:

1. a `BlockSource` produces a validated canonical `Batch`;
2. a `Sink` derives and commits an index from the complete batch;
3. a `CheckpointStore` advances only after the sink succeeds.

This gives the pipeline at-least-once delivery:

```text
source -> full Batch -> sink commit -> checkpoint save
```

Sinks must make commits idempotent, normally by using the complete masterchain
block ID as the batch key. Database schemas, ABI classification, reorg policy,
and historical backfill orchestration intentionally remain outside this crate.
