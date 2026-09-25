mod logging;
mod util;

use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{self, Write};
use std::mem::replace;
use std::path::{Path, PathBuf};
use std::time::Instant;
use clap::Parser;
use log::{error, info};
use walkdir::WalkDir;
use f750c::{bytecode, CompilerInput, CompileResult};

#[derive(Debug, Clone, Parser)]
#[command(author, version, name = "f750c")]
struct Cli {
    input_path: PathBuf,

    #[arg(short, long = "output-name")]
    output_name: Option<String>,

    #[arg(short = 'O', long = "output-directory")]
    output_directory: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    release: bool,
}

fn main() {
    println!("f750c, the Fight750 State Script compiler");
    println!("Copyright (c) 2026, KuromeSama6");
    println!("Bytecode Version {:X}", bytecode::F750_VERSION);

    logging::init();
    let cli = Cli::parse();
    let start_time = Instant::now();

    info!("Input path: {:?}", cli.input_path);

    let input = match read_compiler_input(&cli.input_path) {
        Ok(input) => input,
        Err(err) => {
            error!("Error reading compiler input: {err}");
            return;
        }
    };

    let sw = Instant::now();
    let output = match f750c::compile_source(input) {
        Ok(output) => output,
        Err(err) => {
            error!("{err}");
            return;
        }
    };

    info!("Compilation completed in {:.3?} seconds.", sw.elapsed().as_secs_f64());

    // process output
    let output_file_name = cli.output_name.unwrap_or(match cli.input_path.file_prefix() {
        Some(name) => name.to_string_lossy().to_string(),
        None => {
            error!("Could not determine output file name from input path");
            return;
        }
    });

    info!("Output file name: {}", output_file_name);

    // write binary
    let bin_path = PathBuf::from(format!("{output_file_name}.f750b"));
    if let Err(e) = fs::write(&bin_path, output.bytes.bytes_ref()) {
        error!("Error writing output file: {e}");
        return;
    }

    info!("Bin: {}", bin_path.display());

    if !cli.release {
        // write semantic debug
        {
            let path = PathBuf::from(format!("{output_file_name}.semantic.txt"));
            let mut file = match File::create(&path) {
                Ok(file) => file,
                Err(e) => {
                    error!("Error creating semantic debug file: {e}");
                    return;
                }
            };

            for line in &output.semantic_lines {
                if let Err(e) = writeln!(file, "{}", line) {
                    error!("Error writing to semantic debug file: {e}");
                    return;
                }
            }

            info!("Semantic debug: {}", path.display());
        }

        // write semantic debug (expanded)
        {
            let path = PathBuf::from(format!("{output_file_name}.semantic_expanded.txt"));
            let mut file = match File::create(&path) {
                Ok(file) => file,
                Err(e) => {
                    error!("Error creating semantic debug (expanded) file: {e}");
                    return;
                }
            };

            for line in &output.semantic_lines_expanded {
                if let Err(e) = writeln!(file, "{}", line) {
                    error!("Error writing to semantic debug (expanded) file: {e}");
                    return;
                }
            }

            info!("Semantic debug (expanded): {}", path.display());
        }
    }

    info!("Compile Successful ({:.3?}s)", start_time.elapsed().as_secs_f64());
}

fn read_compiler_input(path: &PathBuf) -> CompileResult<CompilerInput> {
    let mut map = HashMap::new();
    let module_name = path.file_stem().unwrap()
        .to_string_lossy()
        .to_string();

    if !path.is_dir() {
        let content: Vec<String> = fs::read_to_string(path)?
            .lines()
            .map(|c| c.to_string())
            .collect();
        map.insert(module_name.clone(), content);

        return Ok(CompilerInput {
            sources: map,
            module_name,
        });
    }

    for entry in WalkDir::new(path) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                error!("Error reading directory entry: {e}");
                continue;
            }
        };

        if !entry.file_type().is_file() {
            continue;
        }
        let entry_path = entry.path();
        let Some(relative) = util::get_relative_path(path, entry_path) else {
            error!("Error getting relative path for entry: {:?}", entry_path);
            continue;
        };

        let Some(extension) = relative.extension()
            .map(|s| s.to_string_lossy().to_string()) else {
            continue;
        };

        if extension != "f750" {
            continue;
        }

        let relative = relative
            .to_string_lossy()
            .to_string()
            .replace(&format!(".{}", extension), "")
            .replace("\\", ".");

        let relative_path_str = format!("{}.{}", module_name, relative);
        let content: Vec<String> = fs::read_to_string(entry_path)?
            .lines()
            .map(|c| c.to_string())
            .collect();

        map.insert(relative_path_str, content);
    }

    Ok(CompilerInput {
        sources: map,
        module_name,
    })
}