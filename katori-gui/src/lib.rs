use eframe::egui;
use gdbadapter::{GdbAdapter, GdbEvent, Register, AssemblyLine, StackFrame, StreamType};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

pub fn run_gui() -> i32 {
    // Create a tokio runtime for async operations
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = rt.enter();
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Katori - GDB Frontend"),
        ..Default::default()
    };
    
    match eframe::run_native(
        "Katori",
        options,
        Box::new(|_cc| Box::new(KatoriApp::new())),
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
    gdb_adapter: Arc<Mutex<GdbAdapter>>,
    
    /// Event channel for GDB events
    gdb_event_receiver: Option<mpsc::UnboundedReceiver<GdbEvent>>,
    
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

impl KatoriApp {
    fn new() -> Self {
        let gdb_adapter = Arc::new(Mutex::new(GdbAdapter::new()));
        
        Self {
            gdb_adapter,
            gdb_event_receiver: None,
            is_debugging: false,
            is_attached: false,
            current_pid: None,
            current_host_port: "localhost:1337".to_string(),
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
    
    fn start_gdb_session(&mut self) {
        self.is_debugging = true;
        self.console_output.push_str("Starting GDB session...\n");
        
        let adapter = self.gdb_adapter.clone();
        tokio::spawn(async move {
            let mut adapter = adapter.lock().await;
            if let Err(e) = adapter.start_session().await {
                eprintln!("Failed to start GDB: {}", e);
            }
        });
    }
    
    fn stop_gdb_session(&mut self) {
        self.is_debugging = false;
        self.is_attached = false;
        self.console_output.push_str("Stopping GDB session...\n");
        
        let adapter = self.gdb_adapter.clone();
        tokio::spawn(async move {
            let mut adapter = adapter.lock().await;
            let _ = adapter.stop_session().await;
        });
    }
    
    fn attach_to_target(&mut self) {
        match self.attach_mode {
            AttachMode::Process => {
                if let Ok(pid) = self.pid_input.parse::<u32>() {
                    self.current_pid = Some(pid);
                    self.is_attached = true;
                    self.console_output.push_str(&format!("Attaching to process {}...\n", pid));
                    
                    let adapter = self.gdb_adapter.clone();
                    tokio::spawn(async move {
                        let mut adapter = adapter.lock().await;
                        if let Err(e) = adapter.attach_to_process(pid).await {
                            eprintln!("Failed to attach to process: {}", e);
                        }
                    });
                }
            }
            AttachMode::GdbServer => {
                self.is_attached = true;
                self.console_output.push_str(&format!("Attaching to GDB server at {}...\n", self.current_host_port));
                
                let host_port = self.current_host_port.clone();
                let adapter = self.gdb_adapter.clone();
                tokio::spawn(async move {
                    let mut adapter = adapter.lock().await;
                    if let Err(e) = adapter.attach_to_gdbserver(&host_port).await {
                        eprintln!("Failed to connect to GDB server: {}", e);
                    }
                });
            }
        }
    }
    
    fn detach_from_target(&mut self) {
        self.is_attached = false;
        self.current_pid = None;
        self.console_output.push_str("Detaching from target...\n");
        
        let adapter = self.gdb_adapter.clone();
        tokio::spawn(async move {
            let mut adapter = adapter.lock().await;
            let _ = adapter.detach().await;
        });
    }
    
    fn set_breakpoint(&mut self) {
        if !self.breakpoint_input.is_empty() {
            self.breakpoints.push(self.breakpoint_input.clone());
            self.console_output.push_str(&format!("Setting breakpoint at: {}\n", self.breakpoint_input));
            
            let location = self.breakpoint_input.clone();
            let adapter = self.gdb_adapter.clone();
            tokio::spawn(async move {
                let mut adapter = adapter.lock().await;
                if let Err(e) = adapter.set_breakpoint(&location).await {
                    eprintln!("Failed to set breakpoint: {}", e);
                }
            });
            
            self.breakpoint_input.clear();
        }
    }
    
    fn continue_execution(&mut self) {
        self.console_output.push_str("Continuing execution...\n");
        
        let adapter = self.gdb_adapter.clone();
        tokio::spawn(async move {
            let mut adapter = adapter.lock().await;
            if let Err(e) = adapter.continue_execution().await {
                eprintln!("Failed to continue execution: {}", e);
            }
        });
    }
    
    fn step_over(&mut self) {
        self.console_output.push_str("Step over\n");
        
        let adapter = self.gdb_adapter.clone();
        tokio::spawn(async move {
            let mut adapter = adapter.lock().await;
            if let Err(e) = adapter.next().await {
                eprintln!("Failed to step over: {}", e);
            }
        });
    }
    
    fn step_into(&mut self) {
        self.console_output.push_str("Step into\n");
        
        let adapter = self.gdb_adapter.clone();
        tokio::spawn(async move {
            let mut adapter = adapter.lock().await;
            if let Err(e) = adapter.step().await {
                eprintln!("Failed to step into: {}", e);
            }
        });
    }
    
    fn step_out(&mut self) {
        self.console_output.push_str("Step out\n");
        
        let adapter = self.gdb_adapter.clone();
        tokio::spawn(async move {
            let mut adapter = adapter.lock().await;
            if let Err(e) = adapter.step_out().await {
                eprintln!("Failed to step out: {}", e);
            }
        });
    }
    
    fn interrupt_execution(&mut self) {
        self.console_output.push_str("Interrupting execution...\n");
        
        let adapter = self.gdb_adapter.clone();
        tokio::spawn(async move {
            let mut adapter = adapter.lock().await;
            if let Err(e) = adapter.interrupt().await {
                eprintln!("Failed to interrupt execution: {}", e);
            }
        });
    }
    
    fn refresh_debug_info(&mut self) {
        self.console_output.push_str("Refreshing debug information...\n");
        self.auto_refresh_debug_info();
    }
    
    /// Process pending GDB events and update UI state
    fn process_gdb_events(&mut self) {
        let mut should_refresh = false;
        
        // Use try_lock to avoid blocking if the adapter is busy
        if let Ok(adapter) = self.gdb_adapter.try_lock() {
            // Try to get events without blocking
            while let Some(event) = adapter.try_recv_event() {
                match event {
                    GdbEvent::Stream(stream) => {
                        match stream.stream_type {
                            StreamType::Console => {
                                self.console_output.push_str(&format!("GDB: {}\n", stream.content));
                            }
                            StreamType::Target => {
                                self.console_output.push_str(&format!("Target: {}\n", stream.content));
                            }
                            StreamType::Log => {
                                self.console_output.push_str(&format!("{}\n", stream.content));
                            }
                        }
                    }
                    GdbEvent::Result(result) => {
                        // Handle result events if needed
                        self.console_output.push_str(&format!("GDB Result: {:?}\n", result.class));
                    }
                    GdbEvent::Async(async_record) => {
                        // Handle async events - check if we should auto-refresh
                        match async_record.class {
                            gdbadapter::AsyncClass::Stopped => {
                                self.console_output.push_str("Target stopped - refreshing debug info\n");
                                should_refresh = true;
                            }
                            gdbadapter::AsyncClass::Running => {
                                self.console_output.push_str("Target running\n");
                            }
                            _ => {
                                self.console_output.push_str(&format!("GDB Async: {:?}\n", async_record.class));
                            }
                        }
                    }
                }
            }
        }
        
        // Auto-refresh debug info if target stopped
        if should_refresh {
            self.auto_refresh_debug_info();
        }
    }
    
    fn read_memory(&mut self) {
        self.console_output.push_str(&format!("Reading {} bytes from {}\n", self.memory_size, self.memory_address));
        
        let address = self.memory_address.clone();
        let size = self.memory_size;
        let adapter = self.gdb_adapter.clone();
        tokio::spawn(async move {
            let mut adapter = adapter.lock().await;
            if let Err(e) = adapter.read_memory(&address, size).await {
                eprintln!("Failed to read memory: {}", e);
            }
        });
    }
    
    /// Automatically fetch debug information when GDB is stopped
    fn auto_refresh_debug_info(&mut self) {
        if !self.is_debugging || !self.is_attached {
            return;
        }
        
        let adapter = self.gdb_adapter.clone();
        tokio::spawn(async move {
            let mut adapter = adapter.lock().await;
            
            // Get registers
            if let Ok(_result) = adapter.get_registers().await {
                // TODO: Parse and update registers in GUI
            }
            
            // Get stack frames
            if let Ok(_result) = adapter.get_stack_frames().await {
                // TODO: Parse and update stack frames in GUI
            }
            
            // Get assembly around current PC
            if let Ok(_result) = adapter.disassemble_current(20).await {
                // TODO: Parse and update assembly in GUI
            }
        });
    }
}

impl eframe::App for KatoriApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process GDB events to update console output
        self.process_gdb_events();
        
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
        
        // Main content area - Assembly in center with right sidebar
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Main assembly panel (takes most of the space)
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width() * 0.7, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        if self.show_assembly {
                            ui.heading("Assembly");
                            egui::ScrollArea::vertical()
                                .id_source("assembly_scroll")
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    if self.assembly_lines.is_empty() {
                                        ui.centered_and_justified(|ui| {
                                            ui.label("No assembly data available");
                                        });
                                    } else {
                                        for line in &self.assembly_lines {
                                            ui.monospace(format!("0x{}: {}", line.address, line.instruction));
                                        }
                                    }
                                });
                        } else {
                            ui.centered_and_justified(|ui| {
                                ui.label("Assembly view disabled");
                            });
                        }
                    }
                );
                
                ui.separator();
                
                // Right sidebar for registers and stack (takes remaining space)
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width(), ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        // Registers panel (top half of sidebar)
                        if self.show_registers {
                            ui.allocate_ui_with_layout(
                                egui::Vec2::new(ui.available_width(), ui.available_height() * 0.5),
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    ui.heading("Registers");
                                    egui::ScrollArea::vertical()
                                        .id_source("registers_scroll")
                                        .auto_shrink([false, false])
                                        .show(ui, |ui| {
                                            if self.registers.is_empty() {
                                                ui.label("No register data");
                                            } else {
                                                for reg in &self.registers {
                                                    ui.horizontal(|ui| {
                                                        ui.monospace(format!("{:8}", reg.name));
                                                        ui.monospace(&reg.value);
                                                    });
                                                }
                                            }
                                        });
                                }
                            );
                        }
                        
                        ui.separator();
                        
                        // Stack frames panel (bottom half of sidebar)
                        if self.show_stack {
                            ui.allocate_ui_with_layout(
                                egui::Vec2::new(ui.available_width(), ui.available_height()),
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    ui.heading("Stack Frames");
                                    egui::ScrollArea::vertical()
                                        .id_source("stack_scroll")
                                        .auto_shrink([false, false])
                                        .show(ui, |ui| {
                                            if self.stack_frames.is_empty() {
                                                ui.label("No stack data");
                                            } else {
                                                for frame in &self.stack_frames {
                                                    let display = if let Some(func) = &frame.function {
                                                        format!("#{} {} @ 0x{}", frame.level, func, frame.address)
                                                    } else {
                                                        format!("#{} @ 0x{}", frame.level, frame.address)
                                                    };
                                                    ui.monospace(display);
                                                }
                                            }
                                        });
                                }
                            );
                        }
                    }
                );
            });
            
            // Memory viewer (if enabled) - separate section at the bottom
            if self.show_memory {
                ui.separator();
                ui.heading("Memory Viewer");
                ui.horizontal(|ui| {
                    ui.label("Address:");
                    ui.text_edit_singleline(&mut self.memory_address);
                    ui.label("Size:");
                    ui.add(egui::DragValue::new(&mut self.memory_size).speed(1.0));
                    if ui.button("Read").clicked() {
                        self.read_memory();
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
