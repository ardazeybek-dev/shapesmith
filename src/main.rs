use std::io::{self, Read};
use std::process::ExitCode;

use shapesmith::{Options, generate_from_values, input::parse_samples};

const USAGE: &str = "\
Infer TypeScript types, Zod schemas and JSON Schema from JSON samples.

Usage: shapesmith [OPTIONS] [FILE]...

Each FILE (or stdin when none is given) holds one JSON document, a JSON array
of samples, or NDJSON. Samples from every file are combined.

Options:
  -t, --target <TARGET>  ts, zod, schema or all [default: ts]
  -n, --name <NAME>      name of the top-level type [default: Root]
      --no-formats       do not detect date-time, date, email, uuid, url
      --no-enums         do not turn repeating strings into literal unions
      --max-enum <N>     most distinct values an enum may have [default: 8]
      --no-split         read a top-level array as one sample
  -h, --help             print this help
  -V, --version          print the version";

#[derive(PartialEq)]
enum Target {
    Ts,
    Zod,
    Schema,
    All,
}

struct Args {
    target: Target,
    opts: Options,
    files: Vec<String>,
}

fn parse_args(raw: impl IntoIterator<Item = String>) -> Result<Option<Args>, String> {
    let mut args = Args { target: Target::Ts, opts: Options::default(), files: Vec::new() };
    let mut raw = raw.into_iter();
    while let Some(arg) = raw.next() {
        let mut value = |flag: &str| raw.next().ok_or_else(|| format!("{flag} needs a value"));
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("shapesmith {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "-t" | "--target" => {
                args.target = match value(&arg)?.as_str() {
                    "ts" | "typescript" => Target::Ts,
                    "zod" => Target::Zod,
                    "schema" | "json-schema" => Target::Schema,
                    "all" => Target::All,
                    other => {
                        return Err(format!("unknown target {other:?} (expected ts, zod, schema or all)"));
                    }
                }
            }
            "-n" | "--name" => args.opts.root_name = value(&arg)?,
            "--no-formats" => args.opts.detect_formats = false,
            "--no-enums" => args.opts.detect_enums = false,
            "--no-split" => args.opts.split_top_level_arrays = false,
            "--max-enum" => {
                let n = value(&arg)?;
                args.opts.max_enum_values = n
                    .parse()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or(format!("--max-enum expects a positive number, got {n:?}"))?;
            }
            flag if flag.starts_with('-') && flag != "-" => return Err(format!("unknown option {flag}")),
            file => args.files.push(file.to_owned()),
        }
    }
    Ok(Some(args))
}

fn read_samples(args: &Args) -> Result<Vec<serde_json::Value>, String> {
    let split = args.opts.split_top_level_arrays;
    if args.files.is_empty() || args.files == ["-"] {
        let mut text = String::new();
        io::stdin().read_to_string(&mut text).map_err(|e| format!("stdin: {e}"))?;
        return parse_samples(&text, split).map_err(|e| format!("stdin: {e}"));
    }
    let mut samples = Vec::new();
    for file in &args.files {
        let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
        samples.extend(parse_samples(&text, split).map_err(|e| format!("{file}: {e}"))?);
    }
    Ok(samples)
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(Some(args)) => args,
        Ok(None) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}\n\nRun `shapesmith --help` for usage.");
            return ExitCode::from(2);
        }
    };
    let samples = match read_samples(&args) {
        Ok(samples) => samples,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let out = generate_from_values(&samples, &args.opts);
    match args.target {
        Target::Ts => print!("{}", out.typescript),
        Target::Zod => print!("{}", out.zod),
        Target::Schema => print!("{}", out.json_schema),
        Target::All => {
            print!("// ---- TypeScript ----\n{}\n", out.typescript);
            print!("// ---- Zod ----\n{}\n", out.zod);
            print!("// ---- JSON Schema ----\n{}", out.json_schema);
        }
    }
    ExitCode::SUCCESS
}
