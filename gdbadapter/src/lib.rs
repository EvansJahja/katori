/// GDB Adapter module for Katori
/// 
/// This module provides a high-level API for debugging operations using GDB/MI.
/// The implementation is modularized for maintainability and testing.

use tokio::sync::mpsc;
use thiserror::Error;

pub mod parser;
pub mod types;
pub mod process;
pub mod communication;
pub mod commands;
pub mod events;

pub use types::*;
pub use parser::*;
pub use process::GdbProcess;
pub use communication::GdbCommunication;
pub use commands::{GdbCommands, Breakpoint, StackFrame, Variable};
pub use events::{GdbEventHandler, DebugState, ExecutionInfo, ConsoleOutput};

#[derive(Error, Debug)]
pub enum GdbError {
    #[error("Failed to start GDB process: {0}")]
    ProcessStartError(#[from] std::io::Error),
    #[error("GDB command failed: {0}")]
    CommandError(String),
    #[error("Failed to parse GDB output: {0}")]
    ParseError(String),
    #[error("GDB process terminated unexpectedly")]
    ProcessTerminated,
    #[error("Communication error: {0}")]
    CommunicationError(String),
}

pub type Result<T> = std::result::Result<T, GdbError>;

/// High-level GDB adapter that combines process management, communication, and command execution
pub struct GdbAdapter {
    process: Option<GdbProcess>,
    commands: Option<GdbCommands>,
    event_handler: Option<GdbEventHandler>,
}

impl GdbAdapter {
    /// Create a new GDB adapter instance
    pub fn new() -> Self {
        // Install signal protection on Windows
        process::install_signal_protection();
        
        GdbAdapter {
            process: None,
            commands: None,
            event_handler: None,
        }
    }
    
    /// Start a new GDB session
    pub async fn start_session(&mut self) -> Result<()> {
        // Create event channel
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        // Start GDB process
        let gdb_path = "C:\\msys64\\mingw64\\bin\\gdb-multiarch.exe";
        let mut process = GdbProcess::start(gdb_path).await
            .map_err(|e| GdbError::ProcessStartError(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        
        // Get stdio handles
        let stdin = process.take_stdin().ok_or_else(|| {
            GdbError::CommunicationError("Failed to get stdin handle".into())
        })?;
        let stdout = process.take_stdout().ok_or_else(|| {
            GdbError::CommunicationError("Failed to get stdout handle".into())
        })?;
        let stderr = process.take_stderr().ok_or_else(|| {
            GdbError::CommunicationError("Failed to get stderr handle".into())
        })?;
        
        // Set up communication
        let mut communication = GdbCommunication::new(event_sender);
        communication.setup(stdin, stdout, stderr).await;
        
        // Set up commands interface
        let commands = GdbCommands::new(communication);
        
        // Set up event handler
        let event_handler = GdbEventHandler::new(event_receiver);
        
        // Store components
        self.process = Some(process);
        self.commands = Some(commands);
        self.event_handler = Some(event_handler);
        
        Ok(())
    }
    
    /// Stop the current GDB session
    pub async fn stop_session(&mut self) -> Result<()> {
        // Send quit command if communication is available
        if let Some(ref mut commands) = self.commands {
            let _ = commands.communication_mut().send_command("gdb-exit").await;
            commands.stop();
        }
        
        // Stop the process
        if let Some(ref mut process) = self.process {
            let _ = process.kill().await;
        }
        
        // Clean up
        self.process = None;
        self.commands = None;
        self.event_handler = None;
        
        Ok(())
    }
    
    /// Get mutable reference to commands interface
    pub fn commands_mut(&mut self) -> Option<&mut GdbCommands> {
        self.commands.as_mut()
    }
    
    /// Get reference to commands interface
    pub fn commands(&self) -> Option<&GdbCommands> {
        self.commands.as_ref()
    }
    
    /// Get mutable reference to event handler
    pub fn event_handler_mut(&mut self) -> Option<&mut GdbEventHandler> {
        self.event_handler.as_mut()
    }
    
    /// Get reference to event handler
    pub fn event_handler(&self) -> Option<&GdbEventHandler> {
        self.event_handler.as_ref()
    }
    
    /// Check if GDB is running
    pub fn is_running(&self) -> bool {
        self.process.is_some() && 
        self.commands.as_ref().map_or(false, |c| c.is_running())
    }
    
    /// Interrupt execution (break)
    pub async fn interrupt(&mut self) -> Result<()> {
        if let Some(ref mut process) = self.process {
            process.interrupt().map_err(|e| GdbError::CommunicationError(e.to_string()))?;
            Ok(())
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }
    
    // Convenience methods that delegate to the commands module
    
    /// Load an executable file
    pub async fn load_executable(&mut self, path: &str) -> Result<()> {
        if let Some(commands) = &mut self.commands {
            commands.file_exec_and_symbols(path).await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(())
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }
    
    /// Set a breakpoint at the specified location
    pub async fn set_breakpoint(&mut self, location: &str) -> Result<u32> {
        if let Some(commands) = &mut self.commands {
            commands.break_insert(location).await
                .map_err(|e| GdbError::CommandError(e.to_string()))
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }
    
    /// Remove a breakpoint by number
    pub async fn remove_breakpoint(&mut self, number: u32) -> Result<()> {
        if let Some(commands) = &mut self.commands {
            commands.break_delete(number).await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(())
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }
    
    /// List all breakpoints
    pub async fn list_breakpoints(&mut self) -> Result<Vec<Breakpoint>> {
        if let Some(commands) = &mut self.commands {
            commands.break_list().await
                .map_err(|e| GdbError::CommandError(e.to_string()))
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }
    
    /// Execute the target program
    pub async fn run_program(&mut self, args: Option<&str>) -> Result<()> {
        if let Some(commands) = &mut self.commands {
            commands.exec_run(args).await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(())
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }
    
    /// Continue execution
    pub async fn continue_execution(&mut self) -> Result<()> {
        if let Some(commands) = &mut self.commands {
            commands.exec_continue().await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(())
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }
    
    /// Step one instruction
    pub async fn step(&mut self) -> Result<()> {
        if let Some(commands) = &mut self.commands {
            commands.exec_step().await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(())
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }
    
    /// Step over one instruction (next line)
    pub async fn next(&mut self) -> Result<()> {
        if let Some(commands) = &mut self.commands {
            commands.exec_next().await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(())
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }
    
    /// Get stack frames
    pub async fn get_stack_frames(&mut self, low_frame: Option<u32>, high_frame: Option<u32>) -> Result<Vec<StackFrame>> {
        if let Some(commands) = &mut self.commands {
            commands.stack_list_frames(low_frame, high_frame).await
                .map_err(|e| GdbError::CommandError(e.to_string()))
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }
    
    /// List local variables
    pub async fn get_local_variables(&mut self, print_values: bool) -> Result<Vec<Variable>> {
        if let Some(commands) = &mut self.commands {
            commands.stack_list_variables(print_values).await
                .map_err(|e| GdbError::CommandError(e.to_string()))
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }
    
    /// Evaluate expression
    pub async fn evaluate_expression(&mut self, expression: &str) -> Result<String> {
        if let Some(commands) = &mut self.commands {
            commands.data_evaluate_expression(expression).await
                .map_err(|e| GdbError::CommandError(e.to_string()))
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }
    
    // Additional convenience methods that were in the old GdbAdapter
    
    /// Attach to a running process by PID
    pub async fn attach_to_process(&mut self, pid: u32) -> Result<()> {
        if let Some(commands) = &mut self.commands {
            commands.communication_mut().send_command(&format!("target-attach {}", pid)).await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(())
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }

    /// Attach to a remote GDB server
    pub async fn attach_to_gdbserver(&mut self, host_port: &str) -> Result<()> {
        if let Some(commands) = &mut self.commands {
            commands.communication_mut().send_command(&format!("target-select remote {}", host_port)).await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(())
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }

    /// Detach from current target
    pub async fn detach(&mut self) -> Result<()> {
        if let Some(commands) = &mut self.commands {
            commands.communication_mut().send_command("target-detach").await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(())
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }

    /// Step one assembly instruction
    pub async fn step_instruction(&mut self) -> Result<()> {
        if let Some(commands) = &mut self.commands {
            commands.exec_step_instruction().await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(())
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }

    /// Step over one assembly instruction
    pub async fn next_instruction(&mut self) -> Result<()> {
        if let Some(commands) = &mut self.commands {
            commands.exec_next_instruction().await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(())
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }

    /// Step out of current function
    pub async fn step_out(&mut self) -> Result<()> {
        if let Some(commands) = &mut self.commands {
            commands.communication_mut().send_command("exec-finish").await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(())
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }

    /// Get register values
    pub async fn get_registers(&mut self) -> Result<String> {
        if let Some(commands) = &mut self.commands {
            let result = commands.communication_mut().send_command("data-list-register-values x").await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(format!("{:?}", result)) // Return debug format for now
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }

    /// Get register names
    pub async fn get_register_names(&mut self) -> Result<String> {
        if let Some(commands) = &mut self.commands {
            let result = commands.communication_mut().send_command("data-list-register-names").await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(format!("{:?}", result)) // Return debug format for now
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }

    /// Disassemble at current location
    pub async fn disassemble_current(&mut self, lines: u32) -> Result<String> {
        if let Some(commands) = &mut self.commands {
            let result = commands.communication_mut().send_command(&format!("data-disassemble -s $pc -e $pc+{} -- 0", lines * 4)).await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(format!("{:?}", result)) // Return debug format for now
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }

    /// Disassemble at specific address
    pub async fn disassemble_at_address(&mut self, address: &str, lines: u32) -> Result<String> {
        if let Some(commands) = &mut self.commands {
            let result = commands.communication_mut().send_command(&format!("data-disassemble -s {} -e {}+{} -- 0", address, address, lines * 4)).await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(format!("{:?}", result)) // Return debug format for now
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }

    /// Read memory at address
    pub async fn read_memory(&mut self, address: &str, size: u32) -> Result<String> {
        if let Some(commands) = &mut self.commands {
            let result = commands.communication_mut().send_command(&format!("data-read-memory-bytes {} {}", address, size)).await
                .map_err(|e| GdbError::CommandError(e.to_string()))?;
            Ok(format!("{:?}", result)) // Return debug format for now
        } else {
            Err(GdbError::ProcessTerminated)
        }
    }
}

