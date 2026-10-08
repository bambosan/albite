use rspirv::binary::Assemble;
use rspirv::dr::{Instruction, Operand, load_words};
use rspirv::spirv::{Decoration, Op, StorageClass, Word};
use std::collections::{HashMap, HashSet};

pub struct Member {
    pub name: String,
    pub offset: u32,
    pub size: u32,
}
pub struct Packed {
    pub words: Vec<u32>,
    pub members: Vec<Member>,
    pub buffer_size: u32,
}

struct Ids(u32);
impl Ids {
    fn next(&mut self) -> Word {
        let i = self.0;
        self.0 += 1;
        i
    }
}

type Res<T> = Result<T, String>;
type Types = HashMap<Word, Instruction>;

#[derive(Clone, Copy)]
struct Layout {
    align: u32,
    size: u32,
    mat_stride: Option<u32>,
}

fn lit(i: &Instruction, n: usize) -> Res<u32> {
    match i.operands.get(n) {
        Some(Operand::LiteralBit32(v)) => Ok(*v),
        _ => Err(format!(
            "{:?}: expected 32-bit literal at operand {n}",
            i.class.opcode
        )),
    }
}

fn idref(i: &Instruction, n: usize) -> Res<Word> {
    match i.operands.get(n) {
        Some(Operand::IdRef(v)) => Ok(*v),
        _ => Err(format!("{:?}: expected id at operand {n}", i.class.opcode)),
    }
}

fn ovf() -> String {
    "uniform layout size overflow".to_string()
}

fn round_up(v: u32, a: u32) -> Res<u32> {
    if a == 0 {
        return Err("round_up by zero alignment".into());
    }
    let n = v.checked_add(a - 1).ok_or_else(ovf)? / a;
    n.checked_mul(a).ok_or_else(ovf)
}

fn mul(a: u32, b: u32) -> Res<u32> {
    a.checked_mul(b).ok_or_else(ovf)
}

fn ptr_pointee(types: &Types, ptr_ty: Word) -> Res<Word> {
    let t = types
        .get(&ptr_ty)
        .ok_or_else(|| format!("unknown pointer type id {ptr_ty}"))?;
    idref(t, 1)
}

fn is_opaque(t: &Types, id: Word) -> bool {
    let Some(i) = t.get(&id) else { return false };
    match i.class.opcode {
        Op::TypeImage
        | Op::TypeSampler
        | Op::TypeSampledImage
        | Op::TypeAccelerationStructureKHR => true,
        Op::TypeArray | Op::TypeRuntimeArray => idref(i, 0).map_or(false, |el| is_opaque(t, el)),
        _ => false,
    }
}

fn std140(t: &Types, id: Word, strides: &mut Vec<(Word, u32)>) -> Res<Layout> {
    let i = t.get(&id).ok_or_else(|| format!("unknown type id {id}"))?;
    match i.class.opcode {
        Op::TypeInt | Op::TypeFloat => {
            let w = lit(i, 0)?;
            if w < 8 || w % 8 != 0 {
                return Err(format!("unsupported scalar width {w}"));
            }
            Ok(Layout {
                align: w / 8,
                size: w / 8,
                mat_stride: None,
            })
        }
        Op::TypeVector => {
            let c = std140(t, idref(i, 0)?, strides)?;
            let n = lit(i, 1)?;
            let align = mul(c.align, if n == 2 { 2 } else { 4 })?;
            Ok(Layout {
                align,
                size: mul(c.size, n)?,
                mat_stride: None,
            })
        }
        Op::TypeMatrix => {
            let col = std140(t, idref(i, 0)?, strides)?;
            let stride = round_up(col.align, 16)?;
            Ok(Layout {
                align: stride,
                size: mul(stride, lit(i, 1)?)?,
                mat_stride: Some(stride),
            })
        }
        Op::TypeArray => {
            let el = std140(t, idref(i, 0)?, strides)?;
            let len_id = idref(i, 1)?;
            let c = t.get(&len_id).ok_or("unknown array length id")?;
            if c.class.opcode != Op::Constant {
                return Err(
                    "uniform array length must be a plain constant (not a spec constant)".into(),
                );
            }
            let len = lit(c, 0)?;
            let el_align = round_up(el.align, 16)?;
            let stride = round_up(el.size, el_align)?;
            strides.push((id, stride));
            Ok(Layout {
                align: el_align,
                size: mul(stride, len)?,
                mat_stride: el.mat_stride,
            })
        }
        op => Err(format!(
            "unsupported uniform type {op:?} (bool/struct/etc. not handled)"
        )),
    }
}

fn find_or_create(g: &mut Vec<Instruction>, ids: &mut Ids, op: Op, ops: Vec<Operand>) -> Word {
    for i in g.iter() {
        if i.class.opcode == op && i.result_type.is_none() && i.operands == ops {
            if let Some(id) = i.result_id {
                return id;
            }
        }
    }
    let id = ids.next();
    g.push(Instruction::new(op, None, Some(id), ops));
    id
}

fn uniform_ptr(g: &mut Vec<Instruction>, ids: &mut Ids, pointee: Word) -> Word {
    find_or_create(
        g,
        ids,
        Op::TypePointer,
        vec![
            Operand::StorageClass(StorageClass::Uniform),
            Operand::IdRef(pointee),
        ],
    )
}

pub fn construct_ubo(words: &[u32], set: u32, binding: u32) -> Result<Packed, String> {
    let mut m = load_words(words).map_err(|e| format!("{e:?}"))?;
    let hdr = m.header.as_ref().ok_or("no header")?;
    let mut ids = Ids(hdr.bound);
    let (maj, min) = ((hdr.version >> 16) & 0xff, (hdr.version >> 8) & 0xff);

    let types: Types = m
        .types_global_values
        .iter()
        .filter_map(|i| i.result_id.map(|id| (id, i.clone())))
        .collect();

    struct Uni {
        var: Word,
        ty: Word,
        name: String,
        offset: u32,
        size: u32,
        mat_stride: Option<u32>,
    }
    let mut unis = Vec::<Uni>::new();
    let mut strides: Vec<(Word, u32)> = Vec::new();
    let mut cur = 0u32;

    for inst in &m.types_global_values {
        if inst.class.opcode != Op::Variable {
            continue;
        }
        if !matches!(
            inst.operands.first(),
            Some(Operand::StorageClass(StorageClass::UniformConstant))
        ) {
            continue;
        }
        let var = inst.result_id.ok_or("OpVariable without result id")?;
        let ptr_ty = inst.result_type.ok_or("OpVariable without result type")?;
        let pointee = ptr_pointee(&types, ptr_ty)?;

        // samplers/images stay as UniformConstant on purpose
        if is_opaque(&types, pointee) {
            continue;
        }

        let name_of_var = || {
            m.debug_names.iter().find_map(|n| {
                if n.class.opcode == Op::Name && idref(n, 0).ok() == Some(var) {
                    if let Some(Operand::LiteralString(s)) = n.operands.get(1) {
                        return Some(s.clone());
                    }
                }
                None
            })
        };
        let name = name_of_var().unwrap_or_else(|| format!("u{}", unis.len()));

        // collect strides locally; only commit if the uniform is accepted
        let mut local_strides = Vec::new();
        let l = std140(&types, pointee, &mut local_strides)
            .map_err(|e| format!("uniform `{name}`: {e}"))?;

        let offset = round_up(cur, l.align)?;
        cur = offset.checked_add(l.size).ok_or_else(ovf)?;
        strides.extend(local_strides);

        unis.push(Uni {
            var,
            ty: pointee,
            name,
            offset,
            size: l.size,
            mat_stride: l.mat_stride,
        });
    }
    if unis.is_empty() {
        return Ok(Packed {
            words: words.to_vec(),
            members: vec![],
            buffer_size: 0,
        });
    }
    let buffer_size = round_up(cur, 16)?;
    let member_of: HashMap<Word, u32> = unis
        .iter()
        .enumerate()
        .map(|(i, u)| (u.var, i as u32))
        .collect();

    // uses outside function blocks (OpSpecConstantOp, initializers, ...) can't be rewritten
    for inst in &m.types_global_values {
        if inst
            .result_id
            .map_or(false, |id| member_of.contains_key(&id))
        {
            continue; // the variable declaration itself
        }
        if inst
            .operands
            .iter()
            .any(|o| matches!(o, Operand::IdRef(id) if member_of.contains_key(id)))
        {
            return Err(format!(
                "unsupported use of a loose uniform in a global instruction ({:?})",
                inst.class.opcode
            ));
        }
    }

    // don't double-decorate array types that already carry ArrayStride
    let already_strided: HashSet<Word> = m
        .annotations
        .iter()
        .filter(|a| {
            a.class.opcode == Op::Decorate
                && matches!(
                    a.operands.get(1),
                    Some(Operand::Decoration(Decoration::ArrayStride))
                )
        })
        .filter_map(|a| idref(a, 0).ok())
        .collect();

    let (blk, blk_ptr, blk_var) = (ids.next(), ids.next(), ids.next());
    let g = &mut m.types_global_values;
    g.push(Instruction::new(
        Op::TypeStruct,
        None,
        Some(blk),
        unis.iter().map(|u| Operand::IdRef(u.ty)).collect(),
    ));
    g.push(Instruction::new(
        Op::TypePointer,
        None,
        Some(blk_ptr),
        vec![
            Operand::StorageClass(StorageClass::Uniform),
            Operand::IdRef(blk),
        ],
    ));
    g.push(Instruction::new(
        Op::Variable,
        Some(blk_ptr),
        Some(blk_var),
        vec![Operand::StorageClass(StorageClass::Uniform)],
    ));

    let dec = |t: Word, d: Decoration, v: Option<u32>| {
        let mut ops = vec![Operand::IdRef(t), Operand::Decoration(d)];
        if let Some(v) = v {
            ops.push(Operand::LiteralBit32(v));
        }
        Instruction::new(Op::Decorate, None, None, ops)
    };
    let mdec = |i: u32, d: Decoration, v: Option<u32>| {
        let mut ops = vec![
            Operand::IdRef(blk),
            Operand::LiteralBit32(i),
            Operand::Decoration(d),
        ];
        if let Some(v) = v {
            ops.push(Operand::LiteralBit32(v));
        }
        Instruction::new(Op::MemberDecorate, None, None, ops)
    };

    let mut annots = vec![
        dec(blk, Decoration::Block, None),
        dec(blk_var, Decoration::DescriptorSet, Some(set)),
        dec(blk_var, Decoration::Binding, Some(binding)),
    ];
    let mut seen = HashSet::new();
    for (arr, stride) in strides {
        if !already_strided.contains(&arr) && seen.insert(arr) {
            annots.push(dec(arr, Decoration::ArrayStride, Some(stride)));
        }
    }
    let mut names = vec![Instruction::new(
        Op::Name,
        None,
        None,
        vec![
            Operand::IdRef(blk),
            Operand::LiteralString("_Uniforms".into()),
        ],
    )];
    for (i, u) in unis.iter().enumerate() {
        let i = i as u32;
        annots.push(mdec(i, Decoration::Offset, Some(u.offset)));
        if let Some(ms) = u.mat_stride {
            annots.push(mdec(i, Decoration::ColMajor, None));
            annots.push(mdec(i, Decoration::MatrixStride, Some(ms)));
        }
        names.push(Instruction::new(
            Op::MemberName,
            None,
            None,
            vec![
                Operand::IdRef(blk),
                Operand::LiteralBit32(i),
                Operand::LiteralString(u.name.clone()),
            ],
        ));
    }

    let u32_ty = find_or_create(
        &mut m.types_global_values,
        &mut ids,
        Op::TypeInt,
        vec![Operand::LiteralBit32(32), Operand::LiteralBit32(0)],
    );
    let mut idx_const: HashMap<u32, Word> = HashMap::new();
    let mut uniform_ptrs: HashSet<Word> = HashSet::new();

    for f in &mut m.functions {
        for b in &mut f.blocks {
            let old = std::mem::take(&mut b.instructions);
            let mut out = Vec::with_capacity(old.len());
            for mut inst in old {
                let opcode = inst.class.opcode;
                for op in inst.operands.iter_mut() {
                    let Operand::IdRef(id) = op else { continue };
                    let Some(&mi) = member_of.get(id) else {
                        continue;
                    };
                    if !matches!(opcode, Op::Load | Op::AccessChain | Op::InBoundsAccessChain) {
                        return Err(format!(
                            "unsupported use of a loose uniform by {opcode:?} \
                             (only load/access-chain are rewritten)"
                        ));
                    }
                    let c = match idx_const.get(&mi) {
                        Some(&c) => c,
                        None => {
                            let cid = ids.next();
                            m.types_global_values.push(Instruction::new(
                                Op::Constant,
                                Some(u32_ty),
                                Some(cid),
                                vec![Operand::LiteralBit32(mi)],
                            ));
                            idx_const.insert(mi, cid);
                            cid
                        }
                    };
                    let pty =
                        uniform_ptr(&mut m.types_global_values, &mut ids, unis[mi as usize].ty);
                    let p = ids.next();
                    out.push(Instruction::new(
                        Op::AccessChain,
                        Some(pty),
                        Some(p),
                        vec![Operand::IdRef(blk_var), Operand::IdRef(c)],
                    ));
                    uniform_ptrs.insert(p);
                    *id = p;
                }
                if matches!(opcode, Op::AccessChain | Op::InBoundsAccessChain) {
                    let base = match inst.operands.first() {
                        Some(Operand::IdRef(b)) => Some(*b),
                        _ => None,
                    };
                    if base.map_or(false, |b| uniform_ptrs.contains(&b)) {
                        let rt = inst.result_type.ok_or("access chain without result type")?;
                        let pointee = ptr_pointee(&types, rt)?;
                        inst.result_type =
                            Some(uniform_ptr(&mut m.types_global_values, &mut ids, pointee));
                        uniform_ptrs
                            .insert(inst.result_id.ok_or("access chain without result id")?);
                    }
                }
                out.push(inst);
            }
            b.instructions = out;
        }
    }

    if (maj, min) >= (1, 4) {
        for ep in &mut m.entry_points {
            let mut used = false;
            ep.operands.retain(|o| match o {
                Operand::IdRef(id) if member_of.contains_key(id) => {
                    used = true;
                    false
                }
                _ => true,
            });
            if used {
                ep.operands.push(Operand::IdRef(blk_var));
            }
        }
    }

    m.types_global_values.retain(|i| {
        !(i.class.opcode == Op::Variable
            && i.result_id.map_or(false, |id| member_of.contains_key(&id)))
    });
    m.debug_names.retain(|i| {
        !(i.class.opcode == Op::Name
            && matches!(i.operands.first(), Some(Operand::IdRef(id)) if member_of.contains_key(id)))
    });
    m.annotations.retain(|i| {
        !(i.class.opcode == Op::Decorate
            && matches!(i.operands.first(), Some(Operand::IdRef(id)) if member_of.contains_key(id)))
    });
    m.annotations.extend(annots);
    m.debug_names.extend(names);
    m.header.as_mut().ok_or("no header")?.bound = ids.0;

    Ok(Packed {
        words: m.assemble(),
        members: unis
            .iter()
            .map(|u| Member {
                name: u.name.clone(),
                offset: u.offset,
                size: u.size,
            })
            .collect(),
        buffer_size,
    })
}
