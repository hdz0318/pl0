use crate::ast::{Block as AstBlock, Program, Statement};
use crate::codegen::CodeGenerator;
use crate::lexer::Lexer;
use crate::optimizer::optimize_ast;
use crate::parser::Parser;
use crate::semantic::SemanticAnalyzer;
use crate::symbol_table::SymbolTable;
use crate::types::Instruction;
use crate::vm::{VM, VMState};
use eframe::egui;
use std::time::{Duration, Instant};

#[derive(PartialEq)]
enum Tab {
    Editor,
    Tokens,
    Ast,
    Symbols,
    Optimization,
    Runtime,
}

struct DiffLine {
    raw: Option<(usize, Instruction)>,
    opt: Option<(usize, Instruction)>,
}

fn compute_diff(raw: &[Instruction], opt: &[Instruction]) -> Vec<DiffLine> {
    let n = raw.len();
    let m = opt.len();
    // DP table for LCS length
    let mut dp = vec![vec![0; m + 1]; n + 1];

    for i in 1..=n {
        for j in 1..=m {
            // Instruction derives PartialEq
            if raw[i - 1] == opt[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to find the diff
    let mut i = n;
    let mut j = m;
    let mut diffs = Vec::new();

    while i > 0 && j > 0 {
        if raw[i - 1] == opt[j - 1] {
            diffs.push(DiffLine {
                raw: Some((i - 1, raw[i - 1])),
                opt: Some((j - 1, opt[j - 1])),
            });
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            diffs.push(DiffLine {
                raw: Some((i - 1, raw[i - 1])),
                opt: None,
            });
            i -= 1;
        } else {
            diffs.push(DiffLine {
                raw: None,
                opt: Some((j - 1, opt[j - 1])),
            });
            j -= 1;
        }
    }

    while i > 0 {
        diffs.push(DiffLine {
            raw: Some((i - 1, raw[i - 1])),
            opt: None,
        });
        i -= 1;
    }

    while j > 0 {
        diffs.push(DiffLine {
            raw: None,
            opt: Some((j - 1, opt[j - 1])),
        });
        j -= 1;
    }

    diffs.reverse();
    diffs
}

pub struct Pl0Gui {
    // State
    source_code: String,
    tokens: Vec<(usize, usize, crate::types::TokenType)>,
    ast: Option<Program>,
    symbol_table: Option<SymbolTable>,
    raw_code: Vec<Instruction>,
    opt_code: Vec<Instruction>,
    vm: VM,

    // UI State
    current_tab: Tab,
    status_message: String,
    auto_run: bool,
    last_tick: Instant,
    input_buffer: String,
    use_optimized_vm: bool,

    // Visualization
    viz_root: Option<VizNode>,

    // Compilation diagnostics
    diagnostics: Vec<String>,
}

#[derive(Clone)]
struct VizNode {
    label: String,
    color: egui::Color32,
    children: Vec<VizNode>,
    pos: egui::Pos2,
    width: f32,
}

impl VizNode {
    fn new(label: impl Into<String>, color: egui::Color32) -> Self {
        Self {
            label: label.into(),
            color,
            children: Vec::new(),
            pos: egui::Pos2::ZERO,
            width: 0.0,
        }
    }
}

impl Pl0Gui {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Customize fonts if needed
        // cc.egui_ctx.set_fonts(...);

        let default_code = "program test1;
const a := 10, b := 20;
var x, y, z;
begin
  read(x);
  y := a * x + b;
  z := y / 2;
  write(x, y, z)
end";

        let mut app = Self {
            source_code: default_code.to_string(),
            tokens: Vec::new(),
            ast: None,
            symbol_table: None,
            raw_code: vec![],
            opt_code: vec![],
            vm: VM::new(vec![]),
            current_tab: Tab::Editor,
            status_message: "Ready".to_string(),
            auto_run: false,
            last_tick: Instant::now(),
            input_buffer: String::new(),
            use_optimized_vm: true,
            viz_root: None,
            diagnostics: Vec::new(),
        };
        app.compile();
        app
    }

    fn format_parse_error(&self, err: &crate::parser::ParseError) -> String {
        let lines: Vec<&str> = self.source_code.lines().collect();
        let mut msg = format!("Line {}, Col {}: {}", err.line, err.col, err.message);
        
        if err.line > 0 && err.line <= lines.len() {
            let line_content = lines[err.line - 1];
            msg.push_str(&format!("\n    {}", line_content));
            
            let indent: String = line_content
                .chars()
                .take(err.col - 1)
                .map(|c| if c.is_whitespace() { c } else { ' ' })
                .collect();
            msg.push_str(&format!("\n    {}^", indent));
        }
        msg
    }

    fn compile(&mut self) {
        self.status_message = "Compiling...".to_string();
        self.diagnostics.clear();
        self.tokens.clear();
        self.symbol_table = None;

        // 0. Lexical Analysis (Visualization)
        let mut lexer_viz = Lexer::new(&self.source_code);
        loop {
            let token = lexer_viz.current_token.clone();
            self.tokens
                .push((lexer_viz.token_line, lexer_viz.token_col, token.clone()));
            if token == crate::types::TokenType::Eof {
                break;
            }
            lexer_viz.next_token();
        }

        let lexer = Lexer::new(&self.source_code);
        let mut parser = Parser::new(lexer, false);

        match parser.parse() {
            Ok(mut program) => {
                if !parser.errors.is_empty() {
                    for err in &parser.errors {
                        self.diagnostics.push(self.format_parse_error(err));
                    }
                    self.status_message = "Parsing Failed".to_string();
                    self.raw_code.clear();
                    self.opt_code.clear();
                    return;
                }

                // 1. Generate Raw Code
                let mut raw_program = program.clone();
                let mut sym_table = SymbolTable::new();
                let mut analyzer = SemanticAnalyzer::new(&mut sym_table);

                if let Err(e) = analyzer.analyze(&mut raw_program) {
                    self.diagnostics = e;
                    self.status_message = "Semantic Analysis Failed".to_string();
                    // Even if semantic error, we might want to show symbol table so far?
                    // But usually it fails early. Let's save what we have.
                    self.symbol_table = Some(sym_table);
                    return;
                }

                self.symbol_table = Some(sym_table.clone()); // Save for visualization
                self.ast = Some(raw_program.clone());

                // Build Visualization Tree
                let mut root = build_viz_tree(&raw_program);
                layout_viz_tree(&mut root, 0, &mut 0.0);
                self.viz_root = Some(root);

                let mut generator = CodeGenerator::new();
                self.raw_code = generator.generate(&raw_program, &mut sym_table);

                // 2. Optimize AST & Generate Optimized Code
                optimize_ast(&mut program);
                let mut opt_sym_table = SymbolTable::new();
                let mut opt_analyzer = SemanticAnalyzer::new(&mut opt_sym_table);

                if let Err(e) = opt_analyzer.analyze(&mut program) {
                    self.diagnostics = e;
                    self.status_message = "Semantic Analysis Failed (Opt)".to_string();
                    return;
                }

                let mut opt_generator = CodeGenerator::new();
                let code_from_ast = opt_generator.generate(&program, &mut opt_sym_table);

                // 3. Peephole Optimization (Removed)
                self.opt_code = code_from_ast;
                let code_to_run = if self.use_optimized_vm {
                    self.opt_code.clone()
                } else {
                    self.raw_code.clone()
                };
                self.vm = VM::new(code_to_run);
                self.status_message = "Compilation Successful".to_string();
            }
            Err(_) => {
                // This branch might be unreachable now if we handle errors in Ok, 
                // but if parse() returns Err, it means catastrophic failure or we changed parser logic.
                // Let's just show whatever errors we have.
                if !parser.errors.is_empty() {
                    for err in &parser.errors {
                        self.diagnostics.push(self.format_parse_error(err));
                    }
                } else {
                    self.diagnostics.push("Unknown Parse Error".to_string());
                }
                self.status_message = "Parsing Failed".to_string();
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
                ui.selectable_value(&mut self.current_tab, Tab::Tokens, "🔤 Tokens");
                ui.selectable_value(&mut self.current_tab, Tab::Ast, "🌳 AST");
                ui.selectable_value(&mut self.current_tab, Tab::Symbols, "📚 Symbols");
                ui.selectable_value(&mut self.current_tab, Tab::Optimization, "⚡ Optimization");
                ui.selectable_value(&mut self.current_tab, Tab::Runtime, "🚀 Runtime");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !self.diagnostics.is_empty() {
                        ui.colored_label(egui::Color32::RED, &self.status_message);
                    } else {
                        ui.label(&self.status_message);
                    }
                });
            });
        });

        // Central Panel: Content
        egui::CentralPanel::default().show(ctx, |ui| match self.current_tab {
            Tab::Editor => self.show_editor(ui),
            Tab::Tokens => self.show_tokens(ui),
            Tab::Ast => self.show_ast(ui),
            Tab::Symbols => self.show_symbols(ui),
            Tab::Optimization => self.show_optimization(ui),
            Tab::Runtime => self.show_runtime(ui, ctx),
        });
    }
}

impl Pl0Gui {
    fn show_editor(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Source Code");
            if ui.button("📂 Open File...").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("PL/0 Source", &["pl0", "txt"])
                    .pick_file()
                    && let Ok(content) = std::fs::read_to_string(path) {
                        self.source_code = content;
                        self.compile();
                    }
        });

        let available_height = ui.available_height();
        let error_height = if !self.diagnostics.is_empty() {
            (available_height * 0.3).max(100.0)
        } else {
            0.0
        };
        let editor_height = available_height - error_height;

        egui::ScrollArea::vertical()
            .id_salt("editor_scroll")
            .max_height(editor_height)
            .show(ui, |ui| {
                let response = ui.add_sized(
                    ui.available_size(),
                    egui::TextEdit::multiline(&mut self.source_code)
                        .font(egui::TextStyle::Monospace)
                        .code_editor()
                        .lock_focus(true),
                );
                if response.changed() {
                    self.compile();
                }
            });

        if !self.diagnostics.is_empty() {
            ui.separator();
            ui.heading("Diagnostics");
            egui::ScrollArea::vertical()
                .id_salt("error_scroll")
                .show(ui, |ui| {
                    for diag in &self.diagnostics {
                        ui.colored_label(egui::Color32::RED, diag);
                        ui.separator();
                    }
                });
        }
    }

    fn show_tokens(&self, ui: &mut egui::Ui) {
        ui.heading("Lexical Analysis (Tokens)");
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("tokens_grid")
                .striped(true)
                .spacing([20.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Line:Col").strong());
                    ui.label(egui::RichText::new("Token Type").strong());
                    ui.label(egui::RichText::new("Value").strong());
                    ui.end_row();

                    for (line, col, token) in &self.tokens {
                        ui.monospace(format!("{}:{}", line, col));
                        match token {
                            crate::types::TokenType::Identifier(s) => {
                                ui.monospace("Identifier");
                                ui.monospace(s);
                            }
                            crate::types::TokenType::Number(n) => {
                                ui.monospace("Number");
                                ui.monospace(n.to_string());
                            }
                            _ => {
                                ui.monospace(format!("{:?}", token));
                                ui.label("");
                            }
                        }
                        ui.end_row();
                    }
                });
        });
    }

    fn show_symbols(&self, ui: &mut egui::Ui) {
        ui.heading("Symbol Table");
        if let Some(sym_table) = &self.symbol_table {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, scope) in sym_table.scopes.iter().enumerate() {
                    egui::CollapsingHeader::new(format!("Scope {}", i))
                        .default_open(true)
                        .show(ui, |ui| {
                            if let Some(parent) = scope.parent {
                                ui.label(format!("Parent Scope: {}", parent));
                            } else {
                                ui.label("Root Scope");
                            }

                            if !scope.symbols.is_empty() {
                                egui::Grid::new(format!("scope_{}_grid", i))
                                    .striped(true)
                                    .spacing([20.0, 4.0])
                                    .show(ui, |ui| {
                                        ui.label(egui::RichText::new("Name").strong());
                                        ui.label(egui::RichText::new("Kind").strong());
                                        ui.label(egui::RichText::new("Details").strong());
                                        ui.end_row();

                                        for (name, sym) in &scope.symbols {
                                            ui.monospace(name);
                                            match &sym.kind {
                                                crate::types::SymbolType::Constant { val } => {
                                                    ui.monospace("Constant");
                                                    ui.monospace(format!("Value: {}", val));
                                                }
                                                crate::types::SymbolType::Variable {
                                                    level,
                                                    addr,
                                                } => {
                                                    ui.monospace("Variable");
                                                    ui.monospace(format!(
                                                        "L: {}, A: {}",
                                                        level, addr
                                                    ));
                                                }
                                                crate::types::SymbolType::Procedure {
                                                    level,
                                                    addr,
                                                } => {
                                                    ui.monospace("Procedure");
                                                    ui.monospace(format!(
                                                        "L: {}, A: {}",
                                                        level, addr
                                                    ));
                                                }
                                            }
                                            ui.end_row();
                                        }
                                    });
                            } else {
                                ui.label("No symbols in this scope.");
                            }
                        });
                }
            });
        } else {
            ui.label("No symbol table available.");
        }
    }

    fn show_ast(&self, ui: &mut egui::Ui) {
        ui.heading("Abstract Syntax Tree (Visualized)");
        if let Some(root) = &self.viz_root {
            egui::ScrollArea::both().show(ui, |ui| {
                let (max_x, max_y) = get_tree_bounds(root);
                let canvas_size = egui::vec2(max_x + 120.0, max_y + 120.0);
                let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::hover());

                // Offset ensures a small margin inside the canvas
                let offset = response.rect.min.to_vec2() + egui::vec2(20.0, 20.0);

                draw_viz_tree(&painter, root, offset);
            });
        } else {
            ui.label("No AST available. Fix compilation errors.");
        }
    }



    fn show_optimization(&self, ui: &mut egui::Ui) {
        let diffs = compute_diff(&self.raw_code, &self.opt_code);

        ui.heading(format!(
            "Optimization Diff ({} -> {} instructions)",
            self.raw_code.len(),
            self.opt_code.len()
        ));

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("diff_grid")
                .striped(true)
                .min_col_width(200.0)
                .spacing([20.0, 4.0]) // Add some spacing between columns and rows
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Raw Bytecode").strong());
                    ui.label(egui::RichText::new("Optimized Bytecode").strong());
                    ui.end_row();

                    for line in diffs {
                        // Left Column (Raw)
                        if let Some((i, instr)) = line.raw {
                            let text = format!("{:3}: {:?} {}, {}", i, instr.f, instr.l, instr.a);
                            if line.opt.is_none() {
                                // Deleted - Red
                                ui.label(
                                    egui::RichText::new(text)
                                        .color(egui::Color32::from_rgb(255, 100, 100)),
                                );
                            } else {
                                // Unchanged
                                ui.monospace(text);
                            }
                        } else {
                            ui.label("");
                        }

                        // Right Column (Optimized)
                        if let Some((i, instr)) = line.opt {
                            let text = format!("{:3}: {:?} {}, {}", i, instr.f, instr.l, instr.a);
                            if line.raw.is_none() {
                                // Added - Green
                                ui.label(
                                    egui::RichText::new(text)
                                        .color(egui::Color32::from_rgb(100, 255, 100)),
                                );
                            } else {
                                // Unchanged
                                ui.monospace(text);
                            }
                        } else {
                            ui.label("");
                        }
                        ui.end_row();
                    }
                });
        });
    }

    fn show_runtime(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Controls Row 1: Actions & Settings
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
                let code_to_run = if self.use_optimized_vm {
                    self.opt_code.clone()
                } else {
                    self.raw_code.clone()
                };
                self.vm = VM::new(code_to_run);
                self.auto_run = false;
            }

            ui.separator();
            if ui
                .checkbox(&mut self.use_optimized_vm, "Use Optimized Code")
                .changed()
            {
                let code_to_run = if self.use_optimized_vm {
                    self.opt_code.clone()
                } else {
                    self.raw_code.clone()
                };
                self.vm = VM::new(code_to_run);
                self.auto_run = false;
            }
        });

        // Controls Row 2: Status & Registers
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Registers:").strong());
            ui.label(format!("PC: {:<3}", self.vm.p));
            ui.label(format!("BP: {:<3}", self.vm.b));
            ui.label(format!("SP: {:<3}", self.vm.t));
            ui.label(format!("IR: {:?}", self.vm.i));

            ui.separator();
            ui.label(format!("State: {:?}", self.vm.state));

            ui.separator();
            ui.label(format!("Total Instructions: {}", self.vm.instruction_count));
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
                                        .monospace()
                                        .strong()
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
                    .id_salt("vm_stack")
                    .show(ui, |ui| {
                        egui::Grid::new("stack_grid")
                            .striped(true)
                            .spacing([10.0, 4.0])
                            .min_col_width(40.0)
                            .show(ui, |ui| {
                                // Header
                                ui.label(egui::RichText::new("Addr").strong().underline());
                                ui.label(egui::RichText::new("Value").strong().underline());
                                ui.label(egui::RichText::new("Tags").strong().underline());
                                ui.end_row();

                                // Content
                                let limit = ((self.vm.t + 1).max(self.vm.b + 3))
                                    .min(self.vm.stack.len());
                                for (i, val) in self.vm.stack.iter().enumerate().take(limit) {
                                    let is_bp = i == self.vm.b;
                                    let is_sp = i == self.vm.t;
                                    let is_sl = i == self.vm.b;
                                    let is_dl = i == self.vm.b + 1;
                                    let is_ra = i == self.vm.b + 2;

                                    let mut addr_text =
                                        egui::RichText::new(format!("{:03}", i)).monospace();
                                    let mut val_text =
                                        egui::RichText::new(val.to_string()).monospace();

                                    if is_bp {
                                        addr_text = addr_text.color(egui::Color32::LIGHT_BLUE);
                                        val_text = val_text.strong();
                                    }
                                    if is_sp {
                                        addr_text = addr_text.color(egui::Color32::LIGHT_RED);
                                        val_text = val_text.strong();
                                    }

                                    ui.label(addr_text);
                                    ui.label(val_text);

                                    // Tags column
                                    ui.horizontal(|ui| {
                                        if is_bp {
                                            ui.label(
                                                egui::RichText::new(" BP ")
                                                    .small()
                                                    .background_color(egui::Color32::from_rgb(
                                                        0, 100, 200,
                                                    ))
                                                    .color(egui::Color32::WHITE),
                                            );
                                        }
                                        if is_sp {
                                            ui.label(
                                                egui::RichText::new(" SP ")
                                                    .small()
                                                    .background_color(egui::Color32::from_rgb(
                                                        200, 50, 50,
                                                    ))
                                                    .color(egui::Color32::WHITE),
                                            );
                                        }
                                        if is_sl {
                                            ui.label(
                                                egui::RichText::new(" SL ")
                                                    .small()
                                                    .background_color(egui::Color32::GRAY)
                                                    .color(egui::Color32::WHITE),
                                            );
                                        }
                                        if is_dl {
                                            ui.label(
                                                egui::RichText::new(" DL ")
                                                    .small()
                                                    .background_color(egui::Color32::GRAY)
                                                    .color(egui::Color32::WHITE),
                                            );
                                        }
                                        if is_ra {
                                            ui.label(
                                                egui::RichText::new(" RA ")
                                                    .small()
                                                    .background_color(egui::Color32::GRAY)
                                                    .color(egui::Color32::WHITE),
                                            );
                                        }
                                    });
                                    ui.end_row();
                                }
                            });
                    });
            });

            // 3. I/O View
            columns[2].vertical(|ui| {
                ui.heading("I/O Console");

                egui::Frame::canvas(ui.style())
                    .fill(egui::Color32::from_rgb(40, 44, 52)) // Dark console background
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        // Output Area
                        let footer_height = 30.0;
                        let available_height = ui.available_height() - footer_height;

                        egui::ScrollArea::vertical()
                            .id_salt("vm_output")
                            .max_height(available_height)
                            .stick_to_bottom(true) // Auto-scroll to bottom
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.set_min_height(available_height);

                                for line in &self.vm.output {
                                    ui.label(
                                        egui::RichText::new(line)
                                            .monospace()
                                            .color(egui::Color32::LIGHT_GRAY),
                                    );
                                }

                                if self.vm.state == VMState::WaitingForInput {
                                    ui.label(
                                        egui::RichText::new("? Waiting for input...")
                                            .monospace()
                                            .italics()
                                            .color(egui::Color32::YELLOW),
                                    );
                                }
                            });

                        ui.separator();

                        // Input Area
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(">")
                                    .monospace()
                                    .strong()
                                    .color(egui::Color32::WHITE),
                            );

                            let response = ui.add(
                                egui::TextEdit::singleline(&mut self.input_buffer)
                                    .frame(false)
                                    .desired_width(f32::INFINITY)
                                    .text_color(egui::Color32::WHITE)
                                    .font(egui::TextStyle::Monospace)
                                    .hint_text("Enter number..."),
                            );

                            // Handle Enter key
                            if ((response.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                || (response.has_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))))
                                && !self.input_buffer.is_empty()
                                    && let Ok(val) = self.input_buffer.trim().parse::<i64>() {
                                        self.vm.input_queue.push(val);
                                        // Echo input to output
                                        self.vm.output.push(format!("> {}", val));

                                        if self.vm.state == VMState::WaitingForInput {
                                            self.vm.state = VMState::Running;
                                        }
                                        self.input_buffer.clear();

                                        // Keep focus
                                        response.request_focus();
                                    }

                            // Auto-focus if waiting
                            if self.vm.state == VMState::WaitingForInput && !response.has_focus() {
                                response.request_focus();
                            }
                        });
                    });
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

fn build_viz_tree(program: &Program) -> VizNode {
    let mut root = VizNode::new("Program", egui::Color32::from_rgb(200, 200, 255));
    root.children.push(build_block_node(&program.block));
    root
}

fn build_block_node(block: &AstBlock) -> VizNode {
    let mut node = VizNode::new("Block", egui::Color32::from_rgb(255, 200, 200));

    // Consts
    if !block.consts.is_empty() {
        let mut consts_node = VizNode::new("Consts", egui::Color32::LIGHT_GRAY);
        for c in &block.consts {
            consts_node.children.push(VizNode::new(
                format!("{}={}", c.name, c.value),
                egui::Color32::WHITE,
            ));
        }
        node.children.push(consts_node);
    }

    // Vars
    if !block.vars.is_empty() {
        let mut vars_node = VizNode::new("Vars", egui::Color32::LIGHT_GRAY);
        for v in &block.vars {
            vars_node
                .children
                .push(VizNode::new(v.clone(), egui::Color32::WHITE));
        }
        node.children.push(vars_node);
    }

    // Procedures
    for p in &block.procedures {
        let mut proc_node = VizNode::new(format!("Proc {}", p.name), egui::Color32::GOLD);
        proc_node.children.push(build_block_node(&p.block));
        node.children.push(proc_node);
    }

    // Statement
    node.children.push(build_statement_node(&block.statement));

    node
}

fn build_statement_node(stmt: &Statement) -> VizNode {
    match stmt {
        Statement::Assignment { name, expr, .. } => {
            let mut node = VizNode::new(":=", egui::Color32::LIGHT_GREEN);
            node.children
                .push(VizNode::new(name.clone(), egui::Color32::WHITE));
            node.children.push(build_expr_node(expr));
            node
        }
        Statement::Call { name, args, .. } => {
            let mut node = VizNode::new(format!("Call {}", name), egui::Color32::LIGHT_RED);
            for arg in args {
                node.children.push(build_expr_node(arg));
            }
            node
        }
        Statement::BeginEnd { statements } => {
            let mut node = VizNode::new("Begin..End", egui::Color32::from_rgb(200, 200, 255));
            for s in statements {
                node.children.push(build_statement_node(s));
            }
            node
        }
        Statement::If {
            condition,
            then_stmt,
            else_stmt,
            ..
        } => {
            let mut node = VizNode::new("If", egui::Color32::LIGHT_YELLOW);
            node.children.push(build_condition_node(condition));
            node.children.push(build_statement_node(then_stmt));
            if let Some(else_s) = else_stmt {
                node.children.push(build_statement_node(else_s));
            }
            node
        }
        Statement::While { condition, body, .. } => {
            let mut node = VizNode::new("While", egui::Color32::LIGHT_YELLOW);
            node.children.push(build_condition_node(condition));
            node.children.push(build_statement_node(body));
            node
        }
        Statement::Read { names, .. } => {
            let mut node = VizNode::new("Read", egui::Color32::LIGHT_BLUE);
            for name in names {
                node.children
                    .push(VizNode::new(name.clone(), egui::Color32::WHITE));
            }
            node
        }
        Statement::Write { exprs, .. } => {
            let mut node = VizNode::new("Write", egui::Color32::LIGHT_BLUE);
            for expr in exprs {
                node.children.push(build_expr_node(expr));
            }
            node
        }
        Statement::Empty => VizNode::new("Empty", egui::Color32::GRAY),
    }
}

fn build_expr_node(expr: &crate::ast::Expr) -> VizNode {
    match expr {
        crate::ast::Expr::Binary { left, op, right } => {
            let mut node = VizNode::new(format_op(op), egui::Color32::LIGHT_GREEN);
            node.children.push(build_expr_node(left));
            node.children.push(build_expr_node(right));
            node
        }
        crate::ast::Expr::Unary { op, expr } => {
            let mut node = VizNode::new(
                format!("Unary {}", format_op(op)),
                egui::Color32::LIGHT_GREEN,
            );
            node.children.push(build_expr_node(expr));
            node
        }
        crate::ast::Expr::Number(n) => VizNode::new(n.to_string(), egui::Color32::WHITE),
        crate::ast::Expr::Identifier(id) => VizNode::new(id.clone(), egui::Color32::WHITE),
    }
}

fn build_condition_node(cond: &crate::ast::Condition) -> VizNode {
    match cond {
        crate::ast::Condition::Odd { expr } => {
            let mut node = VizNode::new("Odd", egui::Color32::LIGHT_YELLOW);
            node.children.push(build_expr_node(expr));
            node
        }
        crate::ast::Condition::Compare { left, op, right } => {
            let mut node = VizNode::new(format_op(op), egui::Color32::LIGHT_YELLOW);
            node.children.push(build_expr_node(left));
            node.children.push(build_expr_node(right));
            node
        }
    }
}

fn format_op(op: &crate::types::Operator) -> String {
    match op {
        crate::types::Operator::ADD => "+".to_string(),
        crate::types::Operator::SUB => "-".to_string(),
        crate::types::Operator::MUL => "*".to_string(),
        crate::types::Operator::DIV => "/".to_string(),
        crate::types::Operator::EQL => "=".to_string(),
        crate::types::Operator::NEQ => "#".to_string(),
        crate::types::Operator::LSS => "<".to_string(),
        crate::types::Operator::LEQ => "<=".to_string(),
        crate::types::Operator::GTR => ">".to_string(),
        crate::types::Operator::GEQ => ">=".to_string(),
        crate::types::Operator::ODD => "odd".to_string(),
        crate::types::Operator::NEG => "-".to_string(),
        _ => format!("{:?}", op),
    }
}

fn layout_viz_tree(node: &mut VizNode, depth: usize, next_x: &mut f32) {
    let node_width = 100.0;
    let node_height = 60.0;
    let spacing_x = 20.0;

    if node.children.is_empty() {
        node.pos = egui::pos2(*next_x, depth as f32 * node_height);
        node.width = node_width;
        *next_x += node_width + spacing_x;
    } else {
        let _start_x = *next_x;
        for child in &mut node.children {
            layout_viz_tree(child, depth + 1, next_x);
        }

        // Center parent over children
        // If children are spread out, we want the parent to be in the middle of the range covered by children
        let first_child_x = node.children.first().unwrap().pos.x;
        let last_child_x = node.children.last().unwrap().pos.x;
        let center_x = (first_child_x + last_child_x) / 2.0;

        node.pos = egui::pos2(center_x, depth as f32 * node_height);
        node.width = node_width;

        // If the children didn't advance next_x enough (e.g. they were placed to the left of current next_x because of recursion logic?),
        // actually next_x is always increasing in this simple algorithm.
        // But we need to make sure we don't overlap with previous subtrees if we just center.
        // The simple "next_x" approach guarantees no overlap between leaves.
        // Parent centering is safe.
    }
}

fn get_tree_bounds(node: &VizNode) -> (f32, f32) {
    let mut max_x = node.pos.x + node.width;
    let mut max_y = node.pos.y + 50.0;

    for child in &node.children {
        let (cx, cy) = get_tree_bounds(child);
        max_x = max_x.max(cx);
        max_y = max_y.max(cy);
    }
    (max_x, max_y)
}

fn draw_viz_tree(painter: &egui::Painter, node: &VizNode, offset: egui::Vec2) {
    let node_size = egui::vec2(90.0, 30.0);
    let node_pos = node.pos + offset;
    let rect = egui::Rect::from_min_size(node_pos, node_size);

    // Draw edges to children
    for child in &node.children {
        let child_pos = child.pos + offset;
        let child_rect = egui::Rect::from_min_size(child_pos, node_size);

        let start = rect.center_bottom();
        let end = child_rect.center_top();

        // Bezier curve for nicer edges
        let control_point1 = start + egui::vec2(0.0, 20.0);
        let control_point2 = end - egui::vec2(0.0, 20.0);

        let cubic_bezier = egui::epaint::CubicBezierShape::from_points_stroke(
            [start, control_point1, control_point2, end],
            false,
            egui::Color32::TRANSPARENT, // fill
            egui::Stroke::new(1.5, egui::Color32::GRAY),
        );
        painter.add(cubic_bezier);

        draw_viz_tree(painter, child, offset);
    }

    // Draw node box
    painter.rect_filled(rect, 5.0, node.color);
    painter.rect_stroke(
        rect,
        5.0,
        egui::Stroke::new(1.0, egui::Color32::BLACK),
        egui::StrokeKind::Middle,
    );

    // Draw label
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        &node.label,
        egui::FontId::proportional(14.0),
        egui::Color32::BLACK,
    );
}
