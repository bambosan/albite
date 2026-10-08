pub mod error;
pub mod hlsl_patch;
pub mod reflect;

use albite_core::{attrib::*, container::*, options::*, prepare_spirv::*, stage_io::*, ubo};
pub use error::DxilError;
use spirv_cross2::{
    Compiler as CrossCompiler, Module,
    compile::hlsl::{CompilerOptions, HlslShaderModel},
    targets::Hlsl,
};
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::path::PathBuf;

pub struct CompiledStage {
    pub bytes: Vec<u8>,
    pub hlsl_source: String,
}

pub fn compile(options: &Options, source: &str) -> Result<CompiledStage, DxilError> {
    let stage = options.shader_type;

    // stage 1: GLSL -> SPIR-V
    let spirv_src = compile_src_to_spirv(options, source, TranspileTarget::Hlsl)?;
    if stage.is_vertex() {
        validate_attrib_names(spirv_src.io.input_names()).map_err(DxilError::InvalidVertexInput)?;
    }

    // stage 2: SPIR-V -> HLSL
    let modified_words = ubo::construct_ubo(&spirv_src.spirv_words, 0, stage.is_fragment() as u32)
        .map_err(DxilError::Ubo)?;
    let module = Module::from_words(&modified_words.words);
    let mut cross = CrossCompiler::<Hlsl>::new(module)?;

    if stage.is_vertex() {
        for input in &spirv_src.io.inputs {
            if let Some(attr) = Attrib::from_str(&input.name) {
                cross.remap_vertex_attribute(input.location, attr.semantic())?;
            }
        }
    }

    let (uniforms, ubo_size, ssbos) = reflect::reflect_uniforms(&mut cross)?;

    let mut hopts = CompilerOptions::default();
    hopts.nonwritable_uav_texture_as_srv = true;

    let target_profile = if options.profile.eq_ignore_ascii_case("s_6_0") {
        hopts.shader_model = HlslShaderModel::ShaderModel6_0;
        match stage {
            ShaderType::Vertex => "vs_6_0",
            ShaderType::Fragment => "ps_6_0",
            ShaderType::Compute => "cs_6_0",
        }
    } else {
        hopts.shader_model = HlslShaderModel::ShaderModel6_5;
        match stage {
            ShaderType::Vertex => "vs_6_5",
            ShaderType::Fragment => "ps_6_5",
            ShaderType::Compute => "cs_6_5",
        }
    };

    let artifact = cross.compile(&hopts)?;
    let mut hlsl_source = artifact.to_string();

    for ssbo in &ssbos {
        hlsl_source = hlsl_patch::byte_address_to_structured(
            &hlsl_source,
            &ssbo.name,
            &ssbo.struct_name,
            &ssbo.layout,
        )
        .map_err(DxilError::HlslPatch)?;
    }

    // stage 3: HLSL -> DXIL
    let dxc = hassle_rs::Dxc::new(locate_library("dxcompiler"))?;
    let dxc_lib = dxc.create_library()?;
    let mut dxil_bytes = compile_hlsl(
        &dxc,
        &dxc_lib,
        &hlsl_source,
        &spirv_src.filename,
        target_profile,
    )?;
    dxil_bytes = validate_dxil(&dxc_lib, &dxil_bytes)?;

    let vertex_attrib = if stage.is_vertex() {
        remap_instance_attrib_id(&spirv_src.io.inputs)
    } else {
        vec![]
    };

    let bytes = write_container(&StageOutput {
        backend: Backend::Dxil,
        stage,
        io: &spirv_src.io,
        vertex_attrib_ids: &vertex_attrib,
        uniforms: &uniforms,
        ubo_size,
        local_size: None,
        code: &dxil_bytes,
    })
    .map_err(DxilError::InvalidVertexInput)?;

    Ok(CompiledStage { bytes, hlsl_source })
}

fn compile_hlsl(
    dxc: &hassle_rs::Dxc,
    dxc_library: &hassle_rs::DxcLibrary,
    hlsl_source: &str,
    filename: &str,
    target_profile: &str,
) -> Result<Vec<u8>, DxilError> {
    let compiler = dxc.create_compiler()?;
    let blob = dxc_library.create_blob_with_encoding_from_str(hlsl_source)?;

    match compiler.compile(&blob, filename, "main", target_profile, &["-O0"], None, &[]) {
        Ok(result) => Ok(result.get_result()?.to_vec::<u8>()),
        Err((result, hresult)) => {
            let message = result
                .get_error_buffer()
                .and_then(|error_blob| dxc_library.get_blob_as_string(&error_blob.into()))
                .unwrap_or_else(|_| "<no diagnostic text available>".to_string());
            Err(DxilError::DxcCompile { hresult, message })
        }
    }
}

fn validate_dxil(
    dxc_library: &hassle_rs::DxcLibrary,
    data: &[u8],
) -> Result<Vec<u8>, hassle_rs::HassleError> {
    let dxil = hassle_rs::Dxil::new(locate_library("dxil"))?;
    let validator = dxil.create_validator()?;
    let blob = dxc_library.create_blob_with_encoding(data)?;

    match validator.validate(blob.into()) {
        Ok(blob) => Ok(blob.to_vec::<u8>()),
        Err((result, _win32_error)) => {
            let error_blob = result.get_error_buffer()?;
            Err(hassle_rs::HassleError::ValidationError(
                dxc_library.get_blob_as_string(&error_blob.into())?,
            ))
        }
    }
}

fn locate_library(name: &str) -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    Some(exe_dir.join(format!("{DLL_PREFIX}{name}{DLL_SUFFIX}")))
}

fn remap_instance_attrib_id(inputs: &[IoVar]) -> Vec<u16> {
    let mut ordered: Vec<_> = inputs.iter().collect();
    ordered.sort_by_key(|input| input.location);
    ordered
        .into_iter()
        .filter_map(|input| {
            Attrib::from_str(&input.name).map(|attr| match attr {
                Attrib::Idata0 => Attrib::TexCoord8.id(),
                Attrib::Idata1 => Attrib::TexCoord7.id(),
                Attrib::Idata2 => Attrib::TexCoord6.id(),
                Attrib::Idata3 => Attrib::TexCoord5.id(),
                Attrib::Idata4 => Attrib::TexCoord4.id(),
                other => other.id(),
            })
        })
        .collect()
}
