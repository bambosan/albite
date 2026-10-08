#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Attrib {
    Position,
    Normal,
    Tangent,
    Bitangent,
    Color0,
    Color1,
    Color2,
    Color3,
    Indices,
    Weight,
    TexCoord0,
    TexCoord1,
    TexCoord2,
    TexCoord3,
    TexCoord4,
    TexCoord5,
    TexCoord6,
    TexCoord7,
    TexCoord8,
    Idata0,
    Idata1,
    Idata2,
    Idata3,
    Idata4,
}

impl Attrib {
    pub fn from_str(name: &str) -> Option<Self> {
        match name {
            "a_position" => Some(Self::Position),
            "a_normal" => Some(Self::Normal),
            "a_tangent" => Some(Self::Tangent),
            "a_bitangent" => Some(Self::Bitangent),
            "a_color0" => Some(Self::Color0),
            "a_color1" => Some(Self::Color1),
            "a_color2" => Some(Self::Color2),
            "a_color3" => Some(Self::Color3),
            "a_indices" => Some(Self::Indices),
            "a_weight" => Some(Self::Weight),
            "a_texcoord0" => Some(Self::TexCoord0),
            "a_texcoord1" => Some(Self::TexCoord1),
            "a_texcoord2" => Some(Self::TexCoord2),
            "a_texcoord3" => Some(Self::TexCoord3),
            "a_texcoord4" => Some(Self::TexCoord4),
            "a_texcoord5" => Some(Self::TexCoord5),
            "a_texcoord6" => Some(Self::TexCoord6),
            "a_texcoord7" => Some(Self::TexCoord7),
            "a_texcoord8" => Some(Self::TexCoord8),
            "i_data0" => Some(Self::Idata0),
            "i_data1" => Some(Self::Idata1),
            "i_data2" => Some(Self::Idata2),
            "i_data3" => Some(Self::Idata3),
            "i_data4" => Some(Self::Idata4),
            _ => None,
        }
    }

    pub fn semantic(self) -> &'static str {
        match self {
            Self::Position => "POSITION",
            Self::Normal => "NORMAL",
            Self::Tangent => "TANGENT",
            Self::Bitangent => "BITANGENT",
            Self::Color0 => "COLOR0",
            Self::Color1 => "COLOR1",
            Self::Color2 => "COLOR2",
            Self::Color3 => "COLOR3",
            Self::Indices => "BLENDINDICES",
            Self::Weight => "BLENDWEIGHT",
            Self::TexCoord0 => "TEXCOORD0",
            Self::TexCoord1 => "TEXCOORD1",
            Self::TexCoord2 => "TEXCOORD2",
            Self::TexCoord3 => "TEXCOORD3",
            Self::TexCoord4 => "TEXCOORD4",
            Self::TexCoord5 => "TEXCOORD5",
            Self::TexCoord6 => "TEXCOORD6",
            Self::TexCoord7 => "TEXCOORD7",
            Self::TexCoord8 => "TEXCOORD8",
            Self::Idata0 => "TEXCOORD8",
            Self::Idata1 => "TEXCOORD7",
            Self::Idata2 => "TEXCOORD6",
            Self::Idata3 => "TEXCOORD5",
            Self::Idata4 => "TEXCOORD4",
        }
    }

    pub fn id(self) -> u16 {
        match self {
            Self::Position => 0x1,
            Self::Normal => 0x2,
            Self::Tangent => 0x3,
            Self::Bitangent => 0x4,
            Self::Color0 => 0x5,
            Self::Color1 => 0x6,
            Self::Color2 => 0x18,
            Self::Color3 => 0x19,
            Self::Indices => 0xe,
            Self::Weight => 0xf,
            Self::TexCoord0 => 0x10,
            Self::TexCoord1 => 0x11,
            Self::TexCoord2 => 0x12,
            Self::TexCoord3 => 0x13,
            Self::TexCoord4 => 0x14,
            Self::TexCoord5 => 0x15,
            Self::TexCoord6 => 0x16,
            Self::TexCoord7 => 0x17,
            Self::TexCoord8 => 0x1a,
            Self::Idata0 => u16::MAX,
            Self::Idata1 => u16::MAX,
            Self::Idata2 => u16::MAX,
            Self::Idata3 => u16::MAX,
            Self::Idata4 => u16::MAX,
        }
    }

    pub fn is_instance_data(self) -> bool {
        matches!(
            self,
            Self::Idata0 | Self::Idata1 | Self::Idata2 | Self::Idata3 | Self::Idata4
        )
    }
}

pub fn validate_attrib_names<'a, I>(inputs: I) -> Result<(), String>
where
    I: IntoIterator<Item = &'a str>,
{
    for name in inputs {
        if !Attrib::from_str(name).is_some() {
            return Err(format!(
                "Invalid vertex shader input attribute '{name}'.\n\
                Valid input attributes: a_position, a_normal, a_tangent, a_bitangent, a_indices, a_weight, a_color0-3, a_texcoord0-8, i_data0-4.\n"
            ));
        }
    }
    Ok(())
}

pub fn attrib_ids_from_names<'a, I>(inputs: I) -> Vec<u16>
where
    I: IntoIterator<Item = &'a str>,
{
    inputs
        .into_iter()
        .map(|name| Attrib::from_str(name).unwrap().id())
        .collect()
}
