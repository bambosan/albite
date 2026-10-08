pub use thiserror::Error;

#[derive(Debug, Error)]
pub enum GlslError {
    #[error("{0}")]
    InvalidVertexInput(String),

    #[error(transparent)]
    Prepare(#[from] albite_core::error::PrepareError),

    #[error("spir-v -> glsl cross-compilation / reflection failed: {0}")]
    Cross(#[from] spirv_cross2::SpirvCrossError),
}
