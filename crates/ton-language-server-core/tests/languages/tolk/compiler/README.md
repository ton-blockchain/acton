# Tolk compiler corpus

The `fixtures/` directory is a vendored copy of `tolk-tester/tests` from
`ton-blockchain/ton` commit `83fe78b06c9e66e1069e5f58bb2c2e78018dd13c`.

The fixtures are intentionally not synchronized automatically. Update them as a reviewed change,
then run the `tolk_compiler_corpus` test and account for every new semantic difference explicitly.

The upstream fixtures are distributed under the GNU Lesser General Public License v2.1. See
`LICENSE.LGPL` in this directory.
