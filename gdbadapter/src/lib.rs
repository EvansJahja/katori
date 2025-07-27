/// GDB Adapter module for Katori
/// 
/// This module handles communication with GDB using GDB/MI (Machine Interface)
/// and provides a high-level API for debugging operations.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, oneshot};
use thiserror::Error;

pub mod parser;
pub mod types;

pub use types::*;
pub use parser::*;

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

/// Main GDB adapter that manages the GDB process and communication
pub struct GdbAdapter {
    process: Option<Child>,
    stdin: Option<ChildStdin>,
    event_sender: mpsc::UnboundedSender<GdbEvent>,
    event_receiver: Arc<Mutex<mpsc::UnboundedReceiver<GdbEvent>>>,
    token_counter: AtomicU32,
    pending_commands: Arc<Mutex<HashMap<u32, oneshot::Sender<GdbResult>>>>,
    is_running: Arc<Mutex<bool>>,
}

impl GdbAdapter {
    /// Create a new GDB adapter instance
    pub fn new() -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        // Install custom Ctrl+C handler on Windows to prevent self-termination
        #[cfg(windows)]
        Self::install_ctrl_handler();
        
        GdbAdapter {
            process: None,
            stdin: None,
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            token_counter: AtomicU32::new(1),
            pending_commands: Arc::new(Mutex::new(HashMap::new())),
            is_running: Arc::new(Mutex::new(false)),
        }
    }
    
    #[cfg(windows)]
    /// Install a custom Ctrl+C handler to prevent self-termination when sending CTRL_C_EVENT
    fn install_ctrl_handler() {
        unsafe {
            use winapi::um::consoleapi::SetConsoleCtrlHandler;
            use winapi::shared::minwindef::{BOOL, DWORD, TRUE};
            
            // Define our custom handler function
            unsafe extern "system" fn ctrl_handler(ctrl_type: DWORD) -> BOOL {
                use winapi::um::wincon::{CTRL_C_EVENT, CTRL_BREAK_EVENT};
                
                match ctrl_type {
                    CTRL_C_EVENT => {
                        log::debug!("CTRL_HANDLER: Ignoring CTRL_C_EVENT to prevent self-termination");
                        TRUE // Return TRUE to indicate we handled it (don't terminate)
                    }
                    CTRL_BREAK_EVENT => {
                        log::debug!("CTRL_HANDLER: Ignoring CTRL_BREAK_EVENT to prevent self-termination");
                        TRUE // Return TRUE to indicate we handled it (don't terminate)
                    }
                    _ => {
                        log::debug!("CTRL_HANDLER: Unhandled control event: {}", ctrl_type);
                        0 // Let default handler handle other events
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
    
    /// Start a new GDB session
    pub async fn start_session(&mut self) -> Result<()> {
        if self.is_running() {
            return Err(GdbError::CommandError("GDB session already running".into()));
        }
        
        let gdb_path = "C:\\msys64\\mingw64\\bin\\gdb-multiarch.exe";
        
        let mut process = Command::new(gdb_path)
            .arg("--interpreter=mi3")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
            
        let stdin = process.stdin.take().ok_or_else(|| {
            GdbError::CommunicationError("Failed to get stdin handle".into())
        })?;
        
        let stdout = process.stdout.take().ok_or_else(|| {
            GdbError::CommunicationError("Failed to get stdout handle".into())
        })?;
        
        let stderr = process.stderr.take().ok_or_else(|| {
            GdbError::CommunicationError("Failed to get stderr handle".into())
        })?;
        
        self.stdin = Some(stdin);
        self.process = Some(process);
        
        // Start the output reader task for stdout
        self.start_output_reader(stdout).await;
        
        // Start the stderr reader task
        self.start_stderr_reader(stderr).await;
        
        *self.is_running.lock().unwrap() = true;
        
        Ok(())
    }
    
    /// Start the output reader task that processes GDB output
    async fn start_output_reader(&self, stdout: ChildStdout) {
        log::trace!("start_output_reader: Starting output reader task");
        let event_sender = self.event_sender.clone();
        let pending_commands = self.pending_commands.clone();
        let is_running = self.is_running.clone();
        
        tokio::spawn(async move {
            log::trace!("start_output_reader: Output reader task started");
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            
            while *is_running.lock().unwrap() {
                line.clear();
                log::trace!("start_output_reader: Waiting for next line from GDB...");
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        log::trace!("start_output_reader: EOF reached, breaking");
                        break; // EOF
                    }
                    Ok(bytes_read) => {
                        let trimmed = line.trim();
                        log::trace!("start_output_reader: Read {} bytes: '{}'", bytes_read, trimmed);
                        
                        if !trimmed.is_empty() {
                            log::trace!("start_output_reader: Parsing GDB output: '{}'", trimmed);
                            match parse_gdb_output(trimmed) {
                                Ok(output) => {
                                    log::trace!("start_output_reader: Successfully parsed output: {:?}", output);
                                    match output {
                                        GdbOutput::Result(result) => {
                                            log::trace!("start_output_reader: Processing result with token: {:?}", result.token);
                                            if let Some(token) = result.token {
                                                if let Some(sender) = pending_commands.lock().unwrap().remove(&token) {
                                                    log::trace!("start_output_reader: Sending result to waiting command with token {}", token);
                                                    let _ = sender.send(result);
                                                } else {
                                                    log::trace!("start_output_reader: No pending command found for token {}", token);
                                                }
                                            } else {
                                                log::trace!("start_output_reader: Result has no token, sending as event");
                                                let _ = event_sender.send(GdbEvent::Result(result));
                                            }
                                        }
                                        GdbOutput::Async(async_record) => {
                                            log::trace!("start_output_reader: Processing async record: {:?}", async_record);
                                            let _ = event_sender.send(GdbEvent::Async(async_record));
                                        }
                                        GdbOutput::Stream(stream) => {
                                            log::trace!("start_output_reader: Processing stream: {:?}", stream);
                                            let _ = event_sender.send(GdbEvent::Stream(stream));
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::trace!("start_output_reader: Failed to parse GDB output '{}': {}", trimmed, e);
                                }
                            }
                        } else {
                            log::trace!("start_output_reader: Empty line, skipping");
                        }
                    }
                    Err(e) => {
                        log::trace!("start_output_reader: Error reading from stdout: {}, breaking", e);
                        break;
                    }
                }
            }
            log::trace!("start_output_reader: Output reader task finished");
        });
    }
    
    /// Start the stderr reader task that processes GDB stderr output
    async fn start_stderr_reader(&self, stderr: tokio::process::ChildStderr) {
        let event_sender = self.event_sender.clone();
        let is_running = self.is_running.clone();
        
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            
            while *is_running.lock().unwrap() {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            // Print to CLI console
                            eprintln!("GDB stderr: {}", trimmed);
                            
                            // Send as a log stream event to GUI
                            let stream_record = StreamRecord {
                                stream_type: StreamType::Log,
                                content: format!("GDB stderr: {}", trimmed),
                            };
                            let _ = event_sender.send(GdbEvent::Stream(stream_record));
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }
    
    /// Send a command to GDB and wait for the result
    pub async fn send_command(&mut self, command: &str) -> Result<GdbResult> {
        log::trace!("send_command: Entering with command: '{}'", command);
        
        if !self.is_running() {
            log::trace!("send_command: GDB is not running, returning ProcessTerminated error");
            return Err(GdbError::ProcessTerminated);
        }
        
        let token = self.token_counter.fetch_add(1, Ordering::SeqCst);
        log::trace!("send_command: Generated token: {}", token);
        
        let (sender, receiver) = oneshot::channel();
        
        self.pending_commands.lock().unwrap().insert(token, sender);
        log::trace!("send_command: Inserted token {} into pending commands", token);
        
        let command_line = format!("{}-{}\n", token, command);
        log::trace!("send_command: Formatted command line: '{}'", command_line.trim());
        
        if let Some(ref mut stdin) = self.stdin {
            log::trace!("send_command: Writing command to stdin...");
            stdin.write_all(command_line.as_bytes()).await.map_err(|e| {
                log::trace!("send_command: Failed to write command to stdin: {}", e);
                GdbError::CommunicationError(format!("Failed to write command: {}", e))
            })?;
            
            log::trace!("send_command: Flushing stdin...");
            stdin.flush().await.map_err(|e| {
                log::trace!("send_command: Failed to flush stdin: {}", e);
                GdbError::CommunicationError(format!("Failed to flush command: {}", e))
            })?;
            
            log::trace!("send_command: Command sent successfully, waiting for response...");
        } else {
            log::trace!("send_command: stdin is None, returning ProcessTerminated error");
            return Err(GdbError::ProcessTerminated);
        }
        
        let result = receiver.await.map_err(|_| {
            log::trace!("send_command: Command response channel closed for token {}", token);
            GdbError::CommunicationError("Command response channel closed".into())
        });
        
        log::trace!("send_command: Received result for token {}: {:?}", token, result);
        if let Ok(r) = result {
            if r.class == ResultClass::Error {
                let error_msg = r.results.get("msg")
                    .and_then(|v| v.as_string())
                    .unwrap_or("Unknown error")
                    .to_string();
                log::error!("send_command: GDB returned error: {}", error_msg);
                return Err(GdbError::CommandError(error_msg));
            } else {
                return Ok(r);
            }
        } 
        result
    }

    pub fn get_event_receiver(&self) -> Arc<Mutex<mpsc::UnboundedReceiver<GdbEvent>>> {
        self.event_receiver.clone()
    }
    
    /// Stop the current GDB session
    pub async fn stop_session(&mut self) -> Result<()> {
        if !self.is_running() {
            return Ok(());
        }
        
        // Send quit command
        let _ = self.send_command("gdb-exit").await;
        
        // Clean up
        *self.is_running.lock().unwrap() = false;
        
        if let Some(mut process) = self.process.take() {
            let _ = process.kill().await;
        }
        
        self.stdin = None;
        
        Ok(())
    }
    
    /// Load an executable file
    pub async fn load_executable(&mut self, path: &str) -> Result<GdbResult> {
        self.send_command(&format!("file-exec-and-symbols \"{}\"", path)).await
    }

    /// Attach to a running process by PID
    pub async fn attach_to_process(&mut self, pid: u32) -> Result<GdbResult> {
        self.send_command(&format!("target-attach {}", pid)).await
    }

    /// Attach to a remote GDB server
    pub async fn attach_to_gdbserver(&mut self, host_port: &str) -> Result<GdbResult> {
        self.send_command(&format!("target-select remote {}", host_port)).await
    }

    /// Detach from current target
    pub async fn detach(&mut self) -> Result<GdbResult> {
        self.send_command("target-detach").await
    }

    /// Interrupt execution (break) - sends CTRL_C_EVENT to GDB process
    pub async fn interrupt(&mut self) -> Result<GdbResult> {
        log::debug!("INTERRUPT: Sending CTRL_C_EVENT to GDB process");
        
        if !self.is_running() {
            log::debug!("INTERRUPT: GDB not running");
            return Err(GdbError::ProcessTerminated);
        }
        
        if let Some(ref mut process) = self.process {
            if let Some(pid) = process.id() {
                log::debug!("INTERRUPT: Sending CTRL_C_EVENT to PID {}", pid);
                
                #[cfg(windows)]
                {
                    unsafe {
                        use winapi::um::wincon::{GenerateConsoleCtrlEvent, CTRL_C_EVENT};
                        
                        let result = GenerateConsoleCtrlEvent(CTRL_C_EVENT, pid);
                        if result == 0 {
                            log::error!("INTERRUPT: GenerateConsoleCtrlEvent failed");
                            return Err(GdbError::CommunicationError("Failed to send Ctrl+C event".into()));
                        } else {
                            log::debug!("INTERRUPT: Successfully sent CTRL_C_EVENT");
                        }
                    }
                }
                
                #[cfg(not(windows))]
                {
                    // On non-Windows systems, we could use SIGINT here
                    log::warn!("INTERRUPT: GenerateConsoleCtrlEvent not available on this platform");
                    return Err(GdbError::CommunicationError("Interrupt not supported on this platform".into()));
                }
                
                // Return a synthetic success result since interrupt doesn't return a MI response
                use crate::types::{GdbResult, ResultClass};
                use std::collections::HashMap;
                Ok(GdbResult {
                    token: None,
                    class: ResultClass::Done,
                    results: HashMap::new(),
                })
            } else {
                log::error!("INTERRUPT: Could not get process ID");
                return Err(GdbError::CommunicationError("Could not get process ID".into()));
            }
        } else {
            log::error!("INTERRUPT: No process handle available");
            Err(GdbError::ProcessTerminated)
        }
    }

    /// Set a breakpoint at the specified location
    pub async fn set_breakpoint(&mut self, location: &str) -> Result<GdbResult> {
        self.send_command(&format!("break-insert {}", location)).await
    }

    /// Set a breakpoint at a specific address
    pub async fn set_breakpoint_at_address(&mut self, address: &str) -> Result<GdbResult> {
        self.send_command(&format!("break-insert *{}", address)).await
    }

    /// Remove a breakpoint by number
    pub async fn remove_breakpoint(&mut self, number: u32) -> Result<GdbResult> {
        self.send_command(&format!("break-delete {}", number)).await
    }

    /// List all breakpoints
    pub async fn list_breakpoints(&mut self) -> Result<GdbResult> {
        self.send_command("break-list").await
    }

    /// Execute the target program
    pub async fn run_program(&mut self) -> Result<GdbResult> {
        self.send_command("exec-run").await
    }

    /// Continue execution
    pub async fn continue_execution(&mut self) -> Result<GdbResult> {
        self.send_command("exec-continue").await
    }

    /// Step one instruction
    pub async fn step(&mut self) -> Result<GdbResult> {
        self.send_command("exec-step").await
    }

    /// Step over one instruction (next line)
    pub async fn next(&mut self) -> Result<GdbResult> {
        self.send_command("exec-next").await
    }

    /// Step one assembly instruction
    pub async fn step_instruction(&mut self) -> Result<GdbResult> {
        self.send_command("exec-step-instruction").await
    }

    /// Step over one assembly instruction
    pub async fn next_instruction(&mut self) -> Result<GdbResult> {
        self.send_command("exec-next-instruction").await
    }

    /// Step out of current function
    pub async fn step_out(&mut self) -> Result<GdbResult> {
        self.send_command("exec-finish").await
    }

    /// Get register values
    pub async fn get_registers(&mut self) -> Result<GdbResult> {
        self.send_command("data-list-register-values x").await
    }

    /// Get register names
    pub async fn get_register_names(&mut self) -> Result<GdbResult> {
        self.send_command("data-list-register-names").await
    }

    /// Disassemble at current location
    pub async fn disassemble_current(&mut self, lines: u32) -> Result<GdbResult> {
        self.send_command(&format!("data-disassemble -s $pc -e $pc+{} -- 0", lines * 4)).await
    }

    /// Disassemble at specific address
    pub async fn disassemble_at_address(&mut self, address: &str, lines: u32) -> Result<GdbResult> {
        self.send_command(&format!("data-disassemble -s {} -e {}+{} -- 0", address, address, lines * 4)).await
    }

    /// Get stack frames
    pub async fn get_stack_frames(&mut self) -> Result<GdbResult> {
        self.send_command("stack-list-frames").await
    }

    /// Read memory at address
    pub async fn read_memory(&mut self, address: &str, size: u32) -> Result<GdbResult> {
        self.send_command(&format!("data-read-memory-bytes {} {}", address, size)).await
    }
    
    /// Check if GDB is running
    pub fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap()
    }
    
}

impl Drop for GdbAdapter {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_done_result() {
        let input = "^done";
        let result = parse_gdb_output(input).unwrap();
        
        match result {
            GdbOutput::Result(result) => {
                assert_eq!(result.class, ResultClass::Done);
                assert!(result.results.is_empty());
                assert_eq!(result.token, None);
            }
            _ => panic!("Expected result record"),
        }
    }
    
    #[test]
    fn test_parse_result_with_token() {
        let input = "123^done,bkpt={number=\"1\",type=\"breakpoint\"}";
        let result = parse_gdb_output(input).unwrap();
        
        match result {
            GdbOutput::Result(result) => {
                assert_eq!(result.class, ResultClass::Done);
                assert_eq!(result.token, Some(123));
                assert!(!result.results.is_empty());
            }
            _ => panic!("Expected result record"),
        }
    }
    
    #[test]
    fn test_parse_error_result() {
        let input = "^error,msg=\"No symbol table is loaded.\"";
        let result = parse_gdb_output(input).unwrap();
        
        match result {
            GdbOutput::Result(result) => {
                assert_eq!(result.class, ResultClass::Error);
                assert!(!result.results.is_empty());
            }
            _ => panic!("Expected result record"),
        }
    }
    
    #[test]
    fn test_parse_async_running() {
        let input = "*running,thread-id=\"all\"";
        let result = parse_gdb_output(input).unwrap();
        
        match result {
            GdbOutput::Async(async_record) => {
                assert_eq!(async_record.class, AsyncClass::Running);
                assert!(!async_record.results.is_empty());
            }
            _ => panic!("Expected async record"),
        }
    }
    
    #[test]
    fn test_parse_async_stopped() {
        let input = "*stopped,reason=\"breakpoint-hit\",thread-id=\"1\"";
        let result = parse_gdb_output(input).unwrap();
        
        match result {
            GdbOutput::Async(async_record) => {
                assert_eq!(async_record.class, AsyncClass::Stopped);
                assert!(!async_record.results.is_empty());
            }
            _ => panic!("Expected async record"),
        }
    }
    
    #[test]
    fn test_parse_stream_console() {
        let input = "~\"Hello, World!\\n\"";
        let result = parse_gdb_output(input).unwrap();
        
        match result {
            GdbOutput::Stream(stream) => {
                assert_eq!(stream.stream_type, StreamType::Console);
                assert_eq!(stream.content, "Hello, World!\n");
            }
            _ => panic!("Expected stream record"),
        }
    }
    
    #[test]
    fn test_parse_stream_target() {
        let input = "@\"target output\"";
        let result = parse_gdb_output(input).unwrap();
        
        match result {
            GdbOutput::Stream(stream) => {
                assert_eq!(stream.stream_type, StreamType::Target);
                assert_eq!(stream.content, "target output");
            }
            _ => panic!("Expected stream record"),
        }
    }
    
    #[test]
    fn test_parse_stream_log() {
        let input = "&\"debug message\"";
        let result = parse_gdb_output(input).unwrap();
        
        match result {
            GdbOutput::Stream(stream) => {
                assert_eq!(stream.stream_type, StreamType::Log);
                assert_eq!(stream.content, "debug message");
            }
            _ => panic!("Expected stream record"),
        }
    }
}
