/// Main application logic and coordination
/// 
/// This module contains the main KatoriApp struct and coordinates
/// between the GDB adapter, UI components, and application state.

use eframe::egui;
use gdbadapter::GdbAdapter;
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, error, debug};

use crate::commands::{GdbCommand, DebugEvent, AttachMode, TargetState};
use crate::state::AppState;
use crate::ui;

/// Main application struct that coordinates all components
pub struct KatoriApp {
    /// GDB adapter instance
    gdb_adapter: Arc<Mutex<GdbAdapter>>,
    
    /// Event communication
    event_receiver: std::sync::mpsc::Receiver<DebugEvent>,
    event_sender: std::sync::mpsc::Sender<DebugEvent>,
    
    /// Command channel for async GDB operations
    command_sender: std::sync::mpsc::Sender<GdbCommand>,
    command_receiver: std::sync::mpsc::Receiver<GdbCommand>,
    
    /// Application state
    state: AppState,
}

impl KatoriApp {
    /// Create a new KatoriApp instance
    pub fn new() -> Self {
        let gdb_adapter = Arc::new(Mutex::new(GdbAdapter::new()));
        
        // Create event communication channels
        let (event_sender, event_receiver) = std::sync::mpsc::channel();
        let (command_sender, command_receiver) = std::sync::mpsc::channel();
        
        // Start background task for processing GDB commands
        let adapter_clone = gdb_adapter.clone();
        let event_sender_clone = event_sender.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                Self::command_processor(adapter_clone, command_receiver, event_sender_clone).await;
            });
        });
        
        Self {
            gdb_adapter,
            event_receiver,
            event_sender,
            command_sender,
            command_receiver: std::sync::mpsc::channel().1, // Dummy receiver since we moved it to background
            state: AppState::new(),
        }
    }
    
    /// Background task that processes GDB commands
    async fn command_processor(
        gdb_adapter: Arc<Mutex<GdbAdapter>>,
        command_receiver: std::sync::mpsc::Receiver<GdbCommand>,
        event_sender: std::sync::mpsc::Sender<DebugEvent>,
    ) {
        while let Ok(command) = command_receiver.recv() {
            let mut adapter = gdb_adapter.lock().await;
            
            match command {
                GdbCommand::StartSession => {
                    match adapter.start_session().await {
                        Ok(_) => {
                            info!("GDB session started successfully");
                            let _ = event_sender.send(DebugEvent::CommandCompleted(command));
                            let _ = event_sender.send(DebugEvent::ConsoleMessage("GDB session started successfully".to_string()));
                        }
                        Err(e) => {
                            error!("Failed to start GDB session: {}", e);
                            let _ = event_sender.send(DebugEvent::CommandFailed(command, e.to_string()));
                            let _ = event_sender.send(DebugEvent::ConsoleMessage(format!("Failed to start GDB: {}", e)));
                        }
                    }
                }
                GdbCommand::StopSession => {
                    match adapter.stop_session().await {
                        Ok(_) => {
                            info!("GDB session stopped successfully");
                            let _ = event_sender.send(DebugEvent::CommandCompleted(command));
                            let _ = event_sender.send(DebugEvent::ConsoleMessage("GDB session stopped".to_string()));
                        }
                        Err(e) => {
                            error!("Failed to stop GDB session: {}", e);
                            let _ = event_sender.send(DebugEvent::CommandFailed(command, e.to_string()));
                            let _ = event_sender.send(DebugEvent::ConsoleMessage(format!("Failed to stop session: {}", e)));
                        }
                    }
                }
                GdbCommand::Attach(mode, target) => {
                    let result = match mode {
                        AttachMode::Process => {
                            if let Ok(pid) = target.parse::<u32>() {
                                adapter.attach_to_process(pid).await.map(|_| Some(pid))
                            } else {
                                Err(gdbadapter::GdbError::CommandError("Invalid PID".to_string()))
                            }
                        }
                        AttachMode::GdbServer => {
                            adapter.attach_to_gdbserver(&target).await.map(|_| None)
                        }
                    };
                    
                    match result {
                        Ok(pid) => {
                            info!("Successfully attached to target");
                            let _ = event_sender.send(DebugEvent::AttachSuccess(pid));
                        }
                        Err(e) => {
                            error!("Failed to attach: {}", e);
                            let _ = event_sender.send(DebugEvent::AttachFailed(e.to_string()));
                        }
                    }
                }
                GdbCommand::Detach => {
                    match adapter.detach().await {
                        Ok(_) => {
                            info!("Successfully detached from target");
                            let _ = event_sender.send(DebugEvent::DetachSuccess);
                        }
                        Err(e) => {
                            error!("Failed to detach: {}", e);
                            let _ = event_sender.send(DebugEvent::CommandFailed(command, e.to_string()));
                        }
                    }
                }
                GdbCommand::Continue => {
                    match adapter.continue_execution().await {
                        Ok(_) => {
                            let _ = event_sender.send(DebugEvent::TargetStateChanged(TargetState::Running));
                            let _ = event_sender.send(DebugEvent::CommandCompleted(command));
                        }
                        Err(e) => {
                            let _ = event_sender.send(DebugEvent::CommandFailed(command, e.to_string()));
                        }
                    }
                }
                GdbCommand::StepOver => {
                    match adapter.next().await {
                        Ok(_) => {
                            let _ = event_sender.send(DebugEvent::TargetStateChanged(TargetState::Stopped));
                            let _ = event_sender.send(DebugEvent::CommandCompleted(command));
                        }
                        Err(e) => {
                            let _ = event_sender.send(DebugEvent::CommandFailed(command, e.to_string()));
                        }
                    }
                }
                GdbCommand::StepInto => {
                    match adapter.step().await {
                        Ok(_) => {
                            let _ = event_sender.send(DebugEvent::TargetStateChanged(TargetState::Stopped));
                            let _ = event_sender.send(DebugEvent::CommandCompleted(command));
                        }
                        Err(e) => {
                            let _ = event_sender.send(DebugEvent::CommandFailed(command, e.to_string()));
                        }
                    }
                }
                GdbCommand::StepOut => {
                    match adapter.step_out().await {
                        Ok(_) => {
                            let _ = event_sender.send(DebugEvent::TargetStateChanged(TargetState::Stopped));
                            let _ = event_sender.send(DebugEvent::CommandCompleted(command));
                        }
                        Err(e) => {
                            let _ = event_sender.send(DebugEvent::CommandFailed(command, e.to_string()));
                        }
                    }
                }
                GdbCommand::Interrupt => {
                    match adapter.interrupt().await {
                        Ok(_) => {
                            let _ = event_sender.send(DebugEvent::TargetStateChanged(TargetState::Stopped));
                            let _ = event_sender.send(DebugEvent::CommandCompleted(command));
                        }
                        Err(e) => {
                            let _ = event_sender.send(DebugEvent::CommandFailed(command, e.to_string()));
                        }
                    }
                }
                GdbCommand::SetBreakpoint(ref location) => {
                    match adapter.set_breakpoint(location).await {
                        Ok(_) => {
                            let _ = event_sender.send(DebugEvent::CommandCompleted(command));
                        }
                        Err(e) => {
                            let _ = event_sender.send(DebugEvent::CommandFailed(command, e.to_string()));
                        }
                    }
                }
                GdbCommand::ReadMemory(ref address, size) => {
                    match adapter.read_memory(address, size).await {
                        Ok(_result) => {
                            // TODO: Parse memory result and send MemoryUpdated event
                            let _ = event_sender.send(DebugEvent::CommandCompleted(command));
                        }
                        Err(e) => {
                            let _ = event_sender.send(DebugEvent::CommandFailed(command, e.to_string()));
                        }
                    }
                }
                GdbCommand::RefreshDebugInfo => {
                    // Get registers
                    match adapter.get_registers().await {
                        Ok(_result) => {
                            // For now, create some dummy register data
                            let registers = vec![
                                gdbadapter::Register {
                                    number: 0,
                                    name: "pc".to_string(),
                                    value: "0x00000000".to_string(),
                                },
                                gdbadapter::Register {
                                    number: 1,
                                    name: "sp".to_string(),
                                    value: "0x20000000".to_string(),
                                },
                                gdbadapter::Register {
                                    number: 2,
                                    name: "lr".to_string(),
                                    value: "0x00000000".to_string(),
                                },
                            ];
                            let _ = event_sender.send(DebugEvent::RegistersUpdated(registers));
                        }
                        Err(e) => {
                            error!("Failed to get registers: {}", e);
                        }
                    }
                    
                    // Get stack frames
                    match adapter.get_stack_frames(None, None).await {
                        Ok(stack_frames) => {
                            // Convert from commands::StackFrame to types::StackFrame 
                            let _converted_frames: Vec<gdbadapter::Register> = vec![]; // Skip for now
                            let _ = event_sender.send(DebugEvent::StackFramesUpdated(stack_frames));
                        }
                        Err(e) => {
                            error!("Failed to get stack frames: {}", e);
                            // Create dummy stack frame using the commands::StackFrame structure
                            let stack_frames = vec![
                                gdbadapter::StackFrame {
                                    level: 0,
                                    addr: "0x00000000".to_string(),
                                    func: Some("??".to_string()),
                                    file: None,
                                    fullname: None,
                                    line: None,
                                }
                            ];
                            let _ = event_sender.send(DebugEvent::StackFramesUpdated(stack_frames));
                        }
                    }
                    
                    // Get assembly around current PC
                    match adapter.disassemble_current(20).await {
                        Ok(_result) => {
                            // For now, create some dummy assembly data
                            let assembly_lines = vec![
                                gdbadapter::AssemblyLine {
                                    address: "0x00000000".to_string(),
                                    function: Some("??".to_string()),
                                    offset: Some(0),
                                    instruction: "<unavailable>".to_string(),
                                    opcodes: None,
                                },
                            ];
                            let _ = event_sender.send(DebugEvent::AssemblyUpdated(assembly_lines));
                        }
                        Err(e) => {
                            error!("Failed to get assembly: {}", e);
                        }
                    }
                    
                    let _ = event_sender.send(DebugEvent::CommandCompleted(command));
                    let _ = event_sender.send(DebugEvent::ConsoleMessage("Debug info refreshed".to_string()));
                }
            }
        }
    }

    
    /// Process events from the GDB adapter
    fn process_events(&mut self, command_sender: &std::sync::mpsc::Sender<GdbCommand>) {
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                DebugEvent::RegistersUpdated(registers) => {
                    self.state.registers = registers;
                }
                DebugEvent::StackFramesUpdated(stack_frames) => {
                    self.state.stack_frames = stack_frames;
                }
                DebugEvent::AssemblyUpdated(assembly_lines) => {
                    self.state.assembly_lines = assembly_lines;
                }
                DebugEvent::ConsoleMessage(message) => {
                    self.state.add_console_message(message);
                }
                DebugEvent::AttachSuccess(pid) => {
                    self.state.is_attached = true;
                    self.state.current_pid = pid;
                    self.state.target_state = TargetState::Stopped;
                    info!("Successfully attached to target");
                    
                    // Auto-refresh debug info after successful attach
                    if let Err(e) = command_sender.send(GdbCommand::RefreshDebugInfo) {
                        error!("Failed to send auto-refresh command: {}", e);
                    }
                }
                DebugEvent::AttachFailed(error) => {
                    self.state.set_error(format!("Failed to attach: {}", error));
                }
                DebugEvent::DetachSuccess => {
                    self.state.is_attached = false;
                    self.state.current_pid = None;
                    self.state.target_state = TargetState::Detached;
                    info!("Successfully detached from target");
                }
                DebugEvent::CommandCompleted(command) => {
                    debug!("Command completed: {:?}", command);
                    match command {
                        GdbCommand::StartSession => {
                            self.state.is_debugging = true;
                            info!("Debug session started - GUI state updated");
                        }
                        GdbCommand::StopSession => {
                            self.state.reset_debug_state();
                            info!("Debug session stopped - GUI state reset");
                        }
                        GdbCommand::StepOver | GdbCommand::StepInto | GdbCommand::StepOut | GdbCommand::Interrupt => {
                            // Auto-refresh debug info after step commands
                            if let Err(e) = command_sender.send(GdbCommand::RefreshDebugInfo) {
                                error!("Failed to send auto-refresh command: {}", e);
                            }
                        }
                        _ => {}
                    }
                }
                DebugEvent::CommandFailed(command, error) => {
                    match command {
                        GdbCommand::StartSession => {
                            self.state.set_error(format!("Failed to start GDB session: {}", error));
                            self.state.is_debugging = false;
                        }
                        GdbCommand::StopSession => {
                            self.state.set_error(format!("Failed to stop GDB session: {}", error));
                            // Don't reset state on stop failure - we might still be debugging
                        }
                        _ => {
                            self.state.set_error(format!("Command {:?} failed: {}", command, error));
                        }
                    }
                }
                DebugEvent::GdbConnectionLost => {
                    self.state.reset_debug_state();
                    self.state.set_error("Lost connection to GDB".to_string());
                }
                DebugEvent::TargetStateChanged(target_state) => {
                    self.state.target_state = target_state;
                }
            }
        }
    }
}

impl eframe::App for KatoriApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process events from GDB adapter
        let command_sender = self.command_sender.clone();
        self.process_events(&command_sender);
        
        // Render error popup if needed
        ui::render_error_popup(ctx, &mut self.state);
        
        // Menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui::render_view_menu(ui, &mut self.state, &self.command_sender);
            });
        });
        
        // Main toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui::render_toolbar(ui, &mut self.state, &self.command_sender);
        });
        
        // Attach panel
        egui::TopBottomPanel::top("attach_panel").show(ctx, |ui| {
            ui::render_attach_panel(ui, &mut self.state, &self.command_sender);
        });
        
        // Breakpoint panel
        egui::TopBottomPanel::top("breakpoint_panel").show(ctx, |ui| {
            ui::render_breakpoint_panel(ui, &mut self.state, &self.command_sender);
        });
        
        // Error message panel
        if self.state.has_error() {
            egui::TopBottomPanel::top("error_panel").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::RED, &self.state.error_message);
                    if ui.button("✕").clicked() {
                        self.state.clear_error();
                    }
                });
            });
        }
        
        // Console panel (always at bottom when visible)
        if self.state.show_console {
            egui::TopBottomPanel::bottom("console").min_height(150.0).show(ctx, |ui| {
                ui::render_console_panel(ui, &self.state);
            });
        }
        
        // Main content area with side panels
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Main assembly panel (takes most of the space)
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width() * 0.7, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        if self.state.show_assembly {
                            ui::render_assembly_panel(ui, &self.state);
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
                        if self.state.show_registers {
                            ui.allocate_ui_with_layout(
                                egui::Vec2::new(ui.available_width(), ui.available_height() * 0.5),
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    ui::render_registers_panel(ui, &self.state);
                                }
                            );
                        }
                        
                        ui.separator();
                        
                        // Stack frames panel (bottom half of sidebar)
                        if self.state.show_stack {
                            ui.allocate_ui_with_layout(
                                egui::Vec2::new(ui.available_width(), ui.available_height()),
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    ui::render_stack_panel(ui, &self.state);
                                }
                            );
                        }
                    }
                );
            });
            
            // Memory viewer (if enabled) - separate section at the bottom
            if self.state.show_memory {
                ui.separator();
                ui::render_memory_panel(ui, &mut self.state, &self.command_sender);
            }
        });
        
        // Request repaint to keep UI responsive
        ctx.request_repaint();
    }
}
