use eframe::egui;
use gdbadapter::{GdbAdapter, Register, AssemblyLine, StackFrame, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

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
pub enum AttachMode {
    Process,
    GdbServer,
}

impl KatoriApp {
    pub fn new() -> Self {
        let gdb_adapter = Arc::new(Mutex::new(GdbAdapter::new()));
        
        Self {
            gdb_adapter,
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

    // Public methods for testing
    pub fn is_debugging(&self) -> bool {
        self.is_debugging
    }

    pub fn is_attached(&self) -> bool {
        self.is_attached
    }

    pub fn get_host_port(&self) -> &str {
        &self.current_host_port
    }

    pub fn get_console_output(&self) -> &str {
        &self.console_output
    }

    pub fn add_console_message(&mut self, message: &str) {
        self.console_output.push_str(message);
        self.console_output.push('\n');
    }

    pub fn get_attach_mode(&self) -> &AttachMode {
        &self.attach_mode
    }

    pub fn set_attach_mode(&mut self, mode: AttachMode) {
        self.attach_mode = mode;
    }

    pub fn get_breakpoints(&self) -> &Vec<String> {
        &self.breakpoints
    }

    pub fn get_breakpoint_input(&self) -> &str {
        &self.breakpoint_input
    }

    pub fn set_breakpoint_input(&mut self, input: String) {
        self.breakpoint_input = input;
    }

    pub fn add_breakpoint_from_input(&mut self) {
        if !self.breakpoint_input.is_empty() {
            self.breakpoints.push(self.breakpoint_input.clone());
            self.breakpoint_input.clear();
        }
    }

    pub fn get_error_message(&self) -> &str {
        &self.error_message
    }

    pub fn set_error_message(&mut self, message: String) {
        self.error_message = message;
    }

    pub fn clear_error_message(&mut self) {
        self.error_message.clear();
    }

    pub fn get_registers(&self) -> &Vec<Register> {
        &self.registers
    }

    pub fn get_assembly_lines(&self) -> &Vec<AssemblyLine> {
        &self.assembly_lines
    }

    pub fn get_stack_frames(&self) -> &Vec<StackFrame> {
        &self.stack_frames
    }

    pub fn clear_debug_info(&mut self) {
        self.registers.clear();
        self.assembly_lines.clear();
        self.stack_frames.clear();
    }

    pub fn is_registers_visible(&self) -> bool {
        self.show_registers
    }

    pub fn set_registers_visible(&mut self, visible: bool) {
        self.show_registers = visible;
    }

    pub fn is_assembly_visible(&self) -> bool {
        self.show_assembly
    }

    pub fn is_stack_visible(&self) -> bool {
        self.show_stack
    }

    pub fn is_memory_visible(&self) -> bool {
        self.show_memory
    }

    pub fn set_memory_visible(&mut self, visible: bool) {
        self.show_memory = visible;
    }

    pub fn is_console_visible(&self) -> bool {
        self.show_console
    }

    // Async version for testing
    pub async fn start_gdb_session_async(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut adapter = self.gdb_adapter.lock().await;
        adapter.start_session().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
    
    pub fn start_gdb_session(&mut self) {
        self.console_output.push_str("Starting GDB session...\n");
        self.is_debugging = true;
        
        let adapter = self.gdb_adapter.clone();
        let console_sender = self.create_console_sender();
        
        tokio::spawn(async move {
            let mut adapter = adapter.lock().await;
            match adapter.start_session().await {
                Ok(_) => {
                    console_sender.send("GDB session started successfully\n".to_string()).ok();
                }
                Err(e) => {
                    console_sender.send(format!("Failed to start GDB: {}\n", e)).ok();
                }
            }
        });
    }

    pub fn stop_gdb_session(&mut self) {
        self.console_output.push_str("Stopping GDB session...\n");
        
        let adapter = self.gdb_adapter.clone();
        let console_sender = self.create_console_sender();
        
        tokio::spawn(async move {
            let mut adapter = adapter.lock().await;
            match adapter.stop_session().await {
                Ok(_) => {
                    console_sender.send("GDB session stopped\n".to_string()).ok();
                }
                Err(e) => {
                    console_sender.send(format!("Failed to stop session: {}\n", e)).ok();
                }
            }
        });
        
        // Update state immediately for UI responsiveness
        self.is_debugging = false;
        self.is_attached = false;
        self.clear_debug_info();
        self.breakpoints.clear();
    }

    // Helper method to create a channel for console updates (simplified for now)
    fn create_console_sender(&self) -> std::sync::mpsc::Sender<String> {
        let (sender, _receiver) = std::sync::mpsc::channel();
        // In a real implementation, you'd store the receiver and process messages in update()
        sender
    }    fn attach_to_target(&mut self) {
        self.console_output.push_str("Starting attachment process...\n");
        
        // Clone data needed for async operations
        let adapter = self.gdb_adapter.clone();
        let attach_mode = self.attach_mode.clone();
        let pid_input = self.pid_input.clone();
        let host_port = self.current_host_port.clone();

        // Show immediate feedback
        match attach_mode {
            AttachMode::Process => {
                if let Ok(pid) = pid_input.parse::<u32>() {
                    self.console_output.push_str(&format!("Attaching to process {}...\n", pid));
                } else {
                    self.console_output.push_str("Invalid PID format\n");
                    self.error_message = "Invalid PID format".to_string();
                    return;
                }
            }
            AttachMode::GdbServer => {
                self.console_output.push_str(&format!("Attaching to GDB server at {}...\n", host_port));
            }
        }

        // Start async attachment
        tokio::spawn(async move {
            // Start GDB session first if not already running
            let start_result = {
                let mut adapter = adapter.lock().await;
                if !adapter.is_running() {
                    adapter.start_session().await
                } else {
                    Ok(())
                }
            };
            
            if let Err(e) = start_result {
                eprintln!("Failed to start GDB: {}", e);
                return;
            }

            match attach_mode {
                AttachMode::Process => {
                    if let Ok(pid) = pid_input.parse::<u32>() {
                        let result = {
                            let mut adapter = adapter.lock().await;
                            adapter.attach_to_process(pid).await
                        };
                        
                        match result {
                            Ok(_) => {
                                println!("Successfully attached to process {}", pid);
                            }
                            Err(e) => {
                                eprintln!("Failed to attach to process: {}", e);
                            }
                        }
                    }
                }
                AttachMode::GdbServer => {
                    let result = {
                        let mut adapter = adapter.lock().await;
                        adapter.attach_to_gdbserver(&host_port).await
                    };
                    
                    match result {
                        Ok(_) => {
                            println!("Successfully attached to GDB server at {}", host_port);
                        }
                        Err(e) => {
                            eprintln!("Failed to connect to GDB server: {}", e);
                        }
                    }
                }
            }
        });
    }
    
    fn detach_from_target(&mut self) {
        self.console_output.push_str("Detaching from target...\n");
        
        let adapter = self.gdb_adapter.clone();
        
        tokio::spawn(async move {
            let result = {
                let mut adapter = adapter.lock().await;
                adapter.detach().await
            };
            
            match result {
                Ok(_) => {
                    println!("Successfully detached from target");
                }
                Err(e) => {
                    eprintln!("Failed to detach: {}", e);
                }
            }
        });
        
        // Clear state immediately
        self.is_attached = false;
        self.is_debugging = false;
        self.current_pid = None;
        self.registers.clear();
        self.assembly_lines.clear();
        self.stack_frames.clear();
    }
    
    fn set_breakpoint(&mut self) {
        if !self.breakpoint_input.is_empty() {
            self.console_output.push_str(&format!("Setting breakpoint at: {}\n", self.breakpoint_input));
            
            let location = self.breakpoint_input.clone();
            let adapter = self.gdb_adapter.clone();
            
            // Add to breakpoints list immediately for UI feedback
            self.breakpoints.push(location.clone());
            self.breakpoint_input.clear();
            
            tokio::spawn(async move {
                match {
                    let mut adapter = adapter.lock().await;
                    adapter.set_breakpoint(&location).await
                } {
                    Ok(_) => {
                        println!("Breakpoint set successfully at {}", location);
                    }
                    Err(e) => {
                        eprintln!("Failed to set breakpoint: {}", e);
                    }
                }
            });
        }
    }
    
    fn continue_execution(&mut self) {
        self.console_output.push_str("Continuing execution...\n");
        
        let adapter = self.gdb_adapter.clone();
        
        tokio::spawn(async move {
            match {
                let mut adapter = adapter.lock().await;
                adapter.continue_execution().await
            } {
                Ok(_) => {
                    println!("Execution continued");
                }
                Err(e) => {
                    eprintln!("Failed to continue execution: {}", e);
                }
            }
        });
    }
    
    fn step_over(&mut self) {
        self.console_output.push_str("Step over\n");
        
        let adapter = self.gdb_adapter.clone();
        
        tokio::spawn(async move {
            match {
                let mut adapter = adapter.lock().await;
                adapter.next().await
            } {
                Ok(_) => {
                    println!("Step completed");
                }
                Err(e) => {
                    eprintln!("Failed to step over: {}", e);
                }
            }
        });
    }
    
    fn step_into(&mut self) {
        self.console_output.push_str("Step into\n");
        
        let adapter = self.gdb_adapter.clone();
        
        tokio::spawn(async move {
            match {
                let mut adapter = adapter.lock().await;
                adapter.step().await
            } {
                Ok(_) => {
                    println!("Step completed");
                }
                Err(e) => {
                    eprintln!("Failed to step into: {}", e);
                }
            }
        });
    }
    
    fn step_out(&mut self) {
        self.console_output.push_str("Step out\n");
        
        let adapter = self.gdb_adapter.clone();
        
        tokio::spawn(async move {
            match {
                let mut adapter = adapter.lock().await;
                adapter.step_out().await
            } {
                Ok(_) => {
                    println!("Step completed");
                }
                Err(e) => {
                    eprintln!("Failed to step out: {}", e);
                }
            }
        });
    }
    
    fn interrupt_execution(&mut self) {
        self.console_output.push_str("Interrupting execution...\n");
        
        let adapter = self.gdb_adapter.clone();
        
        tokio::spawn(async move {
            match {
                let mut adapter = adapter.lock().await;
                adapter.interrupt().await
            } {
                Ok(_) => {
                    println!("Execution interrupted");
                }
                Err(e) => {
                    eprintln!("Failed to interrupt execution: {}", e);
                }
            }
        });
    }
    
    fn refresh_debug_info(&mut self) {
        self.console_output.push_str("Refreshing debug information...\n");
        self.auto_refresh_debug_info();
    }
    
    fn read_memory(&mut self) {
        self.console_output.push_str(&format!("Reading {} bytes from {}\n", self.memory_size, self.memory_address));
        
        let address = self.memory_address.clone();
        let size = self.memory_size;
        let adapter = self.gdb_adapter.clone();
        
        tokio::spawn(async move {
            match {
                let mut adapter = adapter.lock().await;
                adapter.read_memory(&address, size).await
            } {
                Ok(result) => {
                    println!("Memory read successfully: {:?}", result);
                }
                Err(e) => {
                    eprintln!("Failed to read memory: {}", e);
                }
            }
        });
    }
    
    /// Automatically fetch debug information when GDB is stopped
    fn auto_refresh_debug_info(&mut self) {
        if !self.is_debugging || !self.is_attached {
            return;
        }
        
        self.console_output.push_str("Refreshing debug information...\n");
        
        let adapter = self.gdb_adapter.clone();
        
        // Start async refresh in background
        tokio::spawn(async move {
            let mut adapter = adapter.lock().await;
            
            // Get register names first, then register values
            let mut register_names = Vec::new();
            if let Ok(names_result) = adapter.get_register_names().await {
                println!("Register names result: {:?}", names_result);
                // Extract names from the result
                if let Some(Value::List(names_list)) = names_result.results.get("register-names") {
                    for (i, name_value) in names_list.iter().enumerate() {
                        if let Some(name) = name_value.as_string() {
                            register_names.push((i, name.to_string()));
                        }
                    }
                }
            }
            
            // Get registers
            if let Ok(result) = adapter.get_registers().await {
                println!("Register result: {:?}", result);
            }
            
            // Get stack frames
            if let Ok(result) = adapter.get_stack_frames().await {
                println!("Stack frames result: {:?}", result);
            }
            
            // Get assembly around current PC
            if let Ok(result) = adapter.disassemble_current(20).await {
                println!("Disassembly result: {:?}", result);
            }
            
            println!("Debug info refresh completed");
        });
    }
    
    /// Parse register values from GDB/MI result
    fn parse_registers(result: &gdbadapter::GdbResult, register_names: &[(usize, String)]) -> Option<Vec<Register>> {
        // GDB/MI uses "register-values" field for -data-list-register-values
        if let Some(Value::List(register_list)) = result.results.get("register-values") {
            let mut registers = Vec::new();
            
            for reg_value in register_list {
                if let Some(reg_tuple) = reg_value.as_tuple() {
                    let number = reg_tuple.get("number")?.as_string()?.parse().ok()?;
                    let value = reg_tuple.get("value")?.as_string()?.to_string();
                    
                    // Use the actual register name if available, otherwise use a generic name
                    let name = register_names.iter()
                        .find(|(i, _)| *i == number as usize)
                        .map(|(_, name)| name.clone())
                        .unwrap_or_else(|| format!("r{}", number));
                    
                    registers.push(Register {
                        number,
                        name,
                        value,
                    });
                }
            }
            
            Some(registers)
        } else {
            // Check if there's a different structure
            println!("Available register result keys: {:?}", result.results.keys().collect::<Vec<_>>());
            None
        }
    }
    
    /// Parse stack frames from GDB/MI result
    fn parse_stack_frames(result: &gdbadapter::GdbResult) -> Option<Vec<StackFrame>> {
        // GDB/MI uses "stack" field for -stack-list-frames
        if let Some(Value::List(frame_list)) = result.results.get("stack") {
            let mut frames = Vec::new();
            
            for frame_value in frame_list {
                if let Some(frame_tuple) = frame_value.as_tuple() {
                    let level = frame_tuple.get("level")?.as_string()?.parse().ok()?;
                    let address = frame_tuple.get("addr")?.as_string()?.to_string();
                    let function = frame_tuple.get("func").and_then(|v| v.as_string()).map(|s| s.to_string());
                    let file = frame_tuple.get("file").and_then(|v| v.as_string()).map(|s| s.to_string());
                    let fullname = frame_tuple.get("fullname").and_then(|v| v.as_string()).map(|s| s.to_string());
                    let line = frame_tuple.get("line").and_then(|v| v.as_string()).and_then(|s| s.parse().ok());
                    
                    frames.push(StackFrame {
                        level,
                        address,
                        function,
                        file,
                        fullname,
                        line,
                        arch: None,
                    });
                }
            }
            
            Some(frames)
        } else {
            // Check if there's a different structure
            println!("Available stack result keys: {:?}", result.results.keys().collect::<Vec<_>>());
            None
        }
    }
    
    /// Parse assembly instructions from GDB/MI result
    fn parse_assembly(result: &gdbadapter::GdbResult) -> Option<Vec<AssemblyLine>> {
        // GDB/MI uses "asm_insns" field for -data-disassemble
        if let Some(Value::List(asm_list)) = result.results.get("asm_insns") {
            let mut assembly = Vec::new();
            
            for asm_value in asm_list {
                if let Some(asm_tuple) = asm_value.as_tuple() {
                    let address = asm_tuple.get("address")?.as_string()?.to_string();
                    let instruction = asm_tuple.get("inst")?.as_string()?.to_string();
                    let function = asm_tuple.get("func-name").and_then(|v| v.as_string()).map(|s| s.to_string());
                    let offset = asm_tuple.get("offset").and_then(|v| v.as_string()).and_then(|s| s.parse().ok());
                    let opcodes = asm_tuple.get("opcodes").and_then(|v| v.as_string()).map(|s| s.to_string());
                    
                    assembly.push(AssemblyLine {
                        address,
                        function,
                        offset,
                        instruction,
                        opcodes,
                    });
                }
            }
            
            Some(assembly)
        } else {
            // Check if there's a different structure
            println!("Available assembly result keys: {:?}", result.results.keys().collect::<Vec<_>>());
            None
        }
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
