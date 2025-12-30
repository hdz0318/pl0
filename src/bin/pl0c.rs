use pl0::codegen::CodeGenerator;
use pl0::lexer::Lexer;
use pl0::optimizer::{optimize, optimize_ast};
use pl0::parser::Parser;
use pl0::vm::VM;
use std::env;
use std::fs::{self, File};
use std::io::Write;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut verbose = false;
    let mut positional_args = Vec::new();

    for arg in args.iter().skip(1) {
        if arg == "--verbose" || arg == "-v" {
            verbose = true;
        } else {
            positional_args.push(arg);
        }
    }

    if positional_args.is_empty() {
        eprintln!("Usage: {} <source_file> [output_file] [--verbose]", args[0]);
        std::process::exit(1);
    }

    let source_path = positional_args[0];
    let output_path = if positional_args.len() >= 2 {
        positional_args[1]
    } else {
        "out.asm"
    };

    let source_code = fs::read_to_string(source_path).expect("Failed to read source file");

    let lexer = Lexer::new(&source_code);
    let mut parser = Parser::new(lexer, verbose);

    println!("Compiling {}...", source_path);

    // Parse to AST
    let parse_result = parser.parse();

    if parse_result.is_err() || !parser.errors.is_empty() {
        eprintln!("Compilation failed.");
        let lines: Vec<&str> = source_code.lines().collect();
        for err in &parser.errors {
            eprintln!(
                "{}:{}:{}: error: {}",
                source_path, err.line, err.col, err.message
            );
            if err.line > 0 && err.line <= lines.len() {
                let line_content = lines[err.line - 1];
                eprintln!("    {}", line_content);
                let indent: String = line_content
                    .chars()
                    .take(err.col - 1)
                    .map(|c| if c.is_whitespace() { c } else { ' ' })
                    .collect();
                eprintln!("    {}^", indent);
            }
        }
        std::process::exit(1);
    }

    let mut program = parse_result.unwrap();

    println!("Optimizing AST...");
    optimize_ast(&mut program);

    println!("Generating Code...");
    let mut generator = CodeGenerator::new();
    let code = match generator.generate(&program) {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("Code generation failed: {}", msg);
            std::process::exit(1);
        }
    };

    println!(
        "Compilation successful! Generated {} instructions.",
        code.len()
    );

    println!("Optimizing Bytecode...");
    let optimized_code = optimize(code);
    println!(
        "Optimization successful! Reduced to {} instructions.",
        optimized_code.len()
    );

    let mut file = File::create(output_path).expect("Failed to create output file");
    for instr in &optimized_code {
        writeln!(file, "{:?} {} {}", instr.f, instr.l, instr.a)
            .expect("Failed to write instruction");
    }

    println!("Wrote assembly to {}", output_path);

    println!("Running {}...", source_path);
    let mut vm = VM::new(optimized_code);
    vm.interpret();
}
