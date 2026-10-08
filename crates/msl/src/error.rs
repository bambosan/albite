pub use thiserror::Error;

#[derive(Debug, Error)]
pub enum MslError {
    #[error("{0}")]
    InvalidVertexInput(String),

    #[error("uniform buffer packing failed: {0}")]
    Ubo(String),

    #[error(transparent)]
    Prepare(#[from] albite_core::error::PrepareError),

    #[error("spir-v -> msl cross-compilation / reflection failed: {0}")]
    Cross(#[from] spirv_cross2::SpirvCrossError),
}
