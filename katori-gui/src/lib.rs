use eframe::{egui, CreationContext};
use gdbadapter::{AssemblyLine, GdbAdapter, GdbEvent, Register, StackFrame, Value};
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, warn, error, debug};

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
        Box::new(|cc| Ok(Box::new(KatoriApp::new(cc)))),
    ) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("Error running GUI: {e}");
            1
        }
    }
}

#[derive(Debug, Clone)]
enum GdbCommand {
    Continue,
    StepOver,
    StepInto,
    StepOut,
    Interrupt,
    SetBreakpoint(String),
    RefreshDebugInfo,
    ReadMemory(String, u32),
    // Session management commands
    StartSession,
    StopSession,
    Attach(AttachMode, String), // mode and target (PID or host:port)
    Detach,
}

#[derive(Debug)]
enum DebugEvent {
    RegistersUpdated(Vec<Register>),
    StackFramesUpdated(Vec<StackFrame>),
    AssemblyUpdated(Vec<AssemblyLine>),
    ConsoleMessage(String),
    AttachSuccess(Option<u32>), // PID for process attach, None for gdbserver
    AttachFailed(String),
    DetachSuccess,
    MemoryRead(Vec<u8>),
    MemoryReadFailed(String),
    // Command completion events
    CommandCompleted(GdbCommand),
    CommandFailed(GdbCommand, String),
    GdbConnectionLost,
    TargetStateChanged(TargetState),
}

#[derive(Debug, Clone, PartialEq)]
enum TargetState {
    Running,
    Stopped,
    Detached,
}

/// Main application state
pub struct KatoriApp {
    /// GDB adapter instance
    gdb_adapter: Arc<Mutex<GdbAdapter>>,

    syntax_set: syntect::parsing::SyntaxSet,
    
    /// Event communication
    event_receiver: std::sync::mpsc::Receiver<DebugEvent>,
    event_sender: std::sync::mpsc::Sender<DebugEvent>,
    
    /// Command channel for async GDB operations from GUI
    command_sender: std::sync::mpsc::Sender<GdbCommand>,
    
    /// Debug session state
    is_debugging: bool,
    is_attached: bool,
    current_pid: Option<u32>,
    current_host_port: String,
    target_state: TargetState,
    
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
    pub fn new(cc: &CreationContext) -> Self {
        let gdb_adapter = Arc::new(Mutex::new(GdbAdapter::new()));
        let (event_sender, event_receiver) = std::sync::mpsc::channel();
        let (command_sender, command_receiver) = std::sync::mpsc::channel();
        
        // Start the background command processor
        let adapter_clone = gdb_adapter.clone();
        let event_sender_clone = event_sender.clone();
        let ctx = cc.egui_ctx.clone();
        tokio::spawn(Self::command_processor_task(ctx, adapter_clone, command_receiver, event_sender_clone));

        // Create syntax set

        let mut builder = syntect::parsing::SyntaxSetBuilder::new();

        builder.add_from_folder("syntax", true).unwrap();
        let ps = builder.build();

        for syntax in ps.syntaxes() {
            debug!("Loaded syntax: {}", syntax.name);
        }

        
        Self {
            gdb_adapter,
            syntax_set: ps,
            event_receiver,
            event_sender,
            command_sender,
            is_debugging: false,
            is_attached: false,
            current_pid: None,
            current_host_port: "localhost:1337".to_string(),
            target_state: TargetState::Detached,
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

    /// Background task that processes GDB commands asynchronously
    async fn command_processor_task(
        ctx: egui::Context,
        gdb_adapter: Arc<Mutex<GdbAdapter>>,
        command_receiver: std::sync::mpsc::Receiver<GdbCommand>,
        event_sender: std::sync::mpsc::Sender<DebugEvent>,
    ) {
        info!("Command processor task started");
        
        loop {
            // Use a blocking receiver in a tokio thread to avoid busy waiting
            match command_receiver.recv() {
                Ok(command) => {
                    log::debug!("Command processor received command: {:?}", command);
                    
                    // Process the command with timeout
                    let result = tokio::time::timeout(
                        Self::get_command_timeout(&command),
                        Self::process_command(gdb_adapter.clone(), command.clone(), event_sender.clone())
                    ).await;
                    
                    match result {
                        Ok(Ok(())) => {
                            info!("Command completed successfully: {command:?}");
                            let _ = event_sender.send(DebugEvent::CommandCompleted(command.clone()));
                        }
                        Ok(Err(error)) => {
                            error!("Command failed: {command:?} - {error}");
                            let _ = event_sender.send(DebugEvent::CommandFailed(command, error));
                        }
                        Err(_) => {
                            error!("Command timed out: {command:?}");
                            let _ = event_sender.send(DebugEvent::CommandFailed(
                                command, 
                                "Command timed out".to_string()
                            ));
                        }
                    }
                }
                Err(_) => {
                    // Channel closed, exit the task
                    info!("Command processor task shutting down - channel closed");
                    break;
                }
            }

            while let Ok(event) = gdb_adapter.lock().await.get_event_receiver().lock().unwrap().try_recv() {
                log::debug!("Command processor task received GDB event: {event:?}");
                // Handle the GDB event (e.g., update UI)
                if let GdbEvent::Async(record) = event {
                    log::debug!("Processing async record: {:?}", record);
                }
            }
            ctx.request_repaint(); // Request repaint to update UI with new events
        }
    }
    
    /// Get appropriate timeout for different command types
    fn get_command_timeout(command: &GdbCommand) -> std::time::Duration {
        match command {
            GdbCommand::Continue => std::time::Duration::from_secs(u64::MAX), // Effectively no timeout for continue
            GdbCommand::StepOver | GdbCommand::StepInto | GdbCommand::StepOut => std::time::Duration::from_secs(10),
            GdbCommand::Interrupt => std::time::Duration::from_secs(10),
            GdbCommand::RefreshDebugInfo => std::time::Duration::from_secs(5),
            GdbCommand::SetBreakpoint(_) => std::time::Duration::from_secs(5),
            GdbCommand::ReadMemory(_, _) => std::time::Duration::from_secs(10),
            GdbCommand::StartSession | GdbCommand::StopSession => std::time::Duration::from_secs(15),
            GdbCommand::Attach(_, _) | GdbCommand::Detach => std::time::Duration::from_secs(15),
        }
    }
    
    /// Process a single GDB command
    async fn process_command(
        gdb_adapter: Arc<Mutex<GdbAdapter>>,
        command: GdbCommand,
        event_sender: std::sync::mpsc::Sender<DebugEvent>,
    ) -> Result<(), String> {
        let mut adapter = gdb_adapter.lock().await;
        log::debug!("Processing command: {:?}", command);
        
        match command {
            GdbCommand::Continue => {
                adapter.continue_execution().await
                    .map_err(|e| format!("Continue failed: {e}"))?;
                Ok(())
            }
            GdbCommand::StepOver => {
                adapter.next_instruction().await
                    .map_err(|e| format!("Step over failed: {e}"))?;
                Ok(())
            }
            GdbCommand::StepInto => {
                adapter.step_instruction().await
                    .map_err(|e| format!("Step into failed: {e}"))?;
                Ok(())
            }
            GdbCommand::StepOut => {
                adapter.step_out().await
                    .map_err(|e| format!("Step out failed: {e}"))?;
                Ok(())
            }
            GdbCommand::Interrupt => {
                adapter.interrupt().await
                    .map_err(|e| format!("Interrupt failed: {e}"))?;
                Ok(())
            }
            GdbCommand::SetBreakpoint(location) => {
                adapter.set_breakpoint(&location).await
                    .map_err(|e| format!("Set breakpoint failed: {e}"))?;
                Ok(())
            }
            GdbCommand::RefreshDebugInfo => {
                // This is a special command that sends multiple events
                log::debug!("Refreshing debug info");
                Self::send_refresh_debug_info_internal(adapter, event_sender).await
                    .map_err(|e| format!("RefreshDebugInfo failed: {e}"))?;
                Ok(())
            }
            GdbCommand::ReadMemory(address, size) => {
                match adapter.read_memory(&address, size).await {
                    Ok(result) => {
                        info!("Memory read command completed: {result:?}");
                        // Try to extract memory data from the result
                        // The data-read-memory-bytes command typically returns memory data in the results
                        if let Some(_memory_value) = result.results.get("memory") {
                            // For now, send the raw result - we'll need to parse this properly later
                            // TODO: Parse memory data properly from GDB MI format
                            let dummy_data = vec![0u8; size as usize]; // Placeholder
                            let _ = event_sender.send(DebugEvent::MemoryRead(dummy_data));
                        } else {
                            let _ = event_sender.send(DebugEvent::MemoryReadFailed("No memory data in response".to_string()));
                        }
                    }
                    Err(e) => {
                        error!("Failed to read memory: {e}");
                        let _ = event_sender.send(DebugEvent::MemoryReadFailed(e.to_string()));
                    }
                }
                Ok(())
            }
            GdbCommand::StartSession => {
                adapter.start_session().await
                    .map_err(|e| format!("Start session failed: {e}"))?;
                Ok(())
            }
            GdbCommand::StopSession => {
                adapter.stop_session().await
                    .map_err(|e| format!("Stop session failed: {e}"))?;
                Ok(())
            }
            GdbCommand::Attach(mode, target) => {
                // Start GDB session first if not already running
                if !adapter.is_running() {
                    adapter.start_session().await
                        .map_err(|e| format!("Failed to start GDB: {e}"))?;
                }
                
                match mode {
                    AttachMode::GdbServer => {
                        adapter.attach_to_gdbserver(&target).await
                            .map_err(|e| format!("Attach to GDB server failed: {e}"))?;
                        // Send success event
                        let _ = event_sender.send(DebugEvent::AttachSuccess(None));
                    }
                    AttachMode::Process => {
                        let pid: u32 = target.parse()
                            .map_err(|_| "Invalid PID format".to_string())?;
                        adapter.attach_to_process(pid).await
                            .map_err(|e| format!("Attach to process failed: {e}"))?;
                        // Send success event
                        let _ = event_sender.send(DebugEvent::AttachSuccess(Some(pid)));
                    }
                }
                Ok(())
            }
            GdbCommand::Detach => {
                adapter.detach().await
                    .map_err(|e| format!("Detach failed: {e}"))?;
                // Send success event
                let _ = event_sender.send(DebugEvent::DetachSuccess);
                Ok(())
            }
        }
    }
    
    /// Internal helper to send debug info refresh events
    async fn send_refresh_debug_info_internal(
        mut adapter: tokio::sync::MutexGuard<'_, GdbAdapter>,
        event_sender: std::sync::mpsc::Sender<DebugEvent>,
    ) -> Result<(), String> {
        // Get register names first, then register values
        let mut register_names = Vec::new();
        debug!("send_refresh_debug_info_internal: Getting register names...");
        match adapter.get_register_names().await {
            Ok(names_result) => {
                if let Some(Value::List(names_list)) = names_result.results.get("register-names") {
                    for (i, name_value) in names_list.iter().enumerate() {
                        if let Some(name) = name_value.as_string() {
                            register_names.push((i, name.to_string()));
                        }
                    }
                }
                debug!("send_refresh_debug_info_internal: Parsed {} register names", register_names.len());
            }
            Err(e) => {
                error!("send_refresh_debug_info_internal: Failed to get register names: {e}");
            }
        }
        
        // Get registers
        debug!("send_refresh_debug_info_internal: Getting registers...");
        match adapter.get_registers().await {
            Ok(result) => {
                if let Some(registers) = Self::parse_registers(&result, &register_names) {
                    let _ = event_sender.send(DebugEvent::RegistersUpdated(registers));
                }
            }
            Err(e) => {
                error!("send_refresh_debug_info_internal: Failed to get registers: {e}");
            }
        }
        
        // Get stack frames
        debug!("send_refresh_debug_info_internal: Getting stack frames...");
        match adapter.get_stack_frames().await {
            Ok(result) => {
                match Self::parse_stack_frames(&result) {
                    Ok(stack_frames) => {
                        let _ = event_sender.send(DebugEvent::StackFramesUpdated(stack_frames));
                    }
                    Err(e) => {
                        error!("send_refresh_debug_info_internal: Failed to parse stack frames: {e}");
                    }
                }
            }
            Err(e) => {
                error!("send_refresh_debug_info_internal: Failed to get stack frames: {e}");
            }
        }
        
        // Get assembly around current PC
        debug!("send_refresh_debug_info_internal: Getting assembly...");
        match adapter.disassemble_current(20).await {
            Ok(result) => {
                if let Some(assembly_lines) = Self::parse_assembly(&result) {
                    let _ = event_sender.send(DebugEvent::AssemblyUpdated(assembly_lines));
                }
            }
            Err(e) => {
                error!("send_refresh_debug_info_internal: Failed to get assembly: {e}");
            }
        }
        
        Ok(())
    }


    pub fn clear_debug_info(&mut self) {
        self.registers.clear();
        self.assembly_lines.clear();
        self.stack_frames.clear();
    }
    
    pub fn start_gdb_session(&mut self) {
        info!("start_gdb_session: Starting GDB session operation");
        self.console_output.push_str("Starting GDB session...\n");
        self.is_debugging = true;
        
        info!("start_gdb_session: Sending StartSession command via channel");
        
        // Send command via channel - non-blocking
        if let Err(e) = self.command_sender.send(GdbCommand::StartSession) {
            error!("start_gdb_session: Failed to send StartSession command: {e}");
            self.console_output.push_str(&format!("Failed to send start session command: {e}\n"));
        } else {
            info!("start_gdb_session: StartSession command sent successfully");
            // The result will come back via the event system
        }
    }

    pub fn stop_gdb_session(&mut self) {
        info!("stop_gdb_session: Starting stop session operation");
        self.console_output.push_str("Stopping GDB session...\n");
        
        info!("stop_gdb_session: Sending StopSession command via channel");
        
        // Send command via channel - non-blocking
        if let Err(e) = self.command_sender.send(GdbCommand::StopSession) {
            error!("stop_gdb_session: Failed to send StopSession command: {e}");
            self.console_output.push_str(&format!("Failed to send stop session command: {e}\n"));
        } else {
            info!("stop_gdb_session: StopSession command sent successfully");
            // The result will come back via the event system
        }
        
        // Update state immediately for UI responsiveness
        self.is_debugging = false;
        self.is_attached = false;
        self.clear_debug_info();
        self.breakpoints.clear();
    }

    fn attach_to_target(&mut self) {
        info!("attach_to_target: Starting attachment process");
        self.console_output.push_str("Starting attachment process...\n");
        
        // Show immediate feedback and validate input
        let target = match self.attach_mode {
            AttachMode::Process => {
                if let Ok(pid) = self.pid_input.parse::<u32>() {
                    self.console_output.push_str(&format!("Attaching to process {pid}...\n"));
                    self.pid_input.clone()
                } else {
                    self.console_output.push_str("Invalid PID format\n");
                    self.error_message = "Invalid PID format".to_string();
                    return;
                }
            }
            AttachMode::GdbServer => {
                self.console_output.push_str(&format!("Attaching to GDB server at {}...\n", self.current_host_port));
                self.current_host_port.clone()
            }
        };

        info!("attach_to_target: Sending Attach command via channel");
        
        // Send command via channel - non-blocking
        if let Err(e) = self.command_sender.send(GdbCommand::Attach(self.attach_mode.clone(), target)) {
            error!("attach_to_target: Failed to send Attach command: {e}");
            self.console_output.push_str(&format!("Failed to send attach command: {e}\n"));
        } else {
            info!("attach_to_target: Attach command sent successfully");
            // The result will come back via the event system
        }
    }
    
    fn detach_from_target(&mut self) {
        info!("detach_from_target: Starting detach operation");
        self.console_output.push_str("Detaching from target...\n");
        
        info!("detach_from_target: Sending Detach command via channel");
        
        // Send command via channel - non-blocking
        if let Err(e) = self.command_sender.send(GdbCommand::Detach) {
            error!("detach_from_target: Failed to send Detach command: {e}");
            self.console_output.push_str(&format!("Failed to send detach command: {e}\n"));
        } else {
            info!("detach_from_target: Detach command sent successfully");
            // The result will come back via the event system
        }
        
        // Clear state immediately for UI responsiveness
        self.is_attached = false;
        self.is_debugging = false;
        self.current_pid = None;
        self.registers.clear();
        self.assembly_lines.clear();
        self.stack_frames.clear();
    }
    
    fn set_breakpoint(&mut self) {
        if !self.breakpoint_input.is_empty() {
            info!("set_breakpoint: Starting set breakpoint operation");
            self.console_output.push_str(&format!("Setting breakpoint at: {}\n", self.breakpoint_input));
            
            let location = self.breakpoint_input.clone();
            
            // Add to breakpoints list immediately for UI feedback
            self.breakpoints.push(location.clone());
            self.breakpoint_input.clear();
            
            info!("set_breakpoint: Sending SetBreakpoint command via channel");
            
            // Send command via channel - non-blocking
            if let Err(e) = self.command_sender.send(GdbCommand::SetBreakpoint(location)) {
                error!("set_breakpoint: Failed to send SetBreakpoint command: {e}");
                self.console_output.push_str(&format!("Failed to send set breakpoint command: {e}\n"));
            } else {
                info!("set_breakpoint: SetBreakpoint command sent successfully");
                // The result will come back via the event system
            }
        }
    }
    
    fn continue_execution(&mut self) {
        info!("continue_execution: Starting continue operation (async)");
        self.console_output.push_str("Continuing execution...\n");
        
        if !self.is_debugging || !self.is_attached {
            warn!("continue_execution: Not attached to a debug target (debugging: {}, attached: {})", 
                  self.is_debugging, self.is_attached);
            self.console_output.push_str("Not attached to a debug target\n");
            return;
        }

        info!("continue_execution: Sending Continue command via channel");
        
        // Send command via channel - non-blocking
        if let Err(e) = self.command_sender.send(GdbCommand::Continue) {
            error!("continue_execution: Failed to send Continue command: {e}");
            self.console_output.push_str(&format!("Failed to send continue command: {e}\n"));
        } else {
            info!("continue_execution: Continue command sent successfully");
            // The result will come back via the event system
            // Update UI state immediately to show that we're running
            self.target_state = TargetState::Running;
        }
    }
    
    fn step_over(&mut self) {
        info!("step_over: Starting step over operation (async)");
        self.console_output.push_str("Step over\n");
        
        if !self.is_debugging || !self.is_attached {
            warn!("step_over: Not attached to a debug target (debugging: {}, attached: {})", 
                  self.is_debugging, self.is_attached);
            self.console_output.push_str("Not attached to a debug target\n");
            return;
        }

        info!("step_over: Sending StepOver command via channel");
        
        // Send command via channel - non-blocking
        if let Err(e) = self.command_sender.send(GdbCommand::StepOver) {
            error!("step_over: Failed to send StepOver command: {e}");
            self.console_output.push_str(&format!("Failed to send step over command: {e}\n"));
        } else {
            info!("step_over: StepOver command sent successfully");
            // The result will come back via the event system
        }
    }
    
    fn step_into(&mut self) {
        info!("step_into: Starting step into operation (async)");
        self.console_output.push_str("Step into\n");
        
        if !self.is_debugging || !self.is_attached {
            warn!("step_into: Not attached to a debug target (debugging: {}, attached: {})", 
                  self.is_debugging, self.is_attached);
            self.console_output.push_str("Not attached to a debug target\n");
            return;
        }

        info!("step_into: Sending StepInto command via channel");
        
        // Send command via channel - non-blocking
        if let Err(e) = self.command_sender.send(GdbCommand::StepInto) {
            error!("step_into: Failed to send StepInto command: {e}");
            self.console_output.push_str(&format!("Failed to send step into command: {e}\n"));
        } else {
            info!("step_into: StepInto command sent successfully");
            // The result will come back via the event system
        }
    }
    
    fn step_out(&mut self) {
        info!("step_out: Starting step out operation (async)");
        self.console_output.push_str("Step out\n");
        
        if !self.is_debugging || !self.is_attached {
            warn!("step_out: Not attached to a debug target (debugging: {}, attached: {})", 
                  self.is_debugging, self.is_attached);
            self.console_output.push_str("Not attached to a debug target\n");
            return;
        }

        info!("step_out: Sending StepOut command via channel");
        
        // Send command via channel - non-blocking
        if let Err(e) = self.command_sender.send(GdbCommand::StepOut) {
            error!("step_out: Failed to send StepOut command: {e}");
            self.console_output.push_str(&format!("Failed to send step out command: {e}\n"));
        } else {
            info!("step_out: StepOut command sent successfully");
            // The result will come back via the event system
        }
    }
    
    fn interrupt_execution(&mut self) {
        info!("interrupt_execution: Starting interrupt operation (async)");
        self.console_output.push_str("Interrupting execution...\n");
        
        if !self.is_debugging || !self.is_attached {
            warn!("interrupt_execution: Not attached to a debug target (debugging: {}, attached: {})", 
                  self.is_debugging, self.is_attached);
            self.console_output.push_str("Not attached to a debug target\n");
            return;
        }
        
        info!("interrupt_execution: Sending Interrupt command via channel");
        
        // Send command via channel - non-blocking
        if let Err(e) = self.command_sender.send(GdbCommand::Interrupt) {
            error!("interrupt_execution: Failed to send Interrupt command: {e}");
            self.console_output.push_str(&format!("Failed to send interrupt command: {e}\n"));
        } else {
            info!("interrupt_execution: Interrupt command sent successfully");
            // The result will come back via the event system
        }
    }
    
    fn refresh_debug_info(&mut self) {
        info!("refresh_debug_info: Starting debug info refresh (async)");
        self.console_output.push_str("Refreshing debug information...\n");
        
        if !self.is_debugging || !self.is_attached {
            warn!("refresh_debug_info: Not debugging or attached (debugging: {}, attached: {})", 
                  self.is_debugging, self.is_attached);
            self.console_output.push_str("Not attached to a debug target\n");
            return;
        }

        info!("refresh_debug_info: Sending RefreshDebugInfo command via channel");
        
        // Send command via channel - non-blocking
        if let Err(e) = self.command_sender.send(GdbCommand::RefreshDebugInfo) {
            error!("refresh_debug_info: Failed to send RefreshDebugInfo command: {e}");
            self.console_output.push_str(&format!("Failed to send refresh command: {e}\n"));
        } else {
            info!("refresh_debug_info: RefreshDebugInfo command sent successfully");
            // The result will come back via the event system
        }
    }
    
    fn read_memory(&mut self) {
        info!("read_memory: Starting read memory operation");
        self.console_output.push_str(&format!("Reading {} bytes from {}\n", self.memory_size, self.memory_address));
        
        let address = self.memory_address.clone();
        let size = self.memory_size;
        
        info!("read_memory: Sending ReadMemory command via channel");
        
        // Send command via channel - non-blocking
        if let Err(e) = self.command_sender.send(GdbCommand::ReadMemory(address, size)) {
            error!("read_memory: Failed to send ReadMemory command: {e}");
            self.console_output.push_str(&format!("Failed to send read memory command: {e}\n"));
        } else {
            info!("read_memory: ReadMemory command sent successfully");
            // The result will come back via the event system
        }
    }
    
    /// Automatically fetch debug information when GDB is stopped
    fn auto_refresh_debug_info(&mut self) {
        if !self.is_debugging || !self.is_attached {
            return;
        }
        
        info!("auto_refresh_debug_info: Starting auto refresh");
        self.console_output.push_str("Refreshing debug information...\n");
        
        info!("auto_refresh_debug_info: Sending RefreshDebugInfo command via channel");
        
        // Send command via channel - non-blocking
        if let Err(e) = self.command_sender.send(GdbCommand::RefreshDebugInfo) {
            error!("auto_refresh_debug_info: Failed to send RefreshDebugInfo command: {e}");
            self.console_output.push_str(&format!("Failed to send refresh command: {e}\n"));
        } else {
            info!("auto_refresh_debug_info: RefreshDebugInfo command sent successfully");
            // The result will come back via the event system
        }
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
                        .unwrap_or_else(|| format!("r{number}"));
                    
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
            debug!("parse_registers: Available register result keys: {:?}", result.results.keys().collect::<Vec<_>>());
            None
        }
    }
    
    /// Parse stack frames from GDB/MI result
    fn parse_stack_frames(result: &gdbadapter::GdbResult) -> Result<Vec<StackFrame>, String> {
        // GDB/MI uses "stack" field for -stack-list-frames
        if let Some(Value::List(frame_list)) = result.results.get("stack") {
            let mut frames = Vec::new();
            
            for (index, frame_value) in frame_list.iter().enumerate() {
                if let Some(frame_tuple) = frame_value.as_tuple() {
                    // Check for nested frame structure (frame={...})
                    let actual_frame = if let Some(nested_frame) = frame_tuple.get("frame") {
                        if let Some(nested_tuple) = nested_frame.as_tuple() {
                            nested_tuple
                        } else {
                            return Err(format!("Frame {index} has invalid nested frame structure"));
                        }
                    } else {
                        frame_tuple
                    };
                    
                    let level = actual_frame.get("level")
                        .and_then(|v| v.as_string())
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("Frame {index} missing or invalid 'level' field"))?;
                    
                    let address = actual_frame.get("addr")
                        .and_then(|v| v.as_string())
                        .ok_or_else(|| format!("Frame {index} missing 'addr' field"))?
                        .to_string();
                    
                    let function = actual_frame.get("func").and_then(|v| v.as_string()).map(|s| s.to_string());
                    let file = actual_frame.get("file").and_then(|v| v.as_string()).map(|s| s.to_string());
                    let fullname = actual_frame.get("fullname").and_then(|v| v.as_string()).map(|s| s.to_string());
                    let line = actual_frame.get("line").and_then(|v| v.as_string()).and_then(|s| s.parse().ok());
                    let arch = actual_frame.get("arch").and_then(|v| v.as_string()).map(|s| s.to_string());
                    
                    frames.push(StackFrame {
                        level,
                        address,
                        function,
                        file,
                        fullname,
                        line,
                        arch,
                    });
                } else {
                    return Err(format!("Frame {index} is not a tuple structure"));
                }
            }
            
            Ok(frames)
        } else {
            // Check if there's a different structure
            debug!("parse_stack_frames: Available stack result keys: {:?}", result.results.keys().collect::<Vec<_>>());
            Err(format!("No 'stack' field found in result. Available keys: {:?}", 
                result.results.keys().collect::<Vec<_>>()))
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
            debug!("parse_assembly: Available assembly result keys: {:?}", result.results.keys().collect::<Vec<_>>());
            None
        }
    }
}

impl eframe::App for KatoriApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process events from async operations
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                DebugEvent::RegistersUpdated(registers) => {
                    self.registers = registers;
                    info!("Event: Updated registers: {} items", self.registers.len());
                }
                DebugEvent::StackFramesUpdated(stack_frames) => {
                    self.stack_frames = stack_frames;
                    info!("Event: Updated stack frames: {} items", self.stack_frames.len());
                }
                DebugEvent::AssemblyUpdated(assembly_lines) => {
                    self.assembly_lines = assembly_lines;
                    info!("Event: Updated assembly: {} items", self.assembly_lines.len());
                }
                DebugEvent::ConsoleMessage(message) => {
                    self.console_output.push_str(&message);
                }
                DebugEvent::AttachSuccess(pid) => {
                    self.is_attached = true;
                    self.is_debugging = true;
                    if let Some(pid) = pid {
                        self.current_pid = Some(pid);
                        self.console_output.push_str(&format!("Successfully attached to process {pid}\n"));
                    } else {
                        self.console_output.push_str("Successfully attached to GDB server\n");
                    }
                    // Auto-refresh debug info after successful attach
                    self.auto_refresh_debug_info();
                }
                DebugEvent::AttachFailed(error) => {
                    self.console_output.push_str(&format!("Attach failed: {error}\n"));
                    self.error_message = format!("Attach failed: {error}");
                }
                DebugEvent::DetachSuccess => {
                    self.console_output.push_str("Successfully detached\n");
                }
                DebugEvent::MemoryRead(data) => {
                    self.memory_data = data.clone();
                    self.console_output.push_str(&format!("Memory read successfully: {} bytes\n", data.len()));
                    info!("Event: Memory read completed: {} bytes", data.len());
                }
                DebugEvent::MemoryReadFailed(error) => {
                    self.console_output.push_str(&format!("Memory read failed: {error}\n"));
                    self.error_message = format!("Memory read failed: {error}");
                    info!("Event: Memory read failed: {error}");
                }
                DebugEvent::CommandCompleted(command) => {
                    info!("Event: Command completed: {command:?}");
                    // Update target state if needed
                    match command {
                        GdbCommand::Continue => {
                            self.target_state = TargetState::Running;
                            self.console_output.push_str("Target is now running\n");
                        }
                        GdbCommand::StepOver | GdbCommand::StepInto | GdbCommand::StepOut => {
                            self.target_state = TargetState::Stopped;
                            self.console_output.push_str("Step completed\n");
                            self.auto_refresh_debug_info();
                        }
                        GdbCommand::Interrupt => {
                            self.target_state = TargetState::Stopped;
                            self.console_output.push_str("Target interrupted\n");
                            self.auto_refresh_debug_info();
                        }
                        _ => {}
                    }
                }
                DebugEvent::CommandFailed(command, error) => {
                    error!("Event: Command failed: {command:?} - {error}");
                    self.console_output.push_str(&format!("Command failed: {command:?} - {error}\n"));
                }
                DebugEvent::GdbConnectionLost => {
                    error!("Event: GDB connection lost");
                    self.console_output.push_str("GDB connection lost!\n");
                    self.is_debugging = false;
                    self.is_attached = false;
                    self.target_state = TargetState::Detached;
                }
                DebugEvent::TargetStateChanged(new_state) => {
                    info!("Event: Target state changed to: {new_state:?}");
                    self.target_state = new_state.clone();
                    match new_state {
                        TargetState::Running => {
                            self.console_output.push_str("Target is running\n");
                        }
                        TargetState::Stopped => {
                            self.console_output.push_str("Target stopped\n");
                            // Auto-refresh debug info when stopped
                            if let Err(e) = self.command_sender.send(GdbCommand::RefreshDebugInfo) {
                                error!("Failed to send RefreshDebugInfo command: {e}");
                            }
                        }
                        TargetState::Detached => {
                            self.console_output.push_str("Target detached\n");
                            self.clear_debug_info();
                        }
                    }
                }
            }
        }
        
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
                        ui.close_menu();
                    }
                    if ui.button("Stop Session").clicked() {
                        self.stop_gdb_session();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Attach").clicked() {
                        self.attach_to_target();
                        ui.close_menu();
                    }
                    if ui.button("Detach").clicked() {
                        self.detach_from_target();
                        ui.close_menu();
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

        // Right sidebar for registers and stack
        if self.show_registers || self.show_stack {
            egui::SidePanel::right("debug_sidebar")
                .min_width(250.0)
                .default_width(300.0)
                .resizable(true)
                .show(ctx, |ui| {
                    // Registers panel (top half of sidebar)
                    if self.show_registers {
                        ui.heading("Registers");
                        
                        let available_height = if self.show_stack {
                            ui.available_height() * 0.5
                        } else {
                            ui.available_height()
                        };
                        
                        ui.allocate_ui_with_layout(
                            egui::Vec2::new(ui.available_width(), available_height),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
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
                        
                        if self.show_stack {
                            ui.separator();
                        }
                    }
                    
                    // Stack frames panel (bottom half of sidebar)
                    if self.show_stack {
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
                });
        }

        // Memory viewer at bottom (if enabled)
        if self.show_memory {
            egui::TopBottomPanel::bottom("memory_panel")
                .min_height(200.0)
                .default_height(250.0)
                .resizable(true)
                .show(ctx, |ui| {
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
                    
                    ui.separator();
                    
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
                                        hex_str.push_str(&format!("{byte:02x} "));
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
                });
        }
        
        // Main content area - Assembly takes the remaining space
        egui::CentralPanel::default().show(ctx, |ui| {
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
                            let text = self.assembly_lines.iter()
                                .map(|line| format!("{}: {}",line.address, line.instruction))
                                .collect::<Vec<_>>()
                                .join("\n");

                            self.show_code(ui, text);
                        }
                    });
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("Assembly view disabled");
                });
            }
        });
    }
}

impl KatoriApp {
    fn show_code(&mut self, ui: &mut egui::Ui, text: String) {
        let ps = self.syntax_set.clone();
        let ts = syntect::highlighting::ThemeSet::load_defaults();
        let syntax =
            egui_extras::syntax_highlighting::SyntectSettings { ps, ts };

        // ...elsewhere
        let theme =egui_extras::syntax_highlighting::CodeTheme::default();
        let layout = egui_extras::syntax_highlighting::highlight_with(
            ui.ctx(),
            ui.style(),
            &theme,
            &text,
            "ARM",
            &syntax,
        );

        // let layout = egui_extras::syntax_highlighting::highlight(ui.ctx(), ui.style(), &egui_extras::syntax_highlighting::CodeTheme::default(), &text, "arm");
        ui.add(egui::Label::new(layout));
        // egui_extras::syntax_highlighting::code_view_ui(ui, &egui_extras::syntax_highlighting::CodeTheme::default(), &text, "arm");

        // let language = "C"; 
        // let theme =egui_extras::syntax_highlighting::CodeTheme::default();
        // egui_extras::syntax_highlighting::code_view_ui(ui, &theme, &text, language);

    }

}
