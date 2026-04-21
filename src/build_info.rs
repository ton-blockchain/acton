pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GIT_HASH: &str = env!("GIT_HASH");
pub const BUILD_DATE: &str = env!("BUILD_DATE");
pub const TARGET_TRIPLE: &str = env!("TARGET_TRIPLE");
pub const BUILD_PROFILE: &str = env!("BUILD_PROFILE");
pub const RELEASE_CHANNEL: &str = env!("ACTON_RELEASE_CHANNEL");
pub const SHORT_VERSION: &str = env!("ACTON_SHORT_VERSION");
pub const LONG_VERSION: &str = env!("ACTON_LONG_VERSION");

#[must_use]
pub const fn user_agent() -> &'static str {
    concat!("acton/", env!("CARGO_PKG_VERSION"))
}

#[must_use]
pub fn is_trunk_build() -> bool {
    env!("ACTON_IS_TRUNK_BUILD") == "1"
}
