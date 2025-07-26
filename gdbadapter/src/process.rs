/// Process management for GDB
/// 
/// This module handles GDB process creation, lifecycle management,
/// and platform-specific signal handling.

use std::process::Stdio;
use tokio::process::{Child, Command};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProcessError {
    #[error("Failed to start GDB process: {0}")]
    StartError(#[from] std::io::Error),
    #[error("Process terminated unexpectedly")]
    Terminated,
    #[error("Signal error: {0}")]
    SignalError(String),
}

pub type Result<T> = std::result::Result<T, ProcessError>;

pub struct GdbProcess {
    child: Child,
}

impl GdbProcess {
    /// Start a new GDB process with MI interface
    pub async fn start(gdb_path: &str) -> Result<Self> {
        log::debug!("Starting GDB process: {}", gdb_path);
        
        let child = Command::new(gdb_path)
            .arg("--interpreter=mi3")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
            
        log::debug!("GDB process started with PID: {:?}", child.id());
        
        Ok(GdbProcess { child })
    }
    
    /// Get the process ID
    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }
    
    /// Take stdin handle
    pub fn take_stdin(&mut self) -> Option<tokio::process::ChildStdin> {
        self.child.stdin.take()
    }
    
    /// Take stdout handle
    pub fn take_stdout(&mut self) -> Option<tokio::process::ChildStdout> {
        self.child.stdout.take()
    }
    
    /// Take stderr handle
    pub fn take_stderr(&mut self) -> Option<tokio::process::ChildStderr> {
        self.child.stderr.take()
    }
    
    /// Send interrupt signal to the GDB process
    pub fn interrupt(&mut self) -> Result<()> {
        if let Some(pid) = self.id() {
            log::debug!("Sending interrupt to GDB PID: {}", pid);
            self.send_interrupt_signal(pid)
        } else {
            Err(ProcessError::Terminated)
        }
    }
    
    /// Kill the GDB process
    pub async fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill().await
    }
    
    /// Platform-specific interrupt signal implementation
    #[cfg(windows)]
    fn send_interrupt_signal(&self, pid: u32) -> Result<()> {
        unsafe {
            use winapi::um::wincon::{GenerateConsoleCtrlEvent, CTRL_C_EVENT};
            
            let result = GenerateConsoleCtrlEvent(CTRL_C_EVENT, pid);
            if result == 0 {
                log::error!("GenerateConsoleCtrlEvent failed for PID {}", pid);
                Err(ProcessError::SignalError("Failed to send Ctrl+C event".into()))
            } else {
                log::debug!("Successfully sent CTRL_C_EVENT to PID {}", pid);
                Ok(())
            }
        }
    }
    
    #[cfg(unix)]
    fn send_interrupt_signal(&self, pid: u32) -> Result<()> {
        unsafe {
            let result = libc::kill(pid as i32, libc::SIGINT);
            if result != 0 {
                log::error!("Failed to send SIGINT to PID {}", pid);
                Err(ProcessError::SignalError("Failed to send SIGINT".into()))
            } else {
                log::debug!("Successfully sent SIGINT to PID {}", pid);
                Ok(())
            }
        }
    }
    
    #[cfg(not(any(windows, unix)))]
    fn send_interrupt_signal(&self, _pid: u32) -> Result<()> {
        Err(ProcessError::SignalError("Interrupt not supported on this platform".into()))
    }
}

/// Install custom signal handlers to prevent self-termination when sending signals
#[cfg(windows)]
pub fn install_signal_protection() {
    unsafe {
        use winapi::um::consoleapi::SetConsoleCtrlHandler;
        use winapi::shared::minwindef::{BOOL, DWORD, TRUE};
        
        unsafe extern "system" fn ctrl_handler(ctrl_type: DWORD) -> BOOL {
            use winapi::um::wincon::{CTRL_C_EVENT, CTRL_BREAK_EVENT};
            
            match ctrl_type {
                CTRL_C_EVENT => {
                    log::debug!("CTRL_HANDLER: Ignoring CTRL_C_EVENT to prevent self-termination");
                    TRUE
                }
                CTRL_BREAK_EVENT => {
                    log::debug!("CTRL_HANDLER: Ignoring CTRL_BREAK_EVENT to prevent self-termination");
                    TRUE
                }
                _ => {
                    log::debug!("CTRL_HANDLER: Unhandled control event: {}", ctrl_type);
                    0
                }
            }
        }
        
        let result = SetConsoleCtrlHandler(Some(ctrl_handler), TRUE);
        if result == 0 {
            log::warn!("Failed to install custom Ctrl+C handler");
        } else {
            log::debug!("Successfully installed custom Ctrl+C handler");
        }
    }
}

#[cfg(not(windows))]
pub fn install_signal_protection() {
    // No special signal protection needed on non-Windows platforms
    log::debug!("Signal protection not required on this platform");
}
