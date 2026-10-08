use albite_core::options::*;
use std::path::PathBuf;
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_help();
        return ExitCode::SUCCESS;
    }

    if args.iter().any(|arg| arg == "-v" || arg == "--version") {
        print_version();
        return ExitCode::SUCCESS;
    }

    let options = match parse_args(&args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("error: {message}\n");
            eprintln!("Try 'albite --help' for more information\n");
            return ExitCode::FAILURE;
        }
    };

    match run(&options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(options: &Options) -> Result<(), String> {
    let source = std::fs::read_to_string(&options.input_file_path)
        .map_err(|e| format!("reading {}: {e}", options.input_file_path.display()))?;

    let profile_lower = options.profile.to_ascii_lowercase();
    let (bytes, source_text, backend_name) = match profile_lower.as_str() {
        "msl" => {
            let compiled = albite_msl::compile(options, &source).map_err(|e| e.to_string())?;
            (compiled.bytes, compiled.msl_source, "MSL")
        }
        "s_6_5" | "s_6_0" => {
            let compiled = albite_dxil::compile(options, &source).map_err(|e| e.to_string())?;
            (compiled.bytes, compiled.hlsl_source, "HLSL")
        }
        _ => {
            let compiled = albite_glsl::compile(options, &source).map_err(|e| e.to_string())?;
            (compiled.bytes, compiled.glsl_source, "GLSL")
        }
    };

    std::fs::write(&options.output_file_path, &bytes)
        .map_err(|e| format!("writing {}: {e}", options.output_file_path.display()))?;

    if options.verbose {
        let shader_type = options.shader_type;
        println!(
            "------ wrote {} bytes to {}",
            bytes.len(),
            options.output_file_path.display()
        );
        println!("------ {backend_name} (stage: {shader_type:?})");
        println!("{source_text}");
    }

    Ok(())
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::new();
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();

        match arg {
            "-f" | "--file" => {
                options.input_file_path = PathBuf::from(next_value(args, &mut index, arg)?);
            }
            "-o" | "--output" => {
                options.output_file_path = PathBuf::from(next_value(args, &mut index, arg)?);
            }
            "-t" | "--type" => {
                let c = next_value(args, &mut index, arg)?
                    .chars()
                    .next()
                    .ok_or("shader type is required (-t <TYPE> or --type <TYPE>)")?;
                options.shader_type = ShaderType::from_char(c)
                    .ok_or_else(|| format!("invalid shader type '{c}'; expected v, f, or c"))?;
            }
            "-p" | "--profile" => {
                options.profile = next_value(args, &mut index, arg)?;
            }
            "-i" | "--include" => {
                options
                    .include_dirs
                    .push(PathBuf::from(next_value(args, &mut index, arg)?));
            }
            "-d" | "--define" => {
                parse_defines(&mut options.defines, &next_value(args, &mut index, arg)?);
            }
            "-O0" | "-O1" | "-O2" => {
                options.optimization =
                    OptimizationLevel::from_flag(arg).expect("only accepted -O0, -O1, -O2 flag");
            }
            "--verbose" => {
                options.verbose = true;
            }
            other => return Err(format!("unrecognized argument: {other}")),
        }

        index += 1;
    }

    if options.input_file_path.as_os_str().is_empty() {
        return Err("input file is required (-f <INPUT>)".into());
    }
    if options.output_file_path.as_os_str().is_empty() {
        return Err("output file is required (-o <INPUT>)".into());
    }
    if options.profile.is_empty() {
        return Err("profile is required (-p <PROFILE> or --profile <PROFILE>)".into());
    }

    Ok(options)
}

fn next_value(args: &[String], index: &mut usize, option: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("{option} expects a value"))
}

fn parse_defines(defines: &mut Vec<(String, Option<String>)>, value: &str) {
    for def in value.split(';').map(str::trim).filter(|v| !v.is_empty()) {
        match def.split_once('=') {
            Some((name, value)) => defines.push((name.to_string(), Some(value.to_string()))),
            None => defines.push((def.to_string(), Some("1".to_string()))),
        }
    }
}

fn print_help() {
    println!(
        "\
USAGE:
    albite -f <INPUT> -o <OUTPUT> --type <TYPE> -p <PROFILE> [OPTIONS]

EXAMPLE:
    albite -f shader.vert -o shader.bin --type v -p 310_es

REQUIRED:
    -f, --file <INPUT>          Input GLSL shader file
    -o, --output <OUTPUT>       Output binary
    -t, --type <TYPE>           v = vertex, f = fragment, c = compute
    -p, --profile <PROFILE>     Target profile: 310_es, msl, s_6_0, s_6_5

OPTIONS:
    -i, --include <DIR>         Add an include search directory
    -d, --define <DEFINES>      NAME or NAME=VALUE; separate multiple with ';'

    -O0                         Disable optimization
    -O1                         Optimize for size
    -O2                         Optimize for performance (default)

    --verbose                   Print the cross-compiled shader to stdout
    -h, --help                  Show this help
    -v, --version               Show version information
"
    );
}

fn print_version() {
    println!("albite {VERSION}\nYet another shader compiler for Lazurite");
}
