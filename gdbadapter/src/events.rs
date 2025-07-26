/// Event handling and state management for GDB adapter
/// 
/// This module provides centralized event handling and state tracking
/// for the GDB debugging session.

use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use serde::{Serialize, Deserialize};

use crate::types::{GdbEvent, AsyncRecord, AsyncClass, StreamRecord, StreamType, GdbResult};
use crate::commands::{StackFrame, Breakpoint};

/// Current state of the debugging session
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DebugState {
    /// GDB is not running
    Stopped,
    /// GDB is starting up
    Starting,
    /// Program is loaded but not running
    Loaded,
    /// Program is running
    Running,
    /// Program is paused (breakpoint, step, etc.)
    Paused,
    /// Program has exited
    Exited(i32),
    /// An error occurred
    Error(String),
}

/// Information about the current execution state
#[derive(Debug, Clone)]
pub struct ExecutionInfo {
    pub state: DebugState,
    pub current_frame: Option<StackFrame>,
    pub reason: Option<String>,
    pub signal_name: Option<String>,
    pub signal_meaning: Option<String>,
    pub exit_code: Option<i32>,
}

impl Default for ExecutionInfo {
    fn default() -> Self {
        Self {
            state: DebugState::Stopped,
            current_frame: None,
            reason: None,
            signal_name: None,
            signal_meaning: None,
            exit_code: None,
        }
    }
}

/// Console output from the debugged program
#[derive(Debug, Clone)]
pub struct ConsoleOutput {
    pub content: String,
    pub stream_type: StreamType,
    pub timestamp: std::time::SystemTime,
}

/// Event handler for GDB events
pub struct GdbEventHandler {
    execution_info: Arc<Mutex<ExecutionInfo>>,
    breakpoints: Arc<Mutex<Vec<Breakpoint>>>,
    console_output: Arc<Mutex<Vec<ConsoleOutput>>>,
    event_receiver: mpsc::UnboundedReceiver<GdbEvent>,
    
    // Callbacks for different event types
    state_change_callbacks: Vec<Box<dyn Fn(&ExecutionInfo) + Send + Sync>>,
    output_callbacks: Vec<Box<dyn Fn(&ConsoleOutput) + Send + Sync>>,
    breakpoint_callbacks: Vec<Box<dyn Fn(&[Breakpoint]) + Send + Sync>>,
}

impl GdbEventHandler {
    /// Create a new event handler
    pub fn new(event_receiver: mpsc::UnboundedReceiver<GdbEvent>) -> Self {
        Self {
            execution_info: Arc::new(Mutex::new(ExecutionInfo::default())),
            breakpoints: Arc::new(Mutex::new(Vec::new())),
            console_output: Arc::new(Mutex::new(Vec::new())),
            event_receiver,
            state_change_callbacks: Vec::new(),
            output_callbacks: Vec::new(),
            breakpoint_callbacks: Vec::new(),
        }
    }
    
    /// Get current execution info
    pub fn get_execution_info(&self) -> ExecutionInfo {
        self.execution_info.lock().unwrap().clone()
    }
    
    /// Get current breakpoints
    pub fn get_breakpoints(&self) -> Vec<Breakpoint> {
        self.breakpoints.lock().unwrap().clone()
    }
    
    /// Get recent console output
    pub fn get_console_output(&self, limit: Option<usize>) -> Vec<ConsoleOutput> {
        let output = self.console_output.lock().unwrap();
        if let Some(limit) = limit {
            output.iter().rev().take(limit).cloned().collect()
        } else {
            output.clone()
        }
    }
    
    /// Clear console output
    pub fn clear_console_output(&self) {
        self.console_output.lock().unwrap().clear();
    }
    
    /// Add state change callback
    pub fn on_state_change<F>(&mut self, callback: F)
    where
        F: Fn(&ExecutionInfo) + Send + Sync + 'static,
    {
        self.state_change_callbacks.push(Box::new(callback));
    }
    
    /// Add output callback
    pub fn on_output<F>(&mut self, callback: F)
    where
        F: Fn(&ConsoleOutput) + Send + Sync + 'static,
    {
        self.output_callbacks.push(Box::new(callback));
    }
    
    /// Add breakpoint change callback
    pub fn on_breakpoint_change<F>(&mut self, callback: F)
    where
        F: Fn(&[Breakpoint]) + Send + Sync + 'static,
    {
        self.breakpoint_callbacks.push(Box::new(callback));
    }
    
    /// Start the event handling loop
    pub async fn run(&mut self) {
        log::debug!("GDB event handler started");
        
        while let Some(event) = self.event_receiver.recv().await {
            self.handle_event(event).await;
        }
        
        log::debug!("GDB event handler stopped");
    }
    
    /// Handle a single event
    async fn handle_event(&mut self, event: GdbEvent) {
        match event {
            GdbEvent::Async(async_record) => {
                self.handle_async_record(async_record).await;
            }
            GdbEvent::Stream(stream_record) => {
                self.handle_stream_record(stream_record).await;
            }
            GdbEvent::Result(result) => {
                self.handle_result_record(result).await;
            }
        }
    }
    
    /// Handle async records (execution state changes, etc.)
    async fn handle_async_record(&mut self, record: AsyncRecord) {
        log::debug!("Handling async record: class={:?}", record.class);
        
        let mut execution_info = self.execution_info.lock().unwrap();
        let mut state_changed = false;
        
        match record.class {
            AsyncClass::Running => {
                execution_info.state = DebugState::Running;
                execution_info.current_frame = None;
                execution_info.reason = None;
                state_changed = true;
            }
            AsyncClass::Stopped => {
                execution_info.state = DebugState::Paused;
                
                // Extract stop reason
                if let Some(reason_value) = record.results.get("reason") {
                    if let Some(reason) = reason_value.as_string() {
                        execution_info.reason = Some(reason.to_string());
                        
                        // Handle specific stop reasons
                        match reason {
                            "exited-normally" => {
                                execution_info.state = DebugState::Exited(0);
                                execution_info.exit_code = Some(0);
                            }
                            "exited" => {
                                if let Some(exit_code_value) = record.results.get("exit-code") {
                                    if let Some(exit_code_str) = exit_code_value.as_string() {
                                        if let Ok(code) = exit_code_str.parse::<i32>() {
                                            execution_info.state = DebugState::Exited(code);
                                            execution_info.exit_code = Some(code);
                                        }
                                    }
                                }
                            }
                            "signal-received" => {
                                if let Some(signal_name_value) = record.results.get("signal-name") {
                                    if let Some(signal_name) = signal_name_value.as_string() {
                                        execution_info.signal_name = Some(signal_name.to_string());
                                    }
                                }
                                if let Some(signal_meaning_value) = record.results.get("signal-meaning") {
                                    if let Some(signal_meaning) = signal_meaning_value.as_string() {
                                        execution_info.signal_meaning = Some(signal_meaning.to_string());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                
                // Extract current frame information
                if let Some(frame_value) = record.results.get("frame") {
                    if let Some(frame_tuple) = frame_value.as_tuple() {
                        if let Ok(frame) = StackFrame::from_tuple(frame_tuple) {
                            execution_info.current_frame = Some(frame);
                        }
                    }
                }
                
                state_changed = true;
            }
            AsyncClass::ThreadGroupStarted => {
                // Process started
                log::debug!("Thread group started");
            }
            AsyncClass::ThreadGroupExited => {
                // Process exited
                execution_info.state = DebugState::Exited(0);
                state_changed = true;
            }
            AsyncClass::BreakpointCreated | AsyncClass::BreakpointModified | AsyncClass::BreakpointDeleted => {
                // Breakpoint changed - we'd need to refresh breakpoint list
                log::debug!("Breakpoint event: {:?}", record.class);
            }
            _ => {
                log::debug!("Unhandled async class: {:?}", record.class);
            }
        }
        
        if state_changed {
            let info = execution_info.clone();
            drop(execution_info);
            
            // Notify callbacks
            for callback in &self.state_change_callbacks {
                callback(&info);
            }
        }
    }
    
    /// Handle stream records (console output, etc.)
    async fn handle_stream_record(&mut self, record: StreamRecord) {
        let stream_type = record.stream_type.clone();
        let output = ConsoleOutput {
            content: record.content.clone(),
            stream_type: record.stream_type,
            timestamp: std::time::SystemTime::now(),
        };
        
        // Store output
        {
            let mut console_output = self.console_output.lock().unwrap();
            console_output.push(output.clone());
            
            // Keep only last 1000 entries to prevent memory bloat
            let len = console_output.len();
            if len > 1000 {
                console_output.drain(0..len - 1000);
            }
        }
        
        // Notify callbacks
        for callback in &self.output_callbacks {
            callback(&output);
        }
        
        // Also log important output
        match stream_type {
            StreamType::Console => log::debug!("CONSOLE: {}", record.content),
            StreamType::Target => log::debug!("TARGET: {}", record.content),
            StreamType::Log => log::debug!("LOG: {}", record.content),
        }
    }
    
    /// Handle result records
    async fn handle_result_record(&mut self, _result: GdbResult) {
        // Result records without tokens are typically responses to console commands
        // or other untracked operations
        log::debug!("Received untracked result record");
    }
    
    /// Update breakpoint list
    pub fn update_breakpoints(&self, breakpoints: Vec<Breakpoint>) {
        {
            let mut bp_list = self.breakpoints.lock().unwrap();
            *bp_list = breakpoints.clone();
        }
        
        // Notify callbacks
        for callback in &self.breakpoint_callbacks {
            callback(&breakpoints);
        }
    }
    
    /// Set execution state manually (for initialization, errors, etc.)
    pub fn set_execution_state(&self, state: DebugState) {
        {
            let mut execution_info = self.execution_info.lock().unwrap();
            if execution_info.state != state {
                execution_info.state = state;
                
                let info = execution_info.clone();
                drop(execution_info);
                
                // Notify callbacks
                for callback in &self.state_change_callbacks {
                    callback(&info);
                }
            }
        }
    }
}
