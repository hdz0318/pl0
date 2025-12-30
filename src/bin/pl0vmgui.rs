use eframe::egui;
use pl0::types::{Instruction, OpCode};
use pl0::vm::{VM, VMState};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::{Duration, Instant};

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_title("PL/0 VM GUI"),
        ..Default::default()
    };

    eframe::run_native(
        "PL/0 VM GUI",
        native_options,
        Box::new(|cc| Ok(Box::new(Pl0VmGui::new(cc)))),
    )
}

struct Pl0VmGui {
    // State
    vm: VM,
    instructions: Vec<Instruction>, // Keep a copy for reset

    // UI State
    status_message: String,
    auto_run: bool,
    last_tick: Instant,
    input_buffer: String,
}

impl Pl0VmGui {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Customize fonts if needed
        Self {
            vm: VM::new(vec![]),
            instructions: vec![],
            status_message: "Ready. Load an ASM file to begin.".to_string(),
            auto_run: false,
            last_tick: Instant::now(),
            input_buffer: String::new(),
        }
    }

    fn parse_opcode(s: &str) -> Option<OpCode> {
        match s {
            "LIT" => Some(OpCode::LIT),
            "OPR" => Some(OpCode::OPR),
            "LOD" => Some(OpCode::LOD),
            "STO" => Some(OpCode::STO),
            "CAL" => Some(OpCode::CAL),
            "INT" => Some(OpCode::INT),
            "JMP" => Some(OpCode::JMP),
            "JPC" => Some(OpCode::JPC),
            "RED" => Some(OpCode::RED),
            "WRT" => Some(OpCode::WRT),
            _ => None,
        }
    }

    fn load_asm_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("ASM File", &["asm", "txt", "pl0asm"])
            .pick_file()
        {
            match File::open(&path) {
                Ok(file) => {
                    let reader = BufReader::new(file);
                    let mut new_instructions = Vec::new();
                    let mut success = true;

                    for (i, line) in reader.lines().enumerate() {
                        if let Ok(l) = line {
                            let parts: Vec<&str> = l.split_whitespace().collect();
                            if parts.is_empty() {
                                continue; // Skip empty lines
                            }
                            if parts.len() >= 3 {
                                if let Some(f) = Self::parse_opcode(parts[0]) {
                                    if let (Ok(l), Ok(a)) = (parts[1].parse::<usize>(), parts[2].parse::<i64>()) {
                                        new_instructions.push(Instruction::new(f, l, a));
                                    } else {
                                        self.status_message = format!("Error parsing line {}: Invalid numbers", i + 1);
                                        success = false;
                                        break;
                                    }
                                } else {
                                    self.status_message = format!("Error parsing line {}: Unknown opcode {}", i + 1, parts[0]);
                                    success = false;
                                    break;
                                }
                            } else {
                                // Maybe handle lines that don't match strict format? 
                                // For now, strict: Op L A
                                self.status_message = format!("Error parsing line {}: Expected 'OP L A'", i + 1);
                                success = false;
                                break;
                            }
                        }
                    }

                    if success {
                        self.instructions = new_instructions.clone();
                        self.vm = VM::new(new_instructions);
                        self.status_message = format!("Loaded {} instructions from {:?}", self.instructions.len(), path);
                        self.auto_run = false;
                    }
                }
                Err(e) => {
                    self.status_message = format!("Failed to open file: {}", e);
                }
            }
        }
    }
}

impl eframe::App for Pl0VmGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top Panel: Controls
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("📂 Load ASM").clicked() {
                    self.load_asm_file();
                }

                ui.separator();

                if ui.button("Step").clicked() {
                    if !self.instructions.is_empty() {
                         self.vm.step();
                    }
                }
                if ui
                    .button(if self.auto_run { "Pause" } else { "Run" })
                    .clicked()
                {
                    if !self.instructions.is_empty() {
                        self.auto_run = !self.auto_run;
                    }
                }
                if ui.button("Reset").clicked() {
                    self.vm = VM::new(self.instructions.clone());
                    self.auto_run = false;
                    self.status_message = "VM Reset".to_string();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(&self.status_message);
                });
            });
        });

        // Central Panel: Runtime View
        egui::CentralPanel::default().show(ctx, |ui| {
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
                            if self.vm.code.is_empty() {
                                ui.label("No code loaded.");
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
                                        let is_sl = i == self.vm.b; // Assuming static link is at BP
                                        let is_dl = i == self.vm.b + 1; // dynamic link
                                        let is_ra = i == self.vm.b + 2; // return addr

                                        // Note: The tagging logic here is a simplified visual aid
                                        // based on typical PL/0 stack frame structure (SL, DL, RA)
                                        // It might not be 100% accurate for all frames depending on implementation details in VM
                                        // but it matches the existing GUI logic.

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
        });
    }
}
