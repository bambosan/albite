use crate::uniform::Uniform;
use spirv_cross2::{
    Compiler,
    handle::{Handle, Id, TypeId},
    reflect::{ArrayDimension, DecorationValue, TypeInner},
    spirv,
};

pub enum TypeShape {
    Vec4,
    Mat3,
    Mat4,
}

impl TypeShape {
    pub fn new_uniform(&self, name: String) -> Uniform {
        match self {
            Self::Vec4 => Uniform::new_vec4(name),
            Self::Mat3 => Uniform::new_mat3(name),
            Self::Mat4 => Uniform::new_mat4(name),
        }
    }
}

pub fn describe_type_id<Target>(
    compiler: &Compiler<Target>,
    type_id: Handle<TypeId>,
) -> Result<(TypeShape, u8, u16), spirv_cross2::SpirvCrossError> {
    let ty = compiler.type_description(type_id)?;
    Ok(match ty.inner {
        TypeInner::Array {
            base, dimensions, ..
        } => {
            let count = match dimensions.first() {
                Some(ArrayDimension::Literal(n)) => (*n).clamp(1, u8::MAX as u32) as u8,
                _ => 1,
            };
            let (shape, _, reg_count) = describe_type_id(compiler, base)?;
            (shape, count, count as u16 * reg_count)
        }
        TypeInner::Matrix {
            columns: 3,
            rows: 3,
            ..
        } => (TypeShape::Mat3, 0, 3),
        TypeInner::Matrix {
            columns: 4,
            rows: 4,
            ..
        } => (TypeShape::Mat4, 0, 4),
        _ => (TypeShape::Vec4, 0, 1),
    })
}

pub fn resource_binding<Target, T>(
    compiler: &Compiler<Target>,
    id: Handle<T>,
) -> Result<Option<u32>, spirv_cross2::SpirvCrossError>
where
    Handle<T>: Copy,
    T: Id,
{
    Ok(match compiler.decoration(id, spirv::Decoration::Binding)? {
        Some(DecorationValue::Literal(binding)) => Some(binding),
        _ => None,
    })
}
