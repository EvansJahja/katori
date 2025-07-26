/// UI components for the debugging interface
/// 
/// This module contains reusable UI components for different debugging views.

use eframe::egui;
use crate::state::AppState;
use crate::commands::{GdbCommand, AttachMode};

/// Render the main toolbar with debugging controls
pub fn render_toolbar(
    ui: &mut egui::Ui,
    state: &mut AppState,
    command_sender: &std::sync::mpsc::Sender<GdbCommand>,
) {
    ui.horizontal(|ui| {
        // Session management buttons
        if !state.is_debugging {
            if ui.button("🚀 Start Session").clicked() {
                let _ = command_sender.send(GdbCommand::StartSession);
            }
        } else {
            if ui.button("🛑 Stop Session").clicked() {
                let _ = command_sender.send(GdbCommand::StopSession);
            }
        }
        
        ui.separator();
        
        // Debug control buttons
        let enabled = state.is_attached;
        
        ui.add_enabled_ui(enabled, |ui| {
            if ui.button("▶ Continue").clicked() {
                let _ = command_sender.send(GdbCommand::Continue);
            }
            if ui.button("⏸ Break").clicked() {
                let _ = command_sender.send(GdbCommand::Interrupt);
            }
        });
        
        ui.separator();
        
        ui.add_enabled_ui(enabled, |ui| {
            if ui.button("⬇ Step Into").clicked() {
                let _ = command_sender.send(GdbCommand::StepInto);
            }
            if ui.button("➡ Step Over").clicked() {
                let _ = command_sender.send(GdbCommand::StepOver);
            }
            if ui.button("⬆ Step Out").clicked() {
                let _ = command_sender.send(GdbCommand::StepOut);
            }
        });
        
        ui.separator();
        
        ui.add_enabled_ui(enabled, |ui| {
            if ui.button("🔄 Refresh").clicked() {
                let _ = command_sender.send(GdbCommand::RefreshDebugInfo);
            }
        });
        
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Status indicator
            ui.label(if state.is_debugging {
                if state.is_attached {
                    "🔗 Attached"
                } else {
                    "🔴 Debugging"
                }
            } else {
                "⭕ Ready"
            });
        });
    });
}

/// Render the attach panel
pub fn render_attach_panel(
    ui: &mut egui::Ui,
    state: &mut AppState,
    command_sender: &std::sync::mpsc::Sender<GdbCommand>,
) {
    ui.horizontal(|ui| {
        ui.label("Attach to:");
        ui.selectable_value(&mut state.attach_mode, AttachMode::GdbServer, "GDB Server");
        ui.selectable_value(&mut state.attach_mode, AttachMode::Process, "Process");
        
        match state.attach_mode {
            AttachMode::GdbServer => {
                ui.label("Host:Port:");
                ui.text_edit_singleline(&mut state.current_host_port);
            }
            AttachMode::Process => {
                ui.label("PID:");
                ui.text_edit_singleline(&mut state.pid_input);
            }
        }
        
        if ui.button("Attach").clicked() {
            let target = match state.attach_mode {
                AttachMode::Process => state.pid_input.clone(),
                AttachMode::GdbServer => state.current_host_port.clone(),
            };
            let _ = command_sender.send(GdbCommand::Attach(state.attach_mode.clone(), target));
        }
        
        if state.is_attached && ui.button("Detach").clicked() {
            let _ = command_sender.send(GdbCommand::Detach);
        }
    });
}

/// Render the breakpoint panel
pub fn render_breakpoint_panel(
    ui: &mut egui::Ui,
    state: &mut AppState,
    command_sender: &std::sync::mpsc::Sender<GdbCommand>,
) {
    ui.horizontal(|ui| {
        ui.label("Breakpoint:");
        ui.text_edit_singleline(&mut state.breakpoint_input);
        if ui.button("Add").clicked() && !state.breakpoint_input.is_empty() {
            let _ = command_sender.send(GdbCommand::SetBreakpoint(state.breakpoint_input.clone()));
            state.breakpoints.push(state.breakpoint_input.clone());
            state.breakpoint_input.clear();
        }
        
        ui.separator();
        ui.label("Breakpoints:");
        for (i, bp) in state.breakpoints.iter().enumerate() {
            ui.label(format!("#{} {}", i + 1, bp));
        }
    });
}

/// Render the registers panel
pub fn render_registers_panel(ui: &mut egui::Ui, state: &AppState) {
    ui.heading("Registers");
    
    if state.registers.is_empty() {
        ui.label("No register data available");
        return;
    }
    
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("registers_grid")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Register");
                ui.label("Value");
                ui.end_row();
                
                for register in &state.registers {
                    ui.label(&register.name);
                    ui.label(&register.value);
                    ui.end_row();
                }
            });
    });
}

/// Render the assembly panel
pub fn render_assembly_panel(ui: &mut egui::Ui, state: &AppState) {
    ui.heading("Assembly");
    
    if state.assembly_lines.is_empty() {
        ui.label("No assembly data available");
        return;
    }
    
    egui::ScrollArea::vertical().show(ui, |ui| {
        for line in &state.assembly_lines {
            ui.horizontal(|ui| {
                ui.label(&line.address);
                ui.label(&line.instruction);
            });
        }
    });
}

/// Render the stack frames panel
pub fn render_stack_panel(ui: &mut egui::Ui, state: &AppState) {
    ui.heading("Stack Frames");
    
    if state.stack_frames.is_empty() {
        ui.label("No stack frame data available");
        return;
    }
    
    egui::ScrollArea::vertical().show(ui, |ui| {
        for frame in &state.stack_frames {
            ui.horizontal(|ui| {
                let display = if let Some(func) = &frame.func {
                    format!("#{} {} @ 0x{}", frame.level, func, frame.addr)
                } else {
                    format!("#{} @ 0x{}", frame.level, frame.addr)
                };
                ui.label(display);
            });
        }
    });
}

/// Render the memory viewer panel
pub fn render_memory_panel(
    ui: &mut egui::Ui,
    state: &mut AppState,
    command_sender: &std::sync::mpsc::Sender<GdbCommand>,
) {
    ui.heading("Memory Viewer");
    
    ui.horizontal(|ui| {
        ui.label("Address:");
        ui.text_edit_singleline(&mut state.memory_address);
        ui.label("Size:");
        ui.add(egui::DragValue::new(&mut state.memory_size).clamp_range(1..=4096));
        
        if ui.button("Read").clicked() {
            let _ = command_sender.send(GdbCommand::ReadMemory(
                state.memory_address.clone(),
                state.memory_size,
            ));
        }
    });
    
    if state.memory_data.is_empty() {
        ui.label("No memory data available");
        return;
    }
    
    egui::ScrollArea::vertical().show(ui, |ui| {
        // Display memory as hex dump
        for (i, chunk) in state.memory_data.chunks(16).enumerate() {
            ui.horizontal(|ui| {
                // Address
                ui.label(format!("{:08x}", i * 16));
                
                // Hex bytes
                for byte in chunk {
                    ui.label(format!("{:02x}", byte));
                }
                
                // ASCII representation
                let ascii: String = chunk
                    .iter()
                    .map(|&b| if b.is_ascii_graphic() { b as char } else { '.' })
                    .collect();
                ui.label(ascii);
            });
        }
    });
}

/// Render the console panel
pub fn render_console_panel(ui: &mut egui::Ui, state: &AppState) {
    ui.label("Console Output:");
    egui::ScrollArea::vertical()
        .id_source("console_scroll")
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.monospace(&state.console_output);
        });
}

/// Render the panel visibility controls
pub fn render_view_menu(ui: &mut egui::Ui, state: &mut AppState, command_sender: &std::sync::mpsc::Sender<GdbCommand>) {
    ui.menu_button("File", |ui| {
        if ui.button("Exit").clicked() {
            std::process::exit(0);
        }
    });
    
    ui.menu_button("Debug", |ui| {
        if !state.is_debugging {
            if ui.button("Start Session").clicked() {
                let _ = command_sender.send(GdbCommand::StartSession);
                ui.close_menu();
            }
        } else {
            if ui.button("Stop Session").clicked() {
                let _ = command_sender.send(GdbCommand::StopSession);
                ui.close_menu();
            }
        }
        ui.separator();
        if state.is_debugging && !state.is_attached {
            if ui.button("Attach").clicked() {
                let target = match state.attach_mode {
                    AttachMode::Process => state.pid_input.clone(),
                    AttachMode::GdbServer => state.current_host_port.clone(),
                };
                let _ = command_sender.send(GdbCommand::Attach(state.attach_mode.clone(), target));
                ui.close_menu();
            }
        }
        if state.is_attached {
            if ui.button("Detach").clicked() {
                let _ = command_sender.send(GdbCommand::Detach);
                ui.close_menu();
            }
        }
    });
    
    ui.menu_button("View", |ui| {
        ui.checkbox(&mut state.show_registers, "Registers");
        ui.checkbox(&mut state.show_assembly, "Assembly");
        ui.checkbox(&mut state.show_stack, "Stack Frames");
        ui.checkbox(&mut state.show_memory, "Memory");
        ui.checkbox(&mut state.show_console, "Console");
    });
}

/// Render error messages if any
pub fn render_error_popup(ctx: &egui::Context, state: &mut AppState) {
    if state.has_error() {
        egui::Window::new("Error")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(&state.error_message);
                
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        state.clear_error();
                    }
                });
            });
    }
}
