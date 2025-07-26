use eframe::egui;
use gdbadapter::{GdbAdapter, GdbEvent, AsyncClass, ResultClass, Value, Register, AssemblyLine, StackFrame};

pub fn run_gui() -> i32 {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Katori - GDB Frontend"),
        ..Default::default()
    };
    
    match eframe::run_native(
        "Katori",
        options,
        Box::new(|_cc| Box::new(KatoriApp::default())),
    ) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("Error running GUI: {}", e);
            1
        }
    }
}

/// Main application state
pub struct KatoriApp {
    /// GDB adapter instance
    gdb_adapter: GdbAdapter,
    
    /// Debug session state
    is_debugging: bool,
    is_attached: bool,
    current_pid: Option<u32>,
    current_host_port: String,
    
    /// UI state
    attach_mode: AttachMode,
    console_output: String,
    error_message: String,
    
    /// Debug information
    registers: Vec<Register>,
    assembly_lines: Vec<AssemblyLine>,
    stack_frames: Vec<StackFrame>,
    breakpoints: Vec<String>,
    
    /// UI panels visibility
    show_registers: bool,
    show_assembly: bool,
    show_stack: bool,
    show_memory: bool,
    show_console: bool,
    
    /// Memory viewer state
    memory_address: String,
    memory_size: u32,
    memory_data: Vec<u8>,
    
    /// Input fields
    breakpoint_input: String,
    pid_input: String,
}

#[derive(Debug, Clone, PartialEq)]
enum AttachMode {
    Process,
    GdbServer,
}

impl Default for KatoriApp {
    fn default() -> Self {
        Self {
            gdb_adapter: GdbAdapter::new(),
            is_debugging: false,
            is_attached: false,
            current_pid: None,
            current_host_port: "localhost:1234".to_string(),
            attach_mode: AttachMode::GdbServer,
            console_output: "Welcome to Katori GDB Frontend\n".to_string(),
            error_message: String::new(),
            registers: Vec::new(),
            assembly_lines: Vec::new(),
            stack_frames: Vec::new(),
            breakpoints: Vec::new(),
            show_registers: true,
            show_assembly: true,
            show_stack: true,
            show_memory: false,
            show_console: true,
            memory_address: "0x0".to_string(),
            memory_size: 256,
            memory_data: Vec::new(),
            breakpoint_input: String::new(),
            pid_input: String::new(),
        }
    }
}

impl KatoriApp {
    fn start_gdb_session(&mut self) {
        self.is_debugging = true;
        self.console_output.push_str("GDB session started\n");
    }
    
    fn stop_gdb_session(&mut self) {
        self.is_debugging = false;
        self.is_attached = false;
        self.console_output.push_str("GDB session stopped\n");
    }
    
    fn attach_to_target(&mut self) {
        match self.attach_mode {
            AttachMode::Process => {
                if let Ok(pid) = self.pid_input.parse::<u32>() {
                    self.current_pid = Some(pid);
                    self.is_attached = true;
                    self.console_output.push_str(&format!("Attached to process {}\n", pid));
                }
            }
            AttachMode::GdbServer => {
                self.is_attached = true;
                self.console_output.push_str(&format!("Attached to GDB server at {}\n", self.current_host_port));
            }
        }
    }
    
    fn detach_from_target(&mut self) {
        self.is_attached = false;
        self.current_pid = None;
        self.console_output.push_str("Detached from target\n");
    }
    
    fn set_breakpoint(&mut self) {
        if !self.breakpoint_input.is_empty() {
            self.breakpoints.push(self.breakpoint_input.clone());
            self.console_output.push_str(&format!("Set breakpoint at: {}\n", self.breakpoint_input));
            self.breakpoint_input.clear();
        }
    }
    
    fn continue_execution(&mut self) {
        self.console_output.push_str("Continuing execution...\n");
    }
    
    fn step_over(&mut self) {
        self.console_output.push_str("Step over\n");
    }
    
    fn step_into(&mut self) {
        self.console_output.push_str("Step into\n");
    }
    
    fn step_out(&mut self) {
        self.console_output.push_str("Step out\n");
    }
    
    fn interrupt_execution(&mut self) {
        self.console_output.push_str("Interrupting execution...\n");
    }
    
    fn refresh_debug_info(&mut self) {
        // Placeholder for refreshing registers, assembly, stack frames
        self.console_output.push_str("Refreshing debug information...\n");
    }
}

impl eframe::App for KatoriApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Menu bar
        egui::TopBottomPanel::top("menubar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Exit").clicked() {
                        std::process::exit(0);
                    }
                });
                
                ui.menu_button("Debug", |ui| {
                    if ui.button("Start Session").clicked() {
                        self.start_gdb_session();
                    }
                    if ui.button("Stop Session").clicked() {
                        self.stop_gdb_session();
                    }
                    ui.separator();
                    if ui.button("Attach").clicked() {
                        self.attach_to_target();
                    }
                    if ui.button("Detach").clicked() {
                        self.detach_from_target();
                    }
                });
                
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_registers, "Registers");
                    ui.checkbox(&mut self.show_assembly, "Assembly");
                    ui.checkbox(&mut self.show_stack, "Stack");
                    ui.checkbox(&mut self.show_memory, "Memory");
                    ui.checkbox(&mut self.show_console, "Console");
                });
            });
        });
        
        // Toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Debug control buttons
                ui.separator();
                if ui.button("▶ Continue").clicked() {
                    self.continue_execution();
                }
                if ui.button("⏸ Break").clicked() {
                    self.interrupt_execution();
                }
                ui.separator();
                if ui.button("⬇ Step Into").clicked() {
                    self.step_into();
                }
                if ui.button("➡ Step Over").clicked() {
                    self.step_over();
                }
                if ui.button("⬆ Step Out").clicked() {
                    self.step_out();
                }
                ui.separator();
                if ui.button("🔄 Refresh").clicked() {
                    self.refresh_debug_info();
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(if self.is_debugging {
                        if self.is_attached {
                            "🔗 Attached"
                        } else {
                            "🔴 Debugging"
                        }
                    } else {
                        "⭕ Ready"
                    });
                });
            });
        });
        
        // Attach panel
        egui::TopBottomPanel::top("attach_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Attach to:");
                ui.selectable_value(&mut self.attach_mode, AttachMode::GdbServer, "GDB Server");
                ui.selectable_value(&mut self.attach_mode, AttachMode::Process, "Process");
                
                match self.attach_mode {
                    AttachMode::GdbServer => {
                        ui.label("Host:Port:");
                        ui.text_edit_singleline(&mut self.current_host_port);
                    }
                    AttachMode::Process => {
                        ui.label("PID:");
                        ui.text_edit_singleline(&mut self.pid_input);
                    }
                }
                
                if ui.button("Attach").clicked() {
                    self.attach_to_target();
                }
                
                if self.is_attached && ui.button("Detach").clicked() {
                    self.detach_from_target();
                }
            });
        });
        
        // Breakpoint panel
        egui::TopBottomPanel::top("breakpoint_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Breakpoint:");
                ui.text_edit_singleline(&mut self.breakpoint_input);
                if ui.button("Add").clicked() {
                    self.set_breakpoint();
                }
                
                ui.separator();
                ui.label("Breakpoints:");
                for (i, bp) in self.breakpoints.iter().enumerate() {
                    ui.label(format!("#{} {}", i + 1, bp));
                }
            });
        });
        
        // Error message panel
        if !self.error_message.is_empty() {
            egui::TopBottomPanel::top("error_panel").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::RED, &self.error_message);
                    if ui.button("✕").clicked() {
                        self.error_message.clear();
                    }
                });
            });
        }
        
        // Console at bottom
        if self.show_console {
            egui::TopBottomPanel::bottom("console").min_height(150.0).show(ctx, |ui| {
                ui.label("Console Output:");
                egui::ScrollArea::vertical()
                    .id_source("console_scroll")
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.monospace(&self.console_output);
                    });
            });
        }
        
        // Main content area with panels
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Left panel - Assembly
                if self.show_assembly {
                    ui.vertical(|ui| {
                        ui.heading("Assembly");
                        egui::ScrollArea::vertical()
                            .id_source("assembly_scroll")
                            .show(ui, |ui| {
                                if self.assembly_lines.is_empty() {
                                    ui.label("No assembly data");
                                } else {
                                    for line in &self.assembly_lines {
                                        ui.monospace(format!("{}: {}", line.address, line.instruction));
                                    }
                                }
                            });
                    });
                    ui.separator();
                }
                
                // Middle panel - Registers
                if self.show_registers {
                    ui.vertical(|ui| {
                        ui.heading("Registers");
                        egui::ScrollArea::vertical()
                            .id_source("registers_scroll")
                            .show(ui, |ui| {
                                if self.registers.is_empty() {
                                    ui.label("No register data");
                                } else {
                                    for reg in &self.registers {
                                        ui.monospace(format!("{}: {}", reg.name, reg.value));
                                    }
                                }
                            });
                    });
                    ui.separator();
                }
                
                // Right panel - Stack
                if self.show_stack {
                    ui.vertical(|ui| {
                        ui.heading("Stack Frames");
                        egui::ScrollArea::vertical()
                            .id_source("stack_scroll")
                            .show(ui, |ui| {
                                if self.stack_frames.is_empty() {
                                    ui.label("No stack data");
                                } else {
                                    for frame in &self.stack_frames {
                                        let display = if let Some(func) = &frame.function {
                                            format!("#{} {} @ {}", frame.level, func, frame.address)
                                        } else {
                                            format!("#{} @ {}", frame.level, frame.address)
                                        };
                                        ui.monospace(display);
                                    }
                                }
                            });
                    });
                }
            });
            
            // Memory viewer (if enabled)
            if self.show_memory {
                ui.separator();
                ui.heading("Memory Viewer");
                ui.horizontal(|ui| {
                    ui.label("Address:");
                    ui.text_edit_singleline(&mut self.memory_address);
                    ui.label("Size:");
                    ui.add(egui::DragValue::new(&mut self.memory_size).speed(1.0));
                    if ui.button("Read").clicked() {
                        // Placeholder for memory reading
                        self.console_output.push_str(&format!("Reading {} bytes from {}\n", self.memory_size, self.memory_address));
                    }
                });
                
                // Memory display
                egui::ScrollArea::vertical()
                    .id_source("memory_scroll")
                    .show(ui, |ui| {
                        if self.memory_data.is_empty() {
                            ui.label("No memory data");
                        } else {
                            for (i, chunk) in self.memory_data.chunks(16).enumerate() {
                                let mut hex_str = String::new();
                                let mut ascii_str = String::new();
                                
                                for byte in chunk {
                                    hex_str.push_str(&format!("{:02x} ", byte));
                                    if byte.is_ascii_graphic() {
                                        ascii_str.push(*byte as char);
                                    } else {
                                        ascii_str.push('.');
                                    }
                                }
                                
                                ui.monospace(format!("{:08x}: {:<48} {}", i * 16, hex_str, ascii_str));
                            }
                        }
                    });
            }
        });
    }
}
