use crate::types::{Instruction, OpCode};
use std::io::{self, Write};

#[derive(PartialEq, Debug, Clone)]
pub enum VMState {
    Running,
    Halted,
    WaitingForInput,
    Error(String),
}

pub struct VM {
    pub code: Vec<Instruction>,
    pub stack: Vec<i64>,
    pub pc: usize,
    pub bp: usize,
    pub sp: usize,
    pub output: Vec<String>,
    pub input_queue: Vec<i64>,
    pub state: VMState,
}

impl VM {
    pub fn new(code: Vec<Instruction>) -> Self {
        Self {
            code,
            stack: vec![0; 1000], // Initial stack size
            pc: 0,
            bp: 0,
            sp: 0,
            output: Vec::new(),
            input_queue: Vec::new(),
            state: VMState::Running,
        }
    }

    fn base(&self, mut l: usize) -> usize {
        let mut b = self.bp;
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

        if self.pc >= self.code.len() {
            self.state = VMState::Error("PC out of bounds".to_string());
            return;
        }

        let ir = self.code[self.pc];
        self.pc += 1;

        match ir.f {
            OpCode::LIT => {
                self.stack[self.sp] = ir.a;
                self.sp += 1;
            }
            OpCode::OPR => {
                match ir.a {
                    0 => {
                        // RET
                        self.sp = self.bp;
                        self.pc = self.stack[self.sp + 2] as usize;
                        self.bp = self.stack[self.sp + 1] as usize;
                        if self.pc == 0 {
                            self.state = VMState::Halted;
                        }
                    }
                    1 => {
                        // NEG
                        self.stack[self.sp - 1] = -self.stack[self.sp - 1];
                    }
                    2 => {
                        // ADD
                        self.sp -= 1;
                        self.stack[self.sp - 1] += self.stack[self.sp];
                    }
                    3 => {
                        // SUB
                        self.sp -= 1;
                        self.stack[self.sp - 1] -= self.stack[self.sp];
                    }
                    4 => {
                        // MUL
                        self.sp -= 1;
                        self.stack[self.sp - 1] *= self.stack[self.sp];
                    }
                    5 => {
                        // DIV
                        self.sp -= 1;
                        if self.stack[self.sp] == 0 {
                            self.state = VMState::Error("Division by zero".to_string());
                            return;
                        }
                        self.stack[self.sp - 1] /= self.stack[self.sp];
                    }
                    6 => {
                        // ODD
                        self.stack[self.sp - 1] %= 2;
                    }
                    8 => {
                        // EQL
                        self.sp -= 1;
                        self.stack[self.sp - 1] = if self.stack[self.sp - 1] == self.stack[self.sp]
                        {
                            1
                        } else {
                            0
                        };
                    }
                    9 => {
                        // NEQ
                        self.sp -= 1;
                        self.stack[self.sp - 1] = if self.stack[self.sp - 1] != self.stack[self.sp]
                        {
                            1
                        } else {
                            0
                        };
                    }
                    10 => {
                        // LSS
                        self.sp -= 1;
                        self.stack[self.sp - 1] = if self.stack[self.sp - 1] < self.stack[self.sp] {
                            1
                        } else {
                            0
                        };
                    }
                    11 => {
                        // GEQ
                        self.sp -= 1;
                        self.stack[self.sp - 1] = if self.stack[self.sp - 1] >= self.stack[self.sp]
                        {
                            1
                        } else {
                            0
                        };
                    }
                    12 => {
                        // GTR
                        self.sp -= 1;
                        self.stack[self.sp - 1] = if self.stack[self.sp - 1] > self.stack[self.sp] {
                            1
                        } else {
                            0
                        };
                    }
                    13 => {
                        // LEQ
                        self.sp -= 1;
                        self.stack[self.sp - 1] = if self.stack[self.sp - 1] <= self.stack[self.sp]
                        {
                            1
                        } else {
                            0
                        };
                    }
                    _ => {
                        self.state = VMState::Error(format!("Unknown OPR {}", ir.a));
                    }
                }
            }
            OpCode::LOD => {
                let base = self.base(ir.l);
                let addr = (base as i64 + ir.a) as usize;
                self.stack[self.sp] = self.stack[addr];
                self.sp += 1;
            }
            OpCode::STO => {
                let base = self.base(ir.l);
                let addr = (base as i64 + ir.a) as usize;
                self.sp -= 1;
                self.stack[addr] = self.stack[self.sp];
            }
            OpCode::CAL => {
                let base = self.base(ir.l);
                self.stack[self.sp] = base as i64; // Static Link
                self.stack[self.sp + 1] = self.bp as i64; // Dynamic Link
                self.stack[self.sp + 2] = self.pc as i64; // Return Address
                self.bp = self.sp;
                self.pc = ir.a as usize;
            }
            OpCode::INT => {
                self.sp = (self.sp as i64 + ir.a) as usize;
            }
            OpCode::JMP => {
                self.pc = ir.a as usize;
            }
            OpCode::JPC => {
                self.sp -= 1;
                if self.stack[self.sp] == 0 {
                    self.pc = ir.a as usize;
                }
            }
            OpCode::RED => {
                if let Some(val) = self.input_queue.pop() {
                    let base = self.base(ir.l);
                    let addr = (base as i64 + ir.a) as usize;
                    self.stack[addr] = val;
                } else {
                    // Push back PC to retry this instruction when input is available
                    self.pc -= 1;
                    self.state = VMState::WaitingForInput;
                }
            }
            OpCode::WRT => {
                self.sp -= 1; // Pop
                let val = self.stack[self.sp];
                self.output.push(val.to_string());
            }
        }
    }

    pub fn interpret(&mut self) {
        println!("Start PL/0");
        self.pc = 0;
        self.bp = 0;
        self.sp = 0;
        self.state = VMState::Running;

        loop {
            match self.state {
                VMState::Running => self.step(),
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

        for line in &self.output {
            println!("{}", line);
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

        assert_eq!(vm.stack[vm.sp - 1], 30);
    }
}
