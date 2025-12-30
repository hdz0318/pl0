use crate::ast::{Block as AstBlock, Program, Statement};
use crate::codegen::CodeGenerator;
use crate::lexer::Lexer;
use crate::optimizer::{optimize, optimize_ast};
use crate::parser::Parser;
use crate::symbol_table::SymbolTable;
use crate::types::{Instruction, OpCode};
use crate::vm::{VM, VMState};
use eframe::egui;
use std::time::{Duration, Instant};

#[derive(PartialEq)]
enum Tab {
    Editor,
    AST,
    Optimization,
    Runtime,
}

pub struct Pl0Gui {
    // State
    source_code: String,
    ast: Option<Program>,
    raw_code: Vec<Instruction>,
    opt_code: Vec<Instruction>,
    vm: VM,

    // UI State
    current_tab: Tab,
    status_message: String,
    auto_run: bool,
    last_tick: Instant,
    input_buffer: String,

    // Compilation error
    compile_error: Option<String>,
}

impl Pl0Gui {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Customize fonts if needed
        // cc.egui_ctx.set_fonts(...);

        let default_code = "const m = 7, n = 85;
var x, y, z, q, r;

procedure multiply;
var a, b;
begin
  a := x;
  b := y;
  z := 0;
  while b > 0 do
  begin
    if b / 2 * 2 # b then z := z + a;
    a := 2 * a;
    b := b / 2
  end
end;

begin
  x := m;
  y := n;
  call multiply;
  write(z)
end.";

        let mut app = Self {
            source_code: default_code.to_string(),
            ast: None,
            raw_code: vec![],
            opt_code: vec![],
            vm: VM::new(vec![]),
            current_tab: Tab::Editor,
            status_message: "Ready".to_string(),
            auto_run: false,
            last_tick: Instant::now(),
            input_buffer: String::new(),
            compile_error: None,
        };
        app.compile();
        app
    }

    fn compile(&mut self) {
        self.status_message = "Compiling...".to_string();
        self.compile_error = None;

        let lexer = Lexer::new(&self.source_code);
        let mut parser = Parser::new(lexer, false);

        match parser.parse() {
            Ok(mut program) => {
                self.ast = Some(program.clone());

                // 1. Generate Raw Code
                let mut sym_table = SymbolTable::new();
                let mut generator = CodeGenerator::new();
                self.raw_code = generator.generate(&program, &mut sym_table);

                // 2. Optimize AST & Generate Optimized Code
                optimize_ast(&mut program);
                let mut opt_sym_table = SymbolTable::new();
                let mut opt_generator = CodeGenerator::new();
                let code_from_ast = opt_generator.generate(&program, &mut opt_sym_table);

                // 3. Peephole Optimization
                self.opt_code = optimize(code_from_ast);

                // 4. Initialize VM with Optimized Code
                self.vm = VM::new(self.opt_code.clone());
                self.status_message = "Compilation Successful".to_string();
            }
            Err(_) => {
                let err = format!("Parse Error: {:?}", parser.errors.first());
                self.status_message = err.clone();
                self.compile_error = Some(err);
                self.raw_code.clear();
                self.opt_code.clear();
            }
        }
    }
}

impl eframe::App for Pl0Gui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top Panel: Tabs and Status
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.current_tab, Tab::Editor, "📝 Editor");
                ui.selectable_value(&mut self.current_tab, Tab::AST, "🌳 AST");
                ui.selectable_value(&mut self.current_tab, Tab::Optimization, "⚡ Optimization");
                ui.selectable_value(&mut self.current_tab, Tab::Runtime, "🚀 Runtime");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(err) = &self.compile_error {
                        ui.colored_label(egui::Color32::RED, err);
                    } else {
                        ui.label(&self.status_message);
                    }
                });
            });
        });

        // Central Panel: Content
        egui::CentralPanel::default().show(ctx, |ui| match self.current_tab {
            Tab::Editor => self.show_editor(ui),
            Tab::AST => self.show_ast(ui),
            Tab::Optimization => self.show_optimization(ui),
            Tab::Runtime => self.show_runtime(ui, ctx),
        });
    }
}

impl Pl0Gui {
    fn show_editor(&mut self, ui: &mut egui::Ui) {
        ui.heading("Source Code");
        let response = egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut self.source_code)
                    .font(egui::TextStyle::Monospace)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(30)
                    .lock_focus(true),
            )
        });

        if response.inner.changed() {
            self.compile();
        }
    }

    fn show_ast(&self, ui: &mut egui::Ui) {
        ui.heading("Abstract Syntax Tree");
        egui::ScrollArea::vertical().show(ui, |ui| {
            if let Some(program) = &self.ast {
                self.draw_block(ui, &program.block, "Program Block");
            } else {
                ui.label("No AST available. Fix compilation errors.");
            }
        });
    }

    fn draw_block(&self, ui: &mut egui::Ui, block: &AstBlock, label: &str) {
        egui::CollapsingHeader::new(label)
            .default_open(true)
            .show(ui, |ui| {
                if !block.consts.is_empty() {
                    ui.label(egui::RichText::new("Constants:").strong());
                    for c in &block.consts {
                        ui.label(format!("{} = {}", c.name, c.value));
                    }
                }

                if !block.vars.is_empty() {
                    ui.label(egui::RichText::new("Variables:").strong());
                    ui.label(block.vars.join(", "));
                }

                ui.separator();
                self.draw_statement(ui, &block.statement);
            });
    }

    fn draw_statement(&self, ui: &mut egui::Ui, stmt: &Statement) {
        match stmt {
            Statement::Assignment { name, expr: _ } => {
                ui.label(format!("Assign: {} := ...", name));
            }
            Statement::Call { name, args: _ } => {
                ui.label(format!("Call: {}", name));
            }
            Statement::BeginEnd { statements } => {
                egui::CollapsingHeader::new("Begin ... End")
                    .default_open(true)
                    .show(ui, |ui| {
                        for s in statements {
                            self.draw_statement(ui, s);
                        }
                    });
            }
            Statement::If {
                condition: _,
                then_stmt,
                else_stmt,
            } => {
                egui::CollapsingHeader::new("If ... Then")
                    .default_open(true)
                    .show(ui, |ui| {
                        self.draw_statement(ui, then_stmt);
                        if let Some(else_s) = else_stmt {
                            ui.label("Else");
                            self.draw_statement(ui, else_s);
                        }
                    });
            }
            Statement::While { condition: _, body } => {
                egui::CollapsingHeader::new("While ... Do")
                    .default_open(true)
                    .show(ui, |ui| {
                        self.draw_statement(ui, body);
                    });
            }
            Statement::Read { names } => {
                ui.label(format!("Read: {}", names.join(", ")));
            }
            Statement::Write { exprs: _ } => {
                ui.label("Write: ...");
            }
            Statement::Empty => {
                ui.label("Empty");
            }
        }
    }

    fn show_optimization(&self, ui: &mut egui::Ui) {
        ui.columns(2, |columns| {
            columns[0].heading("Raw Bytecode");
            egui::ScrollArea::vertical()
                .id_salt("raw")
                .show(&mut columns[0], |ui| {
                    for (i, instr) in self.raw_code.iter().enumerate() {
                        ui.monospace(format!("{:3}: {:?}", i, instr));
                    }
                });

            columns[1].heading(format!(
                "Optimized ({} -> {})",
                self.raw_code.len(),
                self.opt_code.len()
            ));
            egui::ScrollArea::vertical()
                .id_salt("opt")
                .show(&mut columns[1], |ui| {
                    for (i, instr) in self.opt_code.iter().enumerate() {
                        ui.monospace(format!("{:3}: {:?}", i, instr));
                    }
                });
        });
    }

    fn show_runtime(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Controls
        ui.horizontal(|ui| {
            if ui.button("Step").clicked() {
                self.vm.step();
            }
            if ui
                .button(if self.auto_run { "Pause" } else { "Run" })
                .clicked()
            {
                self.auto_run = !self.auto_run;
            }
            if ui.button("Reset").clicked() {
                self.vm = VM::new(self.opt_code.clone());
                self.auto_run = false;
            }

            ui.separator();
            ui.label(format!("State: {:?}", self.vm.state));
        });

        ui.separator();

        // Main Runtime View
        ui.columns(3, |columns| {
            // 1. Code View
            columns[0].vertical(|ui| {
                ui.heading("Code");
                egui::ScrollArea::vertical()
                    .id_salt("vm_code")
                    .show(ui, |ui| {
                        for (i, instr) in self.vm.code.iter().enumerate() {
                            let text = format!("{:3}: {:?}", i, instr);
                            if i == self.vm.p {
                                ui.label(
                                    egui::RichText::new(text)
                                        .background_color(egui::Color32::YELLOW)
                                        .color(egui::Color32::BLACK),
                                );
                                ui.scroll_to_cursor(Some(egui::Align::Center));
                            } else {
                                ui.monospace(text);
                            }
                        }
                    });
            });

            // 2. Stack View
            columns[1].vertical(|ui| {
                ui.heading("Stack");
                egui::ScrollArea::vertical()
                    .id_source("vm_stack")
                    .show(ui, |ui| {
                        for (i, val) in self.vm.stack.iter().enumerate().take(self.vm.t + 5) {
                            let mut text = format!("[{:3}] {}", i, val);
                            if i == self.vm.b {
                                text.push_str(" < BP");
                            }
                            if i == self.vm.t {
                                text.push_str(" < SP");
                            }
                            ui.monospace(text);
                        }
                    });
            });

            // 3. I/O View
            columns[2].vertical(|ui| {
                ui.heading("Output");
                egui::ScrollArea::vertical()
                    .id_source("vm_output")
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for line in &self.vm.output {
                            ui.monospace(line);
                        }
                    });

                ui.separator();
                ui.heading("Input");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.input_buffer);
                    if ui.button("Enter").clicked()
                        || (ui.input(|i| i.key_pressed(egui::Key::Enter))
                            && !self.input_buffer.is_empty())
                    {
                        if let Ok(val) = self.input_buffer.trim().parse::<i64>() {
                            self.vm.input_queue.push(val);
                            if self.vm.state == VMState::WaitingForInput {
                                self.vm.state = VMState::Running;
                            }
                            self.input_buffer.clear();
                        }
                    }
                });
                if self.vm.state == VMState::WaitingForInput {
                    ui.colored_label(egui::Color32::YELLOW, "Waiting for input...");
                }
            });
        });

        // Auto-run logic
        if self.auto_run && self.vm.state == VMState::Running {
            if self.last_tick.elapsed() >= Duration::from_millis(50) {
                self.vm.step();
                self.last_tick = Instant::now();
                ctx.request_repaint(); // Request next frame immediately
            } else {
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        }
    }
}
