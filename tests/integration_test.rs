use pl0::codegen::CodeGenerator;
use pl0::lexer::Lexer;
use pl0::optimizer::{optimize, optimize_ast};
use pl0::parser::Parser;
use pl0::vm::{VM, VMState};
use std::fs;
use std::path::Path;

struct TestCase {
    filename: &'static str,
    input: Vec<i64>,
    expected_output: Vec<String>,
}

#[test]
fn test_all_testcases() {
    let test_cases = vec![
        TestCase {
            filename: "base1.txt",
            input: vec![5],
            expected_output: vec!["5".to_string(), "70".to_string(), "35".to_string()],
        },
        TestCase {
            filename: "call.txt",
            input: vec![3, 4],
            expected_output: vec!["4".to_string(), "3".to_string(), "12".to_string()],
        },
        TestCase {
            filename: "if-else.txt",
            input: vec![5],
            expected_output: vec!["9".to_string()],
        },
        TestCase {
            filename: "rucursion.txt",
            input: vec![5],
            expected_output: vec!["5".to_string(), "120".to_string()],
        },
        TestCase {
            filename: "scope.txt",
            input: vec![4],
            expected_output: vec!["600".to_string()],
        },
    ];

    let testcase_dir = Path::new("testcase");
    if !testcase_dir.exists() {
        eprintln!("testcase directory not found");
        return;
    }

    for test_case in test_cases {
        let path = testcase_dir.join(test_case.filename);
        println!("Running test: {}", test_case.filename);

        let mut content = fs::read_to_string(&path).expect("Failed to read file");
        // Ensure content ends with a dot if not present (PL/0 requirement)
        if !content.trim().ends_with('.') {
            content.push('.');
        }

        let lexer = Lexer::new(&content);
        let mut parser = Parser::new(lexer, false);

        // Catch panics during parsing
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let parse_result = parser.parse();
            if parse_result.is_err() || !parser.errors.is_empty() {
                panic!("Parsing failed: {:?}", parser.errors);
            }
            let mut program = parse_result.unwrap();

            optimize_ast(&mut program);

            let mut generator = CodeGenerator::new();
            let code = generator
                .generate(&program)
                .expect("Code generation failed");

            optimize(code)
        }));

        match result {
            Ok(code) => {
                let mut vm = VM::new(code);

                // VM pops from back, so we reverse the input to simulate a queue
                let mut input = test_case.input.clone();
                input.reverse();
                vm.input_queue = input;

                let mut steps = 0;
                while vm.state == VMState::Running && steps < 100000 {
                    vm.step();
                    steps += 1;
                }

                if let VMState::Error(e) = vm.state {
                    panic!("{} failed at runtime: {}", test_case.filename, e);
                }

                assert_eq!(
                    vm.output, test_case.expected_output,
                    "Output mismatch for {}",
                    test_case.filename
                );
                println!("{} passed.", test_case.filename);
            }
            Err(_) => {
                panic!("{} failed parsing.", test_case.filename);
            }
        }
    }
}
