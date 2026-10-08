pub mod error;
pub mod reflect;

use albite_core::{attrib::*, container::*, options::*, prepare_spirv::*, ubo};
pub use error::MslError;
use spirv_cross2::{
    Compiler as CrossCompiler, Module,
    compile::msl::{CompilerOptions, MetalPlatform},
    spirv::ExecutionModel,
    targets::Msl,
};

pub struct CompiledStage {
    pub bytes: Vec<u8>,
    pub msl_source: String,
}

pub fn compile(options: &Options, source: &str) -> Result<CompiledStage, MslError> {
    let stage = options.shader_type;

    // stage 1: GLSL -> SPIR-V
    let spirv_src = compile_src_to_spirv(options, source, TranspileTarget::Msl)?;
    if stage.is_vertex() {
        validate_attrib_names(spirv_src.io.input_names()).map_err(MslError::InvalidVertexInput)?;
    }

    // stage 2: SPIR-V -> MSL
    let modified_words = ubo::construct_ubo(&spirv_src.spirv_words, 0, 0).map_err(MslError::Ubo)?;
    let module = Module::from_words(&modified_words.words);
    let mut cross = CrossCompiler::<Msl>::new(module)?;

    cross.rename_entry_point(
        "main",
        "xlatMtlMain",
        match stage {
            ShaderType::Vertex => ExecutionModel::Vertex,
            ShaderType::Fragment => ExecutionModel::Fragment,
            ShaderType::Compute => ExecutionModel::GLCompute,
        },
    )?;

    let local_size = if stage.is_compute() {
        reflect::extract_local_size(&cross)
    } else {
        None
    };

    let (uniforms, ubo_size) = reflect::reflect_uniforms(&mut cross)?;

    let mut mopts = CompilerOptions::default();
    mopts.platform = MetalPlatform::iOS;
    mopts.enable_decoration_binding = true;

    let artifact = cross.compile(&mopts)?;
    let msl_source = artifact.to_string();

    let vertex_attrib = if stage.is_vertex() {
        attrib_ids_from_names(spirv_src.io.input_names())
    } else {
        vec![]
    };

    let bytes = write_container(&StageOutput {
        backend: Backend::Msl,
        stage,
        io: &spirv_src.io,
        vertex_attrib_ids: &vertex_attrib,
        uniforms: &uniforms,
        ubo_size,
        local_size,
        code: msl_source.as_bytes(),
    })
    .map_err(MslError::InvalidVertexInput)?;

    Ok(CompiledStage { bytes, msl_source })
}
