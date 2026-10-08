use crate::{
    error::DxilError,
    hlsl_patch::{Field, StructLayout},
};
use albite_core::{spirv_reflect::*, uniform::*};
use spirv_cross2::{
    Compiler,
    handle::{Handle, TypeId},
    reflect::ResourceType,
    reflect::{BitWidth, Scalar, ScalarKind, TypeInner},
    targets::Hlsl,
};

pub struct StorageBufferInfo {
    pub name: String,
    pub struct_name: String,
    pub layout: StructLayout,
}

fn scalar_base(s: Scalar) -> Option<&'static str> {
    match (s.kind, s.size) {
        (ScalarKind::Float, BitWidth::Word) => Some("float"),
        (ScalarKind::Int, BitWidth::Word) => Some("int"),
        (ScalarKind::Uint, BitWidth::Word) => Some("uint"),
        _ => None,
    }
}

fn ssbo_element(
    compiler: &Compiler<Hlsl>,
    block_type: Handle<TypeId>,
) -> Result<Option<(String, StructLayout)>, DxilError> {
    let block = compiler.type_description(block_type)?;
    let TypeInner::Struct(block_struct) = block.inner else {
        return Ok(None);
    };
    let Some(last) = block_struct.members.last() else {
        return Ok(None);
    };
    let array_ty = compiler.type_description(last.id)?;
    let TypeInner::Array { base, stride, .. } = array_ty.inner else {
        return Ok(None);
    };
    let elem = compiler.type_description(base)?;
    let (TypeInner::Struct(elem_struct), Some(struct_name)) = (&elem.inner, &elem.name) else {
        return Ok(None);
    };
    let struct_name = struct_name.to_string();
    let array_stride = last.array_stride.or(stride).ok_or_else(|| {
        DxilError::HlslPatch(format!("`{struct_name}`: runtime array has no stride"))
    })?;

    let mut fields = Vec::with_capacity(elem_struct.members.len());
    for member in &elem_struct.members {
        let name = member
            .name
            .as_ref()
            .map(|n| n.to_string())
            .ok_or_else(|| DxilError::HlslPatch(format!("`{struct_name}`: unnamed member")))?;
        let ty = compiler.type_description(member.id)?;
        let (base, count) = match ty.inner {
            TypeInner::Scalar(s) => (scalar_base(s), 1),
            TypeInner::Vector { width, scalar } => (scalar_base(scalar), width as usize),
            _ => (None, 0),
        };
        let Some(base) = base else {
            return Err(DxilError::HlslPatch(format!(
                "`{struct_name}.{name}`: only scalar and vector members are supported"
            )));
        };
        fields.push(Field {
            name,
            base,
            count,
            offset: member.offset as usize,
        });
    }

    let layout = StructLayout::new(fields, array_stride as usize)
        .map_err(|e| DxilError::HlslPatch(format!("`{struct_name}`: {e}")))?;
    Ok(Some((struct_name, layout)))
}

pub fn reflect_uniforms(
    compiler: &mut Compiler<Hlsl>,
) -> Result<(Vec<Uniform>, u16, Vec<StorageBufferInfo>), DxilError> {
    let resources = compiler.shader_resources()?;
    let mut out = Vec::new();
    let mut ubo_size = 0u16;

    for resource in resources.resources_for_type(ResourceType::UniformBuffer)? {
        let block_type = compiler.type_description(resource.base_type_id)?;

        if let TypeInner::Struct(struct_type) = block_type.inner {
            ubo_size = struct_type.size.min(u16::MAX as usize) as u16;

            for member in &struct_type.members {
                if let Some(name) = &member.name {
                    let (shape, count, reg_count) = describe_type_id(compiler, member.id)?;
                    let mut uniform = shape.new_uniform(name.to_string());
                    uniform.count = count;
                    uniform.reg_index = member.offset as u16;
                    uniform.reg_count = reg_count;
                    out.push(uniform);
                }
            }
        }
    }

    for resource in resources.resources_for_type(ResourceType::SampledImage)? {
        let binding = resource_binding(compiler, resource.id)?;
        let mut uniform = Uniform::new_sampler_dxil(resource.name.to_string());
        uniform.count = 1;
        uniform.reg_index = binding.unwrap_or(0) as u16;
        uniform.reg_count = 1;
        out.push(uniform);
    }

    let mut ssbos = Vec::new();
    for resource in resources.resources_for_type(ResourceType::StorageBuffer)? {
        if let Some((struct_name, layout)) = ssbo_element(compiler, resource.base_type_id)? {
            ssbos.push(StorageBufferInfo {
                name: resource.name.to_string(),
                struct_name,
                layout,
            });
        }

        compiler.set_name(resource.id, resource.name)?;
    }

    Ok((out, ubo_size, ssbos))
}
