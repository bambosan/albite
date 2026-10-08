use albite_core::{options::*, spirv_reflect::*, uniform::*};
use spirv_cross2::{
    Compiler,
    reflect::{DecorationValue, ResourceType},
    spirv,
    targets::Glsl,
};

pub fn reflect_uniforms(
    compiler: &mut Compiler<Glsl>,
) -> Result<Vec<Uniform>, spirv_cross2::SpirvCrossError> {
    let mut out = Vec::new();
    let resources = compiler.shader_resources()?;

    for resource in resources.resources_for_type(ResourceType::GlPlainUniform)? {
        let (shape, count, reg_count) = describe_type_id(compiler, resource.base_type_id)?;
        let mut uniform = shape.new_uniform(resource.name.to_string());
        uniform.count = count.max(1);
        uniform.reg_count = reg_count;
        out.push(uniform);
    }

    for resource in resources.resources_for_type(ResourceType::SampledImage)? {
        let mut uniform = Uniform::new_sampler(resource.name.to_string());
        uniform.count = 1;
        uniform.reg_count = 1;
        out.push(uniform);
    }

    for resource in resources.resources_for_type(ResourceType::StorageImage)? {
        let mut uniform = Uniform::new_sampler(resource.name.to_string());
        uniform.count = 1;
        uniform.reg_count = 1;
        out.push(uniform);
    }

    for resource in resources.resources_for_type(ResourceType::StorageBuffer)? {
        if let Ok(opt) = compiler.name(resource.id) {
            if let Some(name) = opt {
                let mut uniform = Uniform::new_sampler(name.to_string());
                uniform.count = 1;
                uniform.reg_index = 0;
                uniform.reg_count = 1;
                out.push(uniform);
            }
        }
    }

    Ok(out)
}

pub fn strip_decorations(
    compiler: &mut Compiler<Glsl>,
    stage: ShaderType,
) -> Result<(), spirv_cross2::SpirvCrossError> {
    let resources = compiler.shader_resources()?;

    for resource in resources.resources_for_type(ResourceType::StageInput)? {
        compiler.set_decoration(
            resource.id,
            spirv::Decoration::Location,
            DecorationValue::unset(),
        )?;
    }

    if stage == ShaderType::Vertex {
        for resource in resources.resources_for_type(ResourceType::StageOutput)? {
            compiler.set_decoration(
                resource.id,
                spirv::Decoration::Location,
                DecorationValue::unset(),
            )?;
        }
    }

    for resource in resources.resources_for_type(ResourceType::GlPlainUniform)? {
        compiler.set_decoration(
            resource.id,
            spirv::Decoration::Location,
            DecorationValue::unset(),
        )?;
        compiler.set_decoration(
            resource.id,
            spirv::Decoration::Binding,
            DecorationValue::unset(),
        )?;
    }

    for resource in resources.resources_for_type(ResourceType::StorageBuffer)? {
        compiler.set_decoration(
            resource.id,
            spirv::Decoration::Location,
            DecorationValue::unset(),
        )?;
    }

    for resource in resources.resources_for_type(ResourceType::StorageImage)? {
        compiler.set_decoration(
            resource.id,
            spirv::Decoration::Location,
            DecorationValue::unset(),
        )?;
    }

    for resource in resources.resources_for_type(ResourceType::SampledImage)? {
        compiler.set_decoration(
            resource.id,
            spirv::Decoration::Location,
            DecorationValue::unset(),
        )?;
        compiler.set_decoration(
            resource.id,
            spirv::Decoration::Binding,
            DecorationValue::unset(),
        )?;
    }

    Ok(())
}
