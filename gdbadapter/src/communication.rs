/// Communication layer for GDB/MI protocol
/// 
/// This module handles the low-level communication with GDB,
/// including command sending, response parsing, and event handling.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout, ChildStderr};
use tokio::sync::{mpsc, oneshot};
use thiserror::Error;

use crate::types::{GdbResult, GdbEvent, GdbOutput, StreamRecord, StreamType, ResultClass};
use crate::parser::parse_gdb_output;

#[derive(Error, Debug)]
pub enum CommunicationError {
    #[error("Failed to write command: {0}")]
    WriteError(#[from] std::io::Error),
    #[error("Command response channel closed")]
    ChannelClosed,
    #[error("GDB returned error: {0}")]
    GdbError(String),
}

pub type Result<T> = std::result::Result<T, CommunicationError>;

/// Manages communication with GDB process
pub struct GdbCommunication {
    stdin: Option<ChildStdin>,
    token_counter: AtomicU32,
    pending_commands: Arc<Mutex<HashMap<u32, oneshot::Sender<GdbResult>>>>,
    event_sender: mpsc::UnboundedSender<GdbEvent>,
    is_running: Arc<Mutex<bool>>,
}

impl GdbCommunication {
    /// Create a new communication manager
    pub fn new(event_sender: mpsc::UnboundedSender<GdbEvent>) -> Self {
        Self {
            stdin: None,
            token_counter: AtomicU32::new(1),
            pending_commands: Arc::new(Mutex::new(HashMap::new())),
            event_sender,
            is_running: Arc::new(Mutex::new(false)),
        }
    }
    
    /// Set up communication with GDB process
    pub async fn setup(
        &mut self,
        stdin: ChildStdin,
        stdout: ChildStdout,
        stderr: ChildStderr,
    ) {
        self.stdin = Some(stdin);
        *self.is_running.lock().unwrap() = true;
        
        // Start background tasks for reading GDB output
        self.start_stdout_reader(stdout).await;
        self.start_stderr_reader(stderr).await;
    }
    
    /// Send a command to GDB and wait for response
    pub async fn send_command(&mut self, command: &str) -> Result<GdbResult> {
        if !*self.is_running.lock().unwrap() {
            return Err(CommunicationError::ChannelClosed);
        }
        
        let token = self.token_counter.fetch_add(1, Ordering::SeqCst);
        let command_line = format!("{}-{}\n", token, command);
        
        log::debug!("SEND[{}]: {} -> {}", token, command, command_line.trim());
        
        let (sender, receiver) = oneshot::channel();
        self.pending_commands.lock().unwrap().insert(token, sender);
        
        if let Some(ref mut stdin) = self.stdin {
            stdin.write_all(command_line.as_bytes()).await?;
            stdin.flush().await?;
            log::debug!("SEND[{}]: Command sent, waiting for response...", token);
        } else {
            return Err(CommunicationError::ChannelClosed);
        }
        
        let result = receiver.await.map_err(|_| CommunicationError::ChannelClosed)?;
        
        log::debug!("RECV[{}]: SUCCESS -> class={:?}", token, result.class);
        
        if result.class == ResultClass::Error {
            let error_msg = result.results.get("msg")
                .and_then(|v| v.as_string())
                .unwrap_or("Unknown error")
                .to_string();
            log::error!("RECV[{}]: GDB ERROR -> {}", token, error_msg);
            return Err(CommunicationError::GdbError(error_msg));
        }
        
        Ok(result)
    }
    
    /// Check if communication is active
    pub fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap()
    }
    
    /// Stop communication
    pub fn stop(&mut self) {
        *self.is_running.lock().unwrap() = false;
        self.stdin = None;
    }
    
    /// Start reading from GDB stdout
    async fn start_stdout_reader(&self, stdout: ChildStdout) {
        let event_sender = self.event_sender.clone();
        let pending_commands = self.pending_commands.clone();
        let is_running = self.is_running.clone();
        
        tokio::spawn(async move {
            log::debug!("GDB stdout reader started");
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            
            while *is_running.lock().unwrap() {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        log::debug!("GDB stdout: EOF reached");
                        break;
                    }
                    Ok(_) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            log::debug!("GDB_OUT: {}", trimmed);
                            Self::process_gdb_output(trimmed, &event_sender, &pending_commands);
                        }
                    }
                    Err(e) => {
                        log::error!("GDB stdout read error: {}", e);
                        break;
                    }
                }
            }
            log::debug!("GDB stdout reader finished");
        });
    }
    
    /// Start reading from GDB stderr
    async fn start_stderr_reader(&self, stderr: ChildStderr) {
        let event_sender = self.event_sender.clone();
        let is_running = self.is_running.clone();
        
        tokio::spawn(async move {
            log::debug!("GDB stderr reader started");
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            
            while *is_running.lock().unwrap() {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            eprintln!("GDB stderr: {}", trimmed);
                            
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
            log::debug!("GDB stderr reader finished");
        });
    }
    
    /// Process a line of GDB output
    fn process_gdb_output(
        line: &str,
        event_sender: &mpsc::UnboundedSender<GdbEvent>,
        pending_commands: &Arc<Mutex<HashMap<u32, oneshot::Sender<GdbResult>>>>,
    ) {
        match parse_gdb_output(line) {
            Ok(output) => {
                match output {
                    GdbOutput::Result(result) => {
                        if let Some(token) = result.token {
                            log::debug!("RECV[{}]: Result -> class={:?}", token, result.class);
                            if let Some(sender) = pending_commands.lock().unwrap().remove(&token) {
                                log::debug!("RECV[{}]: Delivering to waiting command", token);
                                let _ = sender.send(result);
                            } else {
                                log::warn!("RECV[{}]: No pending command found for token!", token);
                            }
                        } else {
                            log::debug!("RECV[NO_TOKEN]: Result without token -> class={:?}", result.class);
                            let _ = event_sender.send(GdbEvent::Result(result));
                        }
                    }
                    GdbOutput::Async(async_record) => {
                        log::debug!("ASYNC: class={:?}, token={:?}", async_record.class, async_record.token);
                        let _ = event_sender.send(GdbEvent::Async(async_record));
                    }
                    GdbOutput::Stream(stream) => {
                        log::debug!("STREAM: type={:?}, content={}", stream.stream_type, stream.content);
                        let _ = event_sender.send(GdbEvent::Stream(stream));
                    }
                }
            }
            Err(e) => {
                log::warn!("GDB_PARSE_ERROR: '{}' -> {}", line, e);
            }
        }
    }
}
