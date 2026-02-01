pub mod analysis;
#[allow(clippy::module_inception)]
pub mod backend;
pub mod diagnostics;
pub mod inlay_hints;
pub mod utils;

pub use analysis::AnalysisResult;
pub use backend::Backend;
