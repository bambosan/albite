use thiserror::Error;

#[derive(Debug, Error)]
pub enum DxilError {
    #[error("{0}")]
    InvalidVertexInput(String),

    #[error(transparent)]
    Prepare(#[from] albite_core::error::PrepareError),

    #[error("uniform buffer packing failed: {0}")]
    Ubo(String),

    #[error("spir-v -> hlsl cross-compilation / reflection failed: {0}")]
    Cross(#[from] spirv_cross2::SpirvCrossError),

    #[error(transparent)]
    Dxc(#[from] hassle_rs::HassleError),

    #[error("hlsl -> dxil compilation failed ({hresult}):\n{message}")]
    DxcCompile {
        hresult: hassle_rs::os::HRESULT,
        message: String,
    },

    #[error("storage buffer rewrite failed: {0}")]
    HlslPatch(String),
}
