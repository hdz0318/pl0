use crate::types::{Instruction, OpCode, Operator};
use std::io::{self, Write};

#[derive(PartialEq, Debug, Clone)]
pub enum VMState {
    Running,
    Halted,
    WaitingForInput,
    Error(String),
}

pub struct VM {
    pub code: Vec<Instruction>, // CODE: Stores P-code
    pub stack: Vec<i64>,        // STACK: Dynamic data space
    pub p: usize,               // P: Program address register (PC)
    pub b: usize,               // B: Base address register (BP)
    pub t: usize,               // T: Top of stack register (SP)
    pub i: Instruction,         // I: Instruction register
    pub output: Vec<String>,
    pub input_queue: Vec<i64>,
    pub state: VMState,
    pub instruction_count: usize,
}

impl VM {
    pub fn new(code: Vec<Instruction>) -> Self {
        Self {
            code,
            stack: vec![0; 1000], // Initial stack size
            p: 0,
            b: 0,
            t: 0,
            i: Instruction::new(OpCode::LIT, 0, 0), // Initial dummy instruction
            output: Vec::new(),
            input_queue: Vec::new(),
            state: VMState::Running,
            instruction_count: 0,
        }
    }

    fn base(&self, mut l: usize) -> usize {
        let mut b = self.b;
        while l > 0 {
            b = self.stack[b] as usize;
            l -= 1;
        }
        b
    }

    pub fn step(&mut self) {
        if self.state != VMState::Running {
            return;
        }

        if self.p >= self.code.len() {
            self.state = VMState::Error("PC out of bounds".to_string());
            return;
        }

        // Fetch instruction into I register
        self.i = self.code[self.p];
        self.p += 1;
        self.instruction_count += 1;

        let ir = self.i; // Use local alias for convenience matching original code structure

        match ir.f {
            OpCode::LIT => {
                self.stack[self.t] = ir.a;
                self.t += 1;
            }
            OpCode::OPR => {
                match Operator::from_i64(ir.a) {
                    Some(Operator::RET) => {
                        // RET
                        self.t = self.b;
                        self.p = self.stack[self.t + 2] as usize;
                        self.b = self.stack[self.t + 1] as usize;
                        if self.p == 0 {
                            self.state = VMState::Halted;
                        }
                    }
                    Some(Operator::NEG) => {
                        // NEG
                        self.stack[self.t - 1] = -self.stack[self.t - 1];
                    }
                    Some(Operator::ADD) => {
                        // ADD
                        self.t -= 1;
                        self.stack[self.t - 1] += self.stack[self.t];
                    }
                    Some(Operator::SUB) => {
                        // SUB
                        self.t -= 1;
                        self.stack[self.t - 1] -= self.stack[self.t];
                    }
                    Some(Operator::MUL) => {
                        // MUL
                        self.t -= 1;
                        self.stack[self.t - 1] *= self.stack[self.t];
                    }
                    Some(Operator::DIV) => {
                        // DIV
                        self.t -= 1;
                        if self.stack[self.t] == 0 {
                            self.state = VMState::Error("Division by zero".to_string());
                            return;
                        }
                        self.stack[self.t - 1] /= self.stack[self.t];
                    }
                    Some(Operator::ODD) => {
                        // ODD
                        self.stack[self.t - 1] %= 2;
                    }
                    Some(Operator::EQL) => {
                        // EQL
                        self.t -= 1;
                        self.stack[self.t - 1] = if self.stack[self.t - 1] == self.stack[self.t] {
                            1
                        } else {
                            0
                        };
                    }
                    Some(Operator::NEQ) => {
                        // NEQ
                        self.t -= 1;
                        self.stack[self.t - 1] = if self.stack[self.t - 1] != self.stack[self.t] {
                            1
                        } else {
                            0
                        };
                    }
                    Some(Operator::LSS) => {
                        // LSS
                        self.t -= 1;
                        self.stack[self.t - 1] = if self.stack[self.t - 1] < self.stack[self.t] {
                            1
                        } else {
                            0
                        };
                    }
                    Some(Operator::GEQ) => {
                        // GEQ
                        self.t -= 1;
                        self.stack[self.t - 1] = if self.stack[self.t - 1] >= self.stack[self.t] {
                            1
                        } else {
                            0
                        };
                    }
                    Some(Operator::GTR) => {
                        // GTR
                        self.t -= 1;
                        self.stack[self.t - 1] = if self.stack[self.t - 1] > self.stack[self.t] {
                            1
                        } else {
                            0
                        };
                    }
                    Some(Operator::LEQ) => {
                        // LEQ
                        self.t -= 1;
                        self.stack[self.t - 1] = if self.stack[self.t - 1] <= self.stack[self.t] {
                            1
                        } else {
                            0
                        };
                    }
                    Some(Operator::WRT) => {
                        // Write stack top
                        self.t -= 1;
                        let val = self.stack[self.t];
                        self.output.push(val.to_string());
                    }
                    Some(Operator::WRL) => {
                        // Write newline
                        self.output.push("\n".to_string());
                    }
                    Some(Operator::RED) => {
                        // Read to stack top
                        if let Some(val) = self.input_queue.pop() {
                            self.stack[self.t] = val;
                            self.t += 1;
                        } else {
                            self.p -= 1;
                            self.state = VMState::WaitingForInput;
                        }
                    }
                    None => {
                        self.state = VMState::Error(format!("Unknown OPR {}", ir.a));
                    }
                }
            }
            OpCode::LOD => {
                let base = self.base(ir.l);
                let addr = (base as i64 + ir.a) as usize;
                self.stack[self.t] = self.stack[addr];
                self.t += 1;
            }
            OpCode::STO => {
                let base = self.base(ir.l);
                let addr = (base as i64 + ir.a) as usize;
                self.t -= 1;
                self.stack[addr] = self.stack[self.t];
            }
            OpCode::CAL => {
                let base = self.base(ir.l);
                self.stack[self.t] = base as i64; // Static Link (SL)
                self.stack[self.t + 1] = self.b as i64; // Dynamic Link (DL)
                self.stack[self.t + 2] = self.p as i64; // Return Address (RA)
                self.b = self.t;
                self.p = ir.a as usize;
            }
            OpCode::INT => {
                self.t = (self.t as i64 + ir.a) as usize;
            }
            OpCode::JMP => {
                self.p = ir.a as usize;
            }
            OpCode::JPC => {
                self.t -= 1;
                if self.stack[self.t] == 0 {
                    self.p = ir.a as usize;
                }
            }
            OpCode::RED => {
                if let Some(val) = self.input_queue.pop() {
                    let base = self.base(ir.l);
                    let addr = (base as i64 + ir.a) as usize;
                    self.stack[addr] = val;
                } else {
                    // Push back PC to retry this instruction when input is available
                    self.p -= 1;
                    self.state = VMState::WaitingForInput;
                }
            }
            OpCode::WRT => {
                self.t -= 1; // Pop
                let val = self.stack[self.t];
                self.output.push(val.to_string());
            }
        }
    }

    pub fn interpret(&mut self) {
        println!("Start PL/0");
        self.p = 0;
        self.b = 0;
        self.t = 0;
        self.state = VMState::Running;
        let mut output_index = 0;

        loop {
            match self.state {
                VMState::Running => {
                    self.step();
                    while output_index < self.output.len() {
                        println!("{}", self.output[output_index]);
                        output_index += 1;
                    }
                }
                VMState::Halted => {
                    println!("Program finished");
                    break;
                }
                VMState::Error(ref e) => {
                    println!("Runtime Error: {}", e);
                    break;
                }
                VMState::WaitingForInput => {
                    print!("Input: ");
                    io::stdout().flush().unwrap();
                    let mut input = String::new();
                    io::stdin().read_line(&mut input).unwrap();
                    if let Ok(val) = input.trim().parse::<i64>() {
                        self.input_queue.push(val);
                        self.state = VMState::Running;
                    } else {
                        println!("Invalid input");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Instruction, OpCode};

    #[test]
    fn test_vm_arithmetic() {
        // 10 + 20
        let code = vec![
            Instruction::new(OpCode::LIT, 0, 10),
            Instruction::new(OpCode::LIT, 0, 20),
            Instruction::new(OpCode::OPR, 0, 2), // ADD
        ];
        let mut vm = VM::new(code);

        // Step through instructions
        vm.step(); // LIT 10
        vm.step(); // LIT 20
        vm.step(); // ADD

        assert_eq!(vm.stack[vm.t - 1], 30);
    }
}
