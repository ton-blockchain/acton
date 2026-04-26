#[cfg(test)]
pub(crate) mod assertions;
#[cfg(test)]
pub(crate) mod compilation;
#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
pub(crate) mod localnet;
#[cfg(test)]
pub(crate) mod project;
#[cfg(test)]
pub(crate) mod snapshots;
#[cfg(test)]
pub(crate) mod tempdir;
#[cfg(test)]
pub(crate) mod toncenter;
#[cfg(test)]
pub(crate) mod verifier;

#[allow(unused_imports)]
pub(crate) use assertions::TestOutputExt;
