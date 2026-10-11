use crate::{error::PrepareError, options::*, stage_io::*};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranspileTarget {
    Glsl,
    Hlsl,
    Msl,
}

impl TranspileTarget {
    fn defines(self) -> (&'static str, Option<&'static str>) {
        match self {
            Self::Glsl => ("TRANSPILE_TARGET__GLSL", Some("1")),
            Self::Hlsl => ("TRANSPILE_TARGET__HLSL", Some("1")),
            Self::Msl => ("TRANSPILE_TARGET__MSL", Some("1")),
        }
    }
}

pub struct PreparedSPIRV {
    pub filename: String,
    pub expanded: String,
    pub io: ShaderIo,
    pub spirv_words: Vec<u32>,
}

fn make_compile_options(
    options: &Options,
    target: TranspileTarget,
) -> Result<shaderc::CompileOptions<'static>, PrepareError> {
    let stage = options.shader_type;

    let mut copts = shaderc::CompileOptions::new()?;

    copts.set_source_language(shaderc::SourceLanguage::GLSL);

    copts.set_target_env(
        shaderc::TargetEnv::OpenGL,
        shaderc::EnvVersion::OpenGL4_5 as u32,
    );

    copts.set_optimization_level(match options.optimization {
        OptimizationLevel::O0 => shaderc::OptimizationLevel::Zero,
        OptimizationLevel::O1 => shaderc::OptimizationLevel::Size,
        OptimizationLevel::O2 => shaderc::OptimizationLevel::Performance,
    });

    copts.set_auto_bind_uniforms(true);
    copts.set_auto_map_locations(true);

    copts.set_generate_debug_info();

    if target == TranspileTarget::Hlsl {
        copts.set_hlsl_offsets(true);
    }

    for (name, value) in [target.defines()] {
        copts.add_macro_definition(name, value);
    }

    for (name, value) in [stage.defines()] {
        copts.add_macro_definition(name, value);
    }

    for (name, value) in &options.defines {
        copts.add_macro_definition(name, value.as_deref());
    }

    let search_dirs = include_search_dirs(options);

    copts.set_include_callback(move |requested, include_type, requesting_source, _depth| {
        let is_relative = include_type == shaderc::IncludeType::Relative;
        match resolve_include(requested, is_relative, requesting_source, &search_dirs) {
            Some((path, content)) => Ok(shaderc::ResolvedInclude {
                resolved_name: path.to_string_lossy().into_owned(),
                content,
            }),
            None => Err(format!("include not found: {requested}")),
        }
    });

    Ok(copts)
}

fn input_filename(options: &Options) -> String {
    options
        .input_file_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string()
}

pub fn preprocess_src(
    options: &Options,
    source: &str,
    target: TranspileTarget,
) -> Result<String, PrepareError> {
    let compiler = shaderc::Compiler::new()?;
    let copts = make_compile_options(options, target)?;
    let filename = input_filename(options);

    Ok(compiler
        .preprocess(source, &filename, "main", Some(&copts))?
        .as_text())
}

pub fn compile_src_to_spirv(
    options: &Options,
    source: &str,
    target: TranspileTarget,
) -> Result<PreparedSPIRV, PrepareError> {
    let stage = options.shader_type;

    let compiler = shaderc::Compiler::new()?;
    let copts = make_compile_options(options, target)?;
    let filename = input_filename(options);

    let preprocessed = compiler
        .preprocess(source, &filename, "main", Some(&copts))?
        .as_text();

    let artifact = compiler.compile_into_spirv(
        &preprocessed,
        stage.shaderc_kind(),
        &filename,
        "main",
        Some(&copts),
    )?;

    let spirv_words = artifact.as_binary().to_vec();

    let io = reflect_io(&spirv_words, stage)?;

    Ok(PreparedSPIRV {
        filename,
        expanded: preprocessed,
        io,
        spirv_words,
    })
}

fn include_search_dirs(options: &Options) -> Vec<PathBuf> {
    let mut dirs = Vec::with_capacity(options.include_dirs.len() + 1);
    if let Some(parent) = options.input_file_path.parent() {
        dirs.push(parent.to_path_buf());
    }
    dirs.extend(options.include_dirs.iter().cloned());
    dirs
}

fn resolve_include(
    requested: &str,
    is_relative: bool,
    requesting_source: &str,
    search_dirs: &[PathBuf],
) -> Option<(PathBuf, String)> {
    if is_relative && let Some(requesting_dir) = Path::new(requesting_source).parent() {
        let candidate = requesting_dir.join(requested);
        if let Ok(content) = std::fs::read_to_string(&candidate) {
            return Some((candidate, content));
        }
    }
    for dir in search_dirs {
        let candidate = dir.join(requested);
        if let Ok(content) = std::fs::read_to_string(&candidate) {
            return Some((candidate, content));
        }
    }
    None
}
