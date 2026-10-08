pub mod error;
pub mod reflect;

use albite_core::{attrib::*, container::*, options::*, prepare_spirv::*};
pub use error::GlslError;
use spirv_cross2::{
    Compiler as CrossCompiler, Module,
    compile::glsl::{CompilerOptions, GlslVersion},
    spirv,
    targets::Glsl,
};

pub struct CompiledStage {
    pub bytes: Vec<u8>,
    pub glsl_source: String,
}

pub fn compile(options: &Options, source: &str) -> Result<CompiledStage, GlslError> {
    let stage = options.shader_type;

    // stage 1: GLSL -> SPIR-V
    let spirv_src = compile_src_to_spirv(options, source, TranspileTarget::Glsl)?;
    if stage.is_vertex() {
        validate_attrib_names(spirv_src.io.input_names()).map_err(GlslError::InvalidVertexInput)?;
    }

    // stage 2: SPIR-V -> RESOLVED GLSL
    let module = Module::from_words(&spirv_src.spirv_words);
    let mut cross = CrossCompiler::<Glsl>::new(module)?;

    if cross
        .declared_capabilities()?
        .contains(&spirv::Capability::SampledCubeArray)
    {
        cross.require_extension("GL_EXT_texture_cube_map_array")?;
    }

    let uniforms = reflect::reflect_uniforms(&mut cross)?;
    reflect::strip_decorations(&mut cross, stage)?;

    let mut gopts = CompilerOptions::default();
    gopts.version = GlslVersion::Glsl310Es;
    gopts.vulkan_semantics = false;

    let artifact = cross.compile(&gopts)?;
    let glsl_source = artifact.to_string();

    let vertex_attrib = if stage.is_vertex() {
        attrib_ids_from_names(spirv_src.io.input_names())
    } else {
        vec![]
    };

    let bytes = write_container(&StageOutput {
        backend: Backend::Glsl,
        stage,
        io: &spirv_src.io,
        vertex_attrib_ids: &vertex_attrib,
        uniforms: &uniforms,
        ubo_size: 0,
        local_size: None,
        code: glsl_source.as_bytes(),
    })
    .map_err(GlslError::InvalidVertexInput)?;

    Ok(CompiledStage { bytes, glsl_source })
}
