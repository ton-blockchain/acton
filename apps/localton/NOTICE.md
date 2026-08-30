# Third-party notice

`assets/gen-zerostate.fif` is adapted from the MyLocalTon genesis script and
TON smart-contract tooling. It was modified for Localton to generate
runtime-only keys and to omit unused highload-wallet bootstrap accounts.

- MyLocalTon: https://github.com/neodix42/MyLocalTon (GPL-3.0)
- TON: https://github.com/ton-blockchain/ton

Official TON executable archives are not distributed in this repository. The
Localton downloads the pinned upstream release directly and verifies its
SHA-256 digest.

The container build compiles TON Center Indexer from the pinned upstream
commit recorded in `Dockerfile`. Its C++ worker, Go API and Python action
classifier are distributed under the upstream MIT license:

- TON Center Indexer: https://github.com/toncenter/ton-indexer

The upstream source tree and compiler outputs are used only in multi-stage
build stages. The repository does not contain prebuilt TON Indexer binaries or
binary archives.
