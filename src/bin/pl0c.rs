use pl0::lexer::Lexer;
use pl0::parser::Parser;
use pl0::vm::VM;
use std::env;
use std::fs::{self, File};
use std::io::Write;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <source_file> [output_file]", args[0]);
        std::process::exit(1);
    }

    let source_path = &args[1];
    let output_path = if args.len() >= 3 { &args[2] } else { "out.asm" };

    let source_code = fs::read_to_string(source_path).expect("Failed to read source file");

    let lexer = Lexer::new(&source_code);
    let mut parser = Parser::new(lexer);

    println!("Compiling {}...", source_path);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        parser.parse();
    }));

    if result.is_err() {
        eprintln!("Compilation failed.");
        std::process::exit(1);
    }

    println!(
        "Compilation successful! Generated {} instructions.",
        parser.generator.code.len()
    );

    let mut file = File::create(output_path).expect("Failed to create output file");
    for instr in &parser.generator.code {
        writeln!(file, "{:?} {} {}", instr.f, instr.l, instr.a)
            .expect("Failed to write instruction");
    }

    println!("Wrote assembly to {}", output_path);

    println!("Running {}...", source_path);
    let mut vm = VM::new(parser.generator.code);
    vm.interpret();
}
