# Shared TON libraries

Libraries in this directory can be used independently of the Acton CLI. Each
library keeps its own Cargo workspace and package versions. Run Cargo commands
from the library directory or pass its manifest with `--manifest-path`.

| Library | Purpose |
| --- | --- |
| [tycho-types](tycho-types/) | Cells, BoC encoding, dictionaries, Merkle proofs, and TON models |

The root Acton workspace keeps its registry dependencies. Verifier is the first
application that uses the local `tycho-types` library:

```toml
[dependencies]
tycho-types = { path = "../../libs/tycho-types" }
```

Run the library tests from the repository root:

```sh
cargo test --manifest-path libs/tycho-types/Cargo.toml --workspace --all-features
```
