use crate::{
    attrib::Attrib,
    options::ShaderType,
    stage_io::{IoVar, ShaderIo},
    uniform::Uniform,
};
use serde::Serialize;

const MAGIC: [u8; 4] = *b"ALBI";
const VERSION: u16 = 1;

const TAG_META: [u8; 4] = *b"META";
const TAG_CODE: [u8; 4] = *b"CODE";

const HEADER_SIZE: usize = 8;
const TABLE_ENTRY_SIZE: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Glsl,
    Msl,
    Dxil,
}

impl Backend {
    fn name(self) -> &'static str {
        match self {
            Self::Glsl => "glsl",
            Self::Msl => "msl",
            Self::Dxil => "dxil",
        }
    }
}

pub struct StageOutput<'a> {
    pub backend: Backend,
    pub stage: ShaderType,
    pub io: &'a ShaderIo,
    pub vertex_attrib_ids: &'a [u16],
    pub uniforms: &'a [Uniform],
    pub ubo_size: u16,
    pub local_size: Option<[u16; 3]>,
    pub code: &'a [u8],
}

#[derive(Serialize)]
struct Meta<'a> {
    producer: Producer,
    backend: &'static str,
    stage: &'static str,
    io: Io<'a>,
    resources: Resources<'a>,
}

#[derive(Serialize)]
struct Producer {
    name: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct Io<'a> {
    inputs: Vec<IoVarOut<'a>>,
    outputs: Vec<IoVarOut<'a>>,
}

#[derive(Serialize)]
struct IoVarOut<'a> {
    name: &'a str,
    location: u32,
    #[serde(rename = "type")]
    ty: &'static str,
    semantic: String,
    per_instance: bool,
    precision: &'static str,
    interpolation: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    attrib_id: Option<u16>,
}

#[derive(Serialize)]
struct Resources<'a> {
    uniforms: Vec<UniformOut<'a>>,
    ubo_size: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    local_size: Option<[u16; 3]>,
}

#[derive(Serialize)]
struct UniformOut<'a> {
    name: &'a str,
    #[serde(rename = "type")]
    ty: &'static str,
    count: u8,
    reg_index: u16,
    reg_count: u16,
}

pub fn write_container(output: &StageOutput) -> Result<Vec<u8>, String> {
    let is_vertex = output.stage.is_vertex();

    let expected_ids = if is_vertex { output.io.inputs.len() } else { 0 };
    if output.vertex_attrib_ids.len() != expected_ids {
        return Err(format!(
            "expected {expected_ids} vertex attribute ids, got {}.",
            output.vertex_attrib_ids.len()
        ));
    }

    let mut inputs = Vec::with_capacity(output.io.inputs.len());
    for (i, var) in output.io.inputs.iter().enumerate() {
        let attrib_id = is_vertex.then(|| output.vertex_attrib_ids[i]);
        inputs.push(io_var_out(var, output.stage, true, attrib_id)?);
    }
    let mut outputs = Vec::with_capacity(output.io.outputs.len());
    for var in &output.io.outputs {
        outputs.push(io_var_out(var, output.stage, false, None)?);
    }

    let meta = Meta {
        producer: Producer {
            name: "albite",
            version: env!("CARGO_PKG_VERSION"),
        },
        backend: output.backend.name(),
        stage: output.stage.name(),
        io: Io { inputs, outputs },
        resources: Resources {
            uniforms: output
                .uniforms
                .iter()
                .map(|u| UniformOut {
                    name: &u.name,
                    ty: u.base_type.name(),
                    count: u.count,
                    reg_index: u.reg_index,
                    reg_count: u.reg_count,
                })
                .collect(),
            ubo_size: output.ubo_size,
            local_size: output.local_size,
        },
    };
    let meta_json = serde_json::to_vec(&meta).map_err(|e| e.to_string())?;

    assemble(&[(TAG_META, &meta_json), (TAG_CODE, output.code)])
}

fn io_var_out<'a>(
    var: &'a IoVar,
    stage: ShaderType,
    is_input: bool,
    attrib_id: Option<u16>,
) -> Result<IoVarOut<'a>, String> {
    let (semantic, per_instance) = if stage.is_vertex() && is_input {
        let attrib = Attrib::from_str(&var.name).unwrap();
        (attrib.semantic().to_string(), attrib.is_instance_data())
    } else {
        (format!("TEXCOORD{}", var.location), false)
    };

    Ok(IoVarOut {
        name: &var.name,
        location: var.location,
        ty: var.ty.name(),
        semantic,
        per_instance,
        precision: var.precision.name(),
        interpolation: var.interpolation.name(),
        attrib_id,
    })
}

fn assemble(sections: &[([u8; 4], &[u8])]) -> Result<Vec<u8>, String> {
    let too_large = || "output is too large for the container format (4 GiB limit)".to_string();

    let data_start = HEADER_SIZE + TABLE_ENTRY_SIZE * sections.len();
    let total = sections
        .iter()
        .try_fold(data_start, |total, (_, data)| total.checked_add(data.len()))
        .ok_or_else(too_large)?;
    u32::try_from(total).map_err(|_| too_large())?;

    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(sections.len() as u16).to_le_bytes());

    let mut offset = data_start;
    for (tag, data) in sections {
        out.extend_from_slice(tag);
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        offset += data.len();
    }
    for (_, data) in sections {
        out.extend_from_slice(data);
    }

    Ok(out)
}
