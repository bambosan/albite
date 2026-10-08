use albite_core::{spirv_reflect::*, uniform::*};
use spirv_cross2::{
    Compiler,
    reflect::{ExecutionModeArguments, ResourceType, TypeInner},
    spirv::ExecutionMode,
    targets::Msl,
};

pub fn reflect_uniforms(
    compiler: &mut Compiler<Msl>,
) -> Result<(Vec<Uniform>, u16), spirv_cross2::SpirvCrossError> {
    let resources = compiler.shader_resources()?;
    let mut out = Vec::new();
    let mut ubo_size = 0u16;

    for resource in resources.resources_for_type(ResourceType::UniformBuffer)? {
        compiler.set_name(resource.id, "_mtl_u")?;

        let block_type = compiler.type_description(resource.base_type_id)?;

        if let TypeInner::Struct(struct_type) = block_type.inner {
            ubo_size = struct_type.size.min(u16::MAX as usize) as u16;

            for member in &struct_type.members {
                if let Some(name) = &member.name {
                    let (shape, count, reg_count) = describe_type_id(compiler, member.id)?;
                    let mut uniform = shape.new_uniform(name.to_string());
                    uniform.count = count.max(1);
                    uniform.reg_index = member.offset as u16;
                    uniform.reg_count = reg_count;
                    out.push(uniform);
                }
            }
        }
    }

    for resource in resources.resources_for_type(ResourceType::SampledImage)? {
        let binding = resource_binding(compiler, resource.id)?;
        let mut uniform = Uniform::new_sampler(resource.name.to_string());
        uniform.reg_index = binding.unwrap_or(0) as u16;
        out.push(uniform);
    }

    for resource in resources.resources_for_type(ResourceType::StorageImage)? {
        let binding = resource_binding(compiler, resource.id)?;
        let mut uniform = Uniform::new_sampler(resource.name.to_string());
        uniform.reg_index = binding.unwrap_or(0) as u16;
        out.push(uniform);
    }

    for resource in resources.resources_for_type(ResourceType::StorageBuffer)? {
        let binding = resource_binding(compiler, resource.id)?;
        if let Ok(opt) = compiler.name(resource.id) {
            if let Some(name) = opt {
                let mut uniform = Uniform::new_sampler(name.to_string());
                uniform.reg_index = binding.unwrap_or(0) as u16;
                out.push(uniform);
            }
        }
    }

    Ok((out, ubo_size))
}

pub fn extract_local_size<T>(cross: &Compiler<T>) -> Option<[u16; 3]> {
    match cross
        .execution_mode_arguments(ExecutionMode::LocalSize)
        .ok()?
    {
        Some(ExecutionModeArguments::LocalSize { x, y, z }) => Some([x as u16, y as u16, z as u16]),
        _ => None,
    }
}
