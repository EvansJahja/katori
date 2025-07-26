/// Application state management
/// 
/// This module manages the overall state of the debugging session
/// and coordinates between different UI components.

use crate::commands::{AttachMode, TargetState};

/// Main application state that holds all debugging and UI state
#[derive(Debug)]
pub struct AppState {
    /// Debug session state
    pub is_debugging: bool,
    pub is_attached: bool,
    pub current_pid: Option<u32>,
    pub current_host_port: String,
    pub target_state: TargetState,
    
    /// UI state
    pub attach_mode: AttachMode,
    pub console_output: String,
    pub error_message: String,
    
    /// Debug information
    pub registers: Vec<gdbadapter::Register>,
    pub assembly_lines: Vec<gdbadapter::AssemblyLine>,
    pub stack_frames: Vec<gdbadapter::StackFrame>,
    pub breakpoints: Vec<String>,
    
    /// UI panels visibility
    pub show_registers: bool,
    pub show_assembly: bool,
    pub show_stack: bool,
    pub show_memory: bool,
    pub show_console: bool,
    
    /// Memory viewer state
    pub memory_address: String,
    pub memory_size: u32,
    pub memory_data: Vec<u8>,
    
    /// Input fields
    pub breakpoint_input: String,
    pub pid_input: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
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
}

impl AppState {
    /// Create a new application state
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Reset debug session state
    pub fn reset_debug_state(&mut self) {
        self.is_debugging = false;
        self.is_attached = false;
        self.current_pid = None;
        self.target_state = TargetState::Detached;
        self.registers.clear();
        self.assembly_lines.clear();
        self.stack_frames.clear();
        self.breakpoints.clear();
        self.memory_data.clear();
        self.error_message.clear();
    }
    
    /// Add a console message
    pub fn add_console_message(&mut self, message: String) {
        self.console_output.push_str(&message);
        self.console_output.push('\n');
        
        // Keep only last 1000 lines to prevent memory bloat
        let lines: Vec<&str> = self.console_output.lines().collect();
        if lines.len() > 1000 {
            self.console_output = lines[lines.len() - 1000..].join("\n");
        }
    }
    
    /// Set error message
    pub fn set_error(&mut self, error: String) {
        self.error_message = error;
        log::error!("GUI Error: {}", self.error_message);
    }
    
    /// Clear error message
    pub fn clear_error(&mut self) {
        self.error_message.clear();
    }
    
    /// Check if any error is present
    pub fn has_error(&self) -> bool {
        !self.error_message.is_empty()
    }
}
