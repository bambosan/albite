use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderType {
    Vertex,
    Fragment,
    Compute,
}

impl ShaderType {
    pub fn from_char(c: char) -> Option<Self> {
        match c.to_ascii_lowercase() {
            'v' => Some(Self::Vertex),
            'f' => Some(Self::Fragment),
            'c' => Some(Self::Compute),
            _ => None,
        }
    }

    pub fn is_vertex(self) -> bool {
        matches!(self, Self::Vertex)
    }

    pub fn is_fragment(self) -> bool {
        matches!(self, Self::Fragment)
    }

    pub fn is_compute(self) -> bool {
        matches!(self, Self::Compute)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Vertex => "vertex",
            Self::Fragment => "fragment",
            Self::Compute => "compute",
        }
    }

    pub fn shaderc_kind(self) -> shaderc::ShaderKind {
        match self {
            Self::Vertex => shaderc::ShaderKind::Vertex,
            Self::Fragment => shaderc::ShaderKind::Fragment,
            Self::Compute => shaderc::ShaderKind::Compute,
        }
    }

    pub fn defines(self) -> (&'static str, Option<&'static str>) {
        match self {
            Self::Vertex => ("SHADER_STAGE__VERTEX", Some("1")),
            Self::Fragment => ("SHADER_STAGE__FRAGMENT", Some("1")),
            Self::Compute => ("SHADER_STAGE__COMPUTE", Some("1")),
        }
    }
}

impl Default for ShaderType {
    fn default() -> Self {
        Self::Vertex
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationLevel {
    O0,
    O1,
    O2,
}

impl OptimizationLevel {
    pub fn from_flag(flag: &str) -> Option<Self> {
        match flag {
            "-O0" => Some(Self::O0),
            "-O1" => Some(Self::O1),
            "-O2" => Some(Self::O2),
            _ => None,
        }
    }
}

impl Default for OptimizationLevel {
    fn default() -> Self {
        Self::O2
    }
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub shader_type: ShaderType,
    pub profile: String,
    pub input_file_path: PathBuf,
    pub output_file_path: PathBuf,
    pub include_dirs: Vec<PathBuf>,
    pub defines: Vec<(String, Option<String>)>,
    pub optimization: OptimizationLevel,
    pub verbose: bool,
    pub preprocess_only: bool,
}

impl Options {
    pub fn new() -> Self {
        Self::default()
    }
}
