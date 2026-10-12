# Albite

Albite is a shader compiler for [Lazurite](https://veka0.github.io/lazurite/), specialized for compiling Minecraft Bedrock shader projects. It takes GLSL source code, compiles it to SPIR-V, cross-compiles it to the selected backend, and stores the result in a small binary container.

```text
USAGE:
    albite -f <INPUT> --type <TYPE> -p <PROFILE> [OPTIONS]

EXAMPLE:
    albite -f shader.vert --type v -p 310_es
    albite -f shader.vert --type v -p 310_es -o shader.bin

REQUIRED:
    -f, --file <INPUT>          Input GLSL shader file
    -t, --type <TYPE>           v = vertex, f = fragment, c = compute
    -p, --profile <PROFILE>     Target profile: 310_es, msl, s_6_0, s_6_5

OPTIONS:
    -o, --output <OUTPUT>       Write the output to a file instead of stdout
    -i, --include <DIR>         Add an include search directory
    -d, --define <DEFINES>      NAME or NAME=VALUE; separate multiple with ';'

    -O0                         Disable optimization
    -O1                         Optimize for size
    -O2                         Optimize for performance (default)

    --verbose                   Print the cross-compiled shader to stderr
    -E, --preprocess-only       Only run the preprocessor and print the expanded
                                source
    --batch-preprocess          Serve many -E requests from one process: read each
                                request from stdin (one argument per line, ended by
                                an empty line) and answer 'OK <n>' / 'ERR <n>'
                                followed by <n> bytes. Avoids per-process start-up
    -h, --help                  Show this help
    -v, --version               Show version information
```

Profile names are case-insensitive. `msl`, `s_6_0` and `s_6_5` select the Metal and DXIL backends; **any other value falls back to GLSL ES 3.10**, so a mistyped profile is not rejected.

`-O0`, `-O1` and `-O2` control the GLSL -> SPIR-V optimization step. For DXIL output, the final HLSL -> DXIL step always runs DXC with `-O0`, regardless of this flag. With `--verbose`, DXIL output prints the HLSL source.

## Prebuilt Binaries

See the latest packages on the [GitHub Releases page](../../releases).

## GLSL

GLSL source code compiled by shaderc, each compile is doing validation (can't turned off), and using the OpenGL 4.5 target environment. Write normal GLSL with a `#version` line and a `main` entry point.

- Use plain (loose) uniforms. For `msl`, `s_6_0` and `s_6_5`, Albite packs them into one uniform buffer with std140 layout, samplers and storage images stay as separate resources and are not packed.
- Uniform bindings and input/output locations are assigned automatically, so explicit `binding` or `location` qualifiers are optional but recommended.
- For `310_es`, cube map arrays automatically enable `GL_EXT_texture_cube_map_array`.

### Vertex Inputs

Vertex input is restricted to what is defined by bgfx, which is used by the RenderDragon graphics engine. Any other name return error.

```text
a_position, a_normal, a_tangent, a_bitangent, a_indices, a_weight
a_color0 to a_color3
a_texcoord0 to a_texcoord8
i_data0 to i_data4 (per-instance data)
```

### Predefined Macros

These macros are defined before the shader is compiled. Each one is set to `1`.

| Macro | Defined when |
| --- | --- |
| `SHADER_STAGE__VERTEX` | `--type v` |
| `SHADER_STAGE__FRAGMENT` | `--type f` |
| `SHADER_STAGE__COMPUTE` | `--type c` |
| `TRANSPILE_TARGET__GLSL` | profile `310_es` |
| `TRANSPILE_TARGET__MSL` | profile `msl` |
| `TRANSPILE_TARGET__HLSL` | profile `s_6_0` or `s_6_5` |

Example:

```glsl
#if SHADER_STAGE__VERTEX
    // vertex only code
#endif

#if TRANSPILE_TARGET__MSL
    // Metal only code
#endif
```

## Container Format

Albite outputs a little-endian section-based binary container. The format is intentionally simple: a fixed header, a section table, then the raw section data.

### Layout

For the current container, there are always two sections: `META` and `CODE`.

```text
Byte offset

0x0000  +--------------------------------------------+
        | Magic: "ALBI"                   4 bytes    |
0x0004  +--------------------------------------------+
        | Version                         u16 LE     |
0x0006  +--------------------------------------------+
        | Section count                   u16 LE     |
0x0008  +--------------------------------------------+
        | tag                             4 bytes    |
        | data offset                     u32 LE     |
        | data size                       u32 LE     |
0x0014  +--------------------------------------------+
        | tag                             4 bytes    |
        | data offset                     u32 LE     |
        | data size                       u32 LE     |
0x0020  +--------------------------------------------+
        | META payload (UTF-8 JSON)                  |
        |                                            |
        | ...                                        |
        +--------------------------------------------+
        | CODE payload                               |
        |                                            |
        | ...                                        |
        +--------------------------------------------+
```

With two sections, the data area starts at `0x20` (32 bytes):

```text
[ Header: 8 bytes ]
[ META table entry: 12 bytes ]
[ CODE table entry: 12 bytes ]
[ META data ]
[ CODE data ]
```

There is no padding, alignment, compression, or extra footer added by the writer.

## Libraries

- [shaderc](https://crates.io/crates/shaderc): compiles GLSL to SPIR-V. It is built from source.
- [spirv-cross2](https://crates.io/crates/spirv-cross2): reflects SPIR-V and converts it to GLSL ES, MSL and HLSL.
- [rspirv](https://crates.io/crates/rspirv): reads and edits SPIR-V.
- [hassle-rs](https://crates.io/crates/hassle-rs): binds to DirectX Shader Compiler.

Also uses `serde`, `serde_json` and `thiserror`.

## AI / LLM Assistance

This project was developed with assistance from AI/LLM tools. All generated code and suggestions were 100% reviewed and refined by human.
