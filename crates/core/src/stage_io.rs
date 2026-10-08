use crate::{error::PrepareError, options::ShaderType};
use spirv_cross2::{
    Compiler, Module,
    handle::{Handle, Id},
    reflect::{BitWidth, DecorationValue, Resource, ResourceType, ScalarKind, TypeInner},
    spirv,
    targets::None as NoTarget,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoType {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Int,
    IVec2,
    IVec3,
    IVec4,
    UInt,
    UVec2,
    UVec3,
    UVec4,
    Mat4,
}

impl IoType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::Vec2 => "vec2",
            Self::Vec3 => "vec3",
            Self::Vec4 => "vec4",
            Self::Int => "int",
            Self::IVec2 => "ivec2",
            Self::IVec3 => "ivec3",
            Self::IVec4 => "ivec4",
            Self::UInt => "uint",
            Self::UVec2 => "uvec2",
            Self::UVec3 => "uvec3",
            Self::UVec4 => "uvec4",
            Self::Mat4 => "mat4",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    None,
    Flat,
    NoPerspective,
    Centroid,
}

impl Interpolation {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Flat => "flat",
            Self::NoPerspective => "noperspective",
            Self::Centroid => "centroid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    None,
    Mediump,
}

impl Precision {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Mediump => "mediump",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoVar {
    pub name: String,
    pub location: u32,
    pub ty: IoType,
    pub interpolation: Interpolation,
    pub precision: Precision,
}

#[derive(Debug, Default, Clone)]
pub struct ShaderIo {
    pub inputs: Vec<IoVar>,
    pub outputs: Vec<IoVar>,
}

impl ShaderIo {
    pub fn input_names(&self) -> Vec<&str> {
        self.inputs.iter().map(|v| v.name.as_str()).collect()
    }

    pub fn output_names(&self) -> Vec<&str> {
        self.outputs.iter().map(|v| v.name.as_str()).collect()
    }
}

pub fn reflect_io(spirv_words: &[u32], stage: ShaderType) -> Result<ShaderIo, PrepareError> {
    if stage.is_compute() {
        return Ok(ShaderIo::default());
    }

    let compiler = Compiler::<NoTarget>::new(Module::from_words(spirv_words))?;
    let resources = compiler.shader_resources()?;

    let mut io = ShaderIo::default();

    for resource in resources.resources_for_type(ResourceType::StageInput)? {
        io.inputs.push(reflect_var(&compiler, &resource, "input")?);
    }

    if stage.is_vertex() {
        for resource in resources.resources_for_type(ResourceType::StageOutput)? {
            io.outputs
                .push(reflect_var(&compiler, &resource, "output")?);
        }
    }

    io.inputs
        .sort_by(|a, b| (a.location, &a.name).cmp(&(b.location, &b.name)));
    io.outputs
        .sort_by(|a, b| (a.location, &a.name).cmp(&(b.location, &b.name)));

    Ok(io)
}

fn reflect_var(
    compiler: &Compiler<NoTarget>,
    resource: &Resource<'_>,
    direction: &str,
) -> Result<IoVar, PrepareError> {
    let name = resource.name.to_string();
    let unsupported = |reason: &str| PrepareError::UnsupportedIo {
        direction: direction.to_string(),
        name: name.clone(),
        reason: reason.to_string(),
    };

    if has_decoration(compiler, resource.base_type_id, spirv::Decoration::Block)? {
        return Err(unsupported(
            "interface blocks are not supported, declare each variable separately",
        ));
    }

    if let TypeInner::Array { .. } = compiler.type_description(resource.type_id)?.inner {
        return Err(unsupported("arrays are not supported"));
    }

    let ty = compiler
        .type_description(resource.base_type_id)
        .map_err(|e| {
            unsupported(&format!(
                "type cannot be described, structs are not supported ({e})"
            ))
        })?;
    let io_type = io_type_of(&ty.inner).ok_or_else(|| {
        unsupported("only float/int/uint scalars and vectors, and mat4, are supported")
    })?;

    let location = match compiler.decoration(resource.id, spirv::Decoration::Location)? {
        Some(DecorationValue::Literal(location)) => location,
        _ => return Err(unsupported("variable has no location")),
    };

    let has = |decoration| has_decoration(compiler, resource.id, decoration);

    let interpolation = if has(spirv::Decoration::Flat)? {
        Interpolation::Flat
    } else if has(spirv::Decoration::NoPerspective)? {
        Interpolation::NoPerspective
    } else if has(spirv::Decoration::Centroid)? {
        Interpolation::Centroid
    } else {
        Interpolation::None
    };

    let precision = if has(spirv::Decoration::RelaxedPrecision)? {
        Precision::Mediump
    } else {
        Precision::None
    };

    Ok(IoVar {
        name,
        location,
        ty: io_type,
        interpolation,
        precision,
    })
}

fn has_decoration<T: Id>(
    compiler: &Compiler<NoTarget>,
    id: Handle<T>,
    decoration: spirv::Decoration,
) -> Result<bool, PrepareError> {
    Ok(compiler.decoration(id, decoration)?.is_some())
}

fn io_type_of(inner: &TypeInner<'_>) -> Option<IoType> {
    let is_32bit = |scalar: &spirv_cross2::reflect::Scalar| scalar.size == BitWidth::Word;

    match inner {
        TypeInner::Scalar(scalar) if is_32bit(scalar) => match scalar.kind {
            ScalarKind::Float => Some(IoType::Float),
            ScalarKind::Int => Some(IoType::Int),
            ScalarKind::Uint => Some(IoType::UInt),
            ScalarKind::Bool => None,
        },
        TypeInner::Vector { width, scalar } if is_32bit(scalar) => match (scalar.kind, width) {
            (ScalarKind::Float, 2) => Some(IoType::Vec2),
            (ScalarKind::Float, 3) => Some(IoType::Vec3),
            (ScalarKind::Float, 4) => Some(IoType::Vec4),
            (ScalarKind::Int, 2) => Some(IoType::IVec2),
            (ScalarKind::Int, 3) => Some(IoType::IVec3),
            (ScalarKind::Int, 4) => Some(IoType::IVec4),
            (ScalarKind::Uint, 2) => Some(IoType::UVec2),
            (ScalarKind::Uint, 3) => Some(IoType::UVec3),
            (ScalarKind::Uint, 4) => Some(IoType::UVec4),
            _ => None,
        },
        TypeInner::Matrix {
            columns: 4,
            rows: 4,
            scalar,
        } if is_32bit(scalar) && scalar.kind == ScalarKind::Float => Some(IoType::Mat4),
        _ => None,
    }
}
