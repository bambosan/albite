use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrepareError {
    #[error("glsl -> spir-v compilation failed:\n{0}")]
    Spirv(#[from] shaderc::Error),

    #[error("malformed uniform declaration: {0}")]
    MalformedDeclaration(String),

    #[error("unsupported shader {direction} '{name}': {reason}")]
    UnsupportedIo {
        direction: String,
        name: String,
        reason: String,
    },

    #[error("spir-v reflection of shader inputs/outputs failed: {0}")]
    Reflect(#[from] spirv_cross2::SpirvCrossError),
}
