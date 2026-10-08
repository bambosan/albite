#[inline]
fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

#[inline]
fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

fn take_ident(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    let mut first = true;
    let mut end = 0;
    for (i, c) in s.char_indices() {
        if first {
            if !is_ident_start(c) {
                return None;
            }
            first = false;
        } else if !is_ident_continue(c) {
            break;
        }
        end = i + c.len_utf8();
    }
    if end == 0 {
        None
    } else {
        Some((&s[..end], &s[end..]))
    }
}

fn find_matching_paren(s: &str) -> Option<usize> {
    let mut depth = 1i32;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn take_parens(s: &str) -> Option<(&str, &str)> {
    let inner = s.trim_start().strip_prefix('(')?;
    let close = find_matching_paren(inner)?;
    Some((&inner[..close], &inner[close + 1..]))
}

fn is_whole_word_at(src: &str, start: usize, len: usize) -> bool {
    let before = src[..start].chars().next_back();
    let after = src[start + len..].chars().next();
    !before.map_or(false, is_ident_continue) && !after.map_or(false, is_ident_continue)
}

/// splits at the LAST occurrence of ch outside any parentheses
fn split_top(s: &str, ch: char) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    let mut last = None;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c == ch && depth == 0 => last = Some(i),
            _ => {}
        }
    }
    let i = last?;
    Some((&s[..i], &s[i + ch.len_utf8()..]))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub base: &'static str, // scalar type: "float" | "int" | "uint"
    pub count: usize,       // 1..=4
    pub offset: usize,      // byte offset taken from the SPIR-V layout
}

impl Field {
    fn size(&self) -> usize {
        self.count * 4
    }

    fn ty(&self) -> String {
        if self.count == 1 {
            self.base.to_string()
        } else {
            format!("{}{}", self.base, self.count)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLayout {
    fields: Vec<Field>,
    stride: usize,
}

impl StructLayout {
    pub fn new(fields: Vec<Field>, array_stride: usize) -> Result<Self, String> {
        let mut tight = 0;
        for f in &fields {
            if f.offset != tight {
                return Err(format!(
                    "member `{}` is at byte {} but a StructuredBuffer packs it at {tight}; \
                     reorder the members to avoid padding",
                    f.name, f.offset
                ));
            }
            tight += f.size();
        }
        if tight == 0 {
            return Err("storage buffer element struct has no members".into());
        }
        if array_stride != tight {
            return Err(format!(
                "array stride {array_stride} != packed struct size {tight}"
            ));
        }
        Ok(Self {
            fields,
            stride: tight,
        })
    }
}

enum Op {
    Load,
    Store,
}

struct Access<'a> {
    op: Op,
    ty: &'a str,
    args: &'a str,
    after: &'a str,
    untyped: bool, // `.Load(..)` / `.Store(..)`: raw uint
}

fn parse_access<'a>(rest: &'a str, buffer: &str) -> Result<Option<Access<'a>>, String> {
    let Some(rest) = rest.trim_start().strip_prefix('.') else {
        return Ok(None);
    };
    let Some((method, rest)) = take_ident(rest) else {
        return Ok(None);
    };
    let op = match method {
        "Load" => Op::Load,
        "Store" => Op::Store,
        _ => return Err(format!("unsupported method `{method}` on `{buffer}`")),
    };
    let t = rest.trim_start();
    let (ty, rest, untyped) = if let Some(t) = t.strip_prefix('<') {
        let e = t.find('>').ok_or("unterminated template argument")?;
        (t[..e].trim(), &t[e + 1..], false)
    } else {
        ("uint", rest, true)
    };
    let (args, after) = take_parens(rest).ok_or("malformed call")?;
    Ok(Some(Access {
        op,
        ty,
        args,
        after,
        untyped,
    }))
}

fn field_access(
    fields: &[Field],
    off: usize,
    ty: &str,
    untyped: bool,
) -> Result<(String, &'static str), String> {
    let f = fields
        .iter()
        .find(|f| off >= f.offset && off < f.offset + f.size())
        .ok_or_else(|| format!("offset {off} outside struct"))?;
    let path = if off == f.offset && (ty == f.ty() || (untyped && f.count == 1)) {
        f.name.clone()
    } else if f.count > 1 && (untyped || ty == f.base) && (off - f.offset) % 4 == 0 {
        format!("{}.{}", f.name, ['x', 'y', 'z', 'w'][(off - f.offset) / 4])
    } else {
        return Err(format!(
            "access <{ty}> at offset {off} doesn't match field `{}`",
            f.name
        ));
    };
    Ok((path, f.base))
}

/// Parses "<idx> * <stride> [+ <offset>]" and checks the stride. Returns (idx, offset).
fn parse_addr(addr: &str, stride: usize) -> Result<(&str, usize), String> {
    let (base, off) = match split_top(addr, '+') {
        Some((b, o)) => (
            b,
            o.trim()
                .parse::<usize>()
                .map_err(|_| format!("non-constant offset in `{addr}`"))?,
        ),
        None => (addr, 0),
    };
    let (idx, s) =
        split_top(base, '*').ok_or_else(|| format!("expected `idx * stride` in `{addr}`"))?;
    let s: usize = s
        .trim()
        .parse()
        .map_err(|_| format!("non-constant stride in `{addr}`"))?;
    if s != stride {
        return Err(format!("stride {s} != struct size {stride} in `{addr}`"));
    }
    Ok((idx.trim(), off))
}

/// Replaces `<from> <buffer>` with `<to><struct_name> <buffer>` (whole words only).
fn replace_decl(
    src: &str,
    from: &str,
    to: &str,
    buffer: &str,
    struct_name: &str,
) -> (String, bool) {
    let mut out = String::with_capacity(src.len());
    let mut pos = 0;
    let mut found = false;
    while let Some(p) = src[pos..].find(from) {
        let at = pos + p;
        let end = at + from.len();
        out.push_str(&src[pos..at]);
        let is_target = is_whole_word_at(src, at, from.len())
            && matches!(take_ident(&src[end..]), Some((n, _)) if n == buffer);
        if is_target {
            out.push_str(&format!("{to}<{struct_name}>"));
            found = true;
        } else {
            out.push_str(from);
        }
        pos = end;
    }
    out.push_str(&src[pos..]);
    (out, found)
}

/// Rewrites `(RW)ByteAddressBuffer <buffer>` into `(RW)StructuredBuffer<struct_name>`,
/// `<buffer>.Load[<T>](idx * stride + off)` into `<buffer>[idx].field`, and
/// `<buffer>.Store[<T>](idx * stride + off, v)` into `<buffer>[idx].field = v`.
pub fn byte_address_to_structured(
    hlsl: &str,
    buffer: &str,
    struct_name: &str,
    layout: &StructLayout,
) -> Result<String, String> {
    let fields = &layout.fields;
    let stride = layout.stride;

    // declaration (the whole-word check keeps the two keywords from matching each other)
    let (decl, ro) = replace_decl(
        hlsl,
        "ByteAddressBuffer",
        "StructuredBuffer",
        buffer,
        struct_name,
    );
    let (decl, rw) = replace_decl(
        &decl,
        "RWByteAddressBuffer",
        "RWStructuredBuffer",
        buffer,
        struct_name,
    );
    if !(ro || rw) {
        return Err(format!(
            "`(RW)ByteAddressBuffer {buffer}` declaration not found"
        ));
    }

    // accesses
    let mut out = String::with_capacity(decl.len());
    let mut pos = 0;
    while let Some(p) = decl[pos..].find(buffer) {
        let at = pos + p;
        let end = at + buffer.len();
        out.push_str(&decl[pos..at]);

        let access = if is_whole_word_at(&decl, at, buffer.len()) {
            parse_access(&decl[end..], buffer)?
        } else {
            None
        };
        let Some(a) = access else {
            out.push_str(buffer);
            pos = end;
            continue;
        };

        match a.op {
            Op::Load => {
                let (idx, off) = parse_addr(a.args, stride)?;
                let (path, _) = field_access(fields, off, a.ty, a.untyped)?;
                out.push_str(&format!("{buffer}[{idx}].{path}"));
            }
            Op::Store => {
                // args = "<addr>, <value>"; the last top-level comma is the separator
                let (addr, value) = split_top(a.args, ',')
                    .ok_or_else(|| format!("expected `addr, value` in `{}`", a.args))?;
                let (idx, off) = parse_addr(addr, stride)?;
                let (path, base) = field_access(fields, off, a.ty, a.untyped)?;
                let value = value.trim();
                // untemplated stores carry a uint bit pattern: convert to the field type
                let rhs = match (a.untyped, base) {
                    (true, "float") => format!("asfloat({value})"),
                    (true, "int") => format!("asint({value})"),
                    _ => value.to_string(),
                };
                out.push_str(&format!("{buffer}[{idx}].{path} = {rhs}"));
            }
        }
        pos = decl.len() - a.after.len();
    }
    out.push_str(&decl[pos..]);
    Ok(out)
}
