#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UniformType {
    Sampler,
    Vec4,
    Mat3,
    Mat4,
    SamplerDXIL,
}

impl UniformType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Sampler => "sampler",
            Self::Vec4 => "vec4",
            Self::Mat3 => "mat3",
            Self::Mat4 => "mat4",
            Self::SamplerDXIL => "sampler_dxil",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Uniform {
    pub name: String,
    pub base_type: UniformType,
    pub count: u8,
    pub reg_index: u16,
    pub reg_count: u16,
}

impl Uniform {
    pub fn new_vec4(name: String) -> Self {
        Self {
            name: name,
            base_type: UniformType::Vec4,
            count: 0,
            reg_index: u16::MAX,
            reg_count: 0,
        }
    }

    pub fn new_mat3(name: String) -> Self {
        Self {
            name: name,
            base_type: UniformType::Mat3,
            count: 0,
            reg_index: u16::MAX,
            reg_count: 0,
        }
    }

    pub fn new_mat4(name: String) -> Self {
        Self {
            name: name,
            base_type: UniformType::Mat4,
            count: 0,
            reg_index: u16::MAX,
            reg_count: 0,
        }
    }

    pub fn new_sampler(name: String) -> Self {
        Self {
            name: name,
            base_type: UniformType::Sampler,
            count: 0,
            reg_index: u16::MAX,
            reg_count: 0,
        }
    }

    pub fn new_sampler_dxil(name: String) -> Self {
        Self {
            name: name,
            base_type: UniformType::SamplerDXIL,
            count: 0,
            reg_index: u16::MAX,
            reg_count: 0,
        }
    }
}
