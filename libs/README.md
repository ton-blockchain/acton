# Shared TON libraries

Libraries in this directory can be used independently of the Acton CLI. Each
library keeps its own Cargo workspace and package versions. Run Cargo commands
from the library directory or pass its manifest with `--manifest-path`.

| Library | Purpose |
| --- | --- |
| [rston](rston/) | Cells, BoC encoding, dictionaries, Merkle proofs, and TON models |

The root Acton workspace keeps its registry dependencies. Verifier is the first
application that uses the local `rston` library:

```toml
[dependencies]
rston = { path = "../../libs/rston" }
```

Run the library tests from the repository root:

```sh
cargo test --manifest-path libs/rston/Cargo.toml --workspace --all-features
```
