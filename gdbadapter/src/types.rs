/// GDB/MI types and data structures
/// 
/// This module defines the data structures used to represent GDB/MI protocol messages.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents the different types of GDB output
#[derive(Debug, Clone, PartialEq)]
pub enum GdbOutput {
    Result(GdbResult),
    Async(AsyncRecord),
    Stream(StreamRecord),
}

/// Represents a GDB/MI result record
#[derive(Debug, Clone, PartialEq)]
pub struct GdbResult {
    pub token: Option<u32>,
    pub class: ResultClass,
    pub results: HashMap<String, Value>,
}

/// GDB/MI result classes
#[derive(Debug, Clone, PartialEq)]
pub enum ResultClass {
    Done,
    Running,
    Connected,
    Error,
    Exit,
}

/// Represents a GDB/MI async record
#[derive(Debug, Clone, PartialEq)]
pub struct AsyncRecord {
    pub token: Option<u32>,
    pub class: AsyncClass,
    pub results: HashMap<String, Value>,
}

/// GDB/MI async classes
#[derive(Debug, Clone, PartialEq)]
pub enum AsyncClass {
    // Exec async records
    Running,
    Stopped,
    
    // Notify async records
    ThreadGroupAdded,
    ThreadGroupRemoved,
    ThreadGroupStarted,
    ThreadGroupExited,
    ThreadCreated,
    ThreadExited,
    ThreadSelected,
    LibraryLoaded,
    LibraryUnloaded,
    TraceframeChanged,
    TsvCreated,
    TsvDeleted,
    TsvModified,
    BreakpointCreated,
    BreakpointModified,
    BreakpointDeleted,
    RecordStarted,
    RecordStopped,
    CmdParamChanged,
    MemoryChanged,
}

/// Represents a GDB/MI stream record
#[derive(Debug, Clone, PartialEq)]
pub struct StreamRecord {
    pub stream_type: StreamType,
    pub content: String,
}

/// Types of GDB/MI streams
#[derive(Debug, Clone, PartialEq)]
pub enum StreamType {
    Console,  // ~ prefix
    Target,   // @ prefix
    Log,      // & prefix
}

/// Represents values in GDB/MI output
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    String(String),
    List(Vec<Value>),
    Tuple(HashMap<String, Value>),
}

impl Value {
    /// Get the value as a string, if possible
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
    
    /// Get the value as a list, if possible
    pub fn as_list(&self) -> Option<&Vec<Value>> {
        match self {
            Value::List(list) => Some(list),
            _ => None,
        }
    }
    
    /// Get the value as a tuple, if possible
    pub fn as_tuple(&self) -> Option<&HashMap<String, Value>> {
        match self {
            Value::Tuple(tuple) => Some(tuple),
            _ => None,
        }
    }
}

/// Events that can be received from GDB
#[derive(Debug, Clone)]
pub enum GdbEvent {
    Result(GdbResult),
    Async(AsyncRecord),
    Stream(StreamRecord),
}

/// Breakpoint information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Breakpoint {
    pub number: String,
    pub breakpoint_type: String,
    pub disposition: String,
    pub enabled: String,
    pub address: Option<String>,
    pub function: Option<String>,
    pub file: Option<String>,
    pub fullname: Option<String>,
    pub line: Option<u32>,
    pub thread_groups: Vec<String>,
    pub times: u32,
}

/// Frame information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub address: String,
    pub function: Option<String>,
    pub args: Vec<Argument>,
    pub file: Option<String>,
    pub fullname: Option<String>,
    pub line: Option<u32>,
    pub arch: Option<String>,
}

/// Function argument
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Argument {
    pub name: String,
    pub value: String,
}

/// Stop reason for stopped events
#[derive(Debug, Clone, PartialEq)]
pub enum StopReason {
    BreakpointHit,
    WatchpointTrigger,
    ReadWatchpointTrigger,
    AccessWatchpointTrigger,
    FunctionFinished,
    LocationReached,
    WatchpointScope,
    EndSteppingRange,
    ExitedSignalled,
    Exited,
    ExitedNormally,
    SignalReceived,
    SolibEvent,
    Fork,
    Vfork,
    SyscallEntry,
    SyscallReturn,
    Exec,
    NoHistory,
}

impl StopReason {
    /// Parse a stop reason from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "breakpoint-hit" => Some(StopReason::BreakpointHit),
            "watchpoint-trigger" => Some(StopReason::WatchpointTrigger),
            "read-watchpoint-trigger" => Some(StopReason::ReadWatchpointTrigger),
            "access-watchpoint-trigger" => Some(StopReason::AccessWatchpointTrigger),
            "function-finished" => Some(StopReason::FunctionFinished),
            "location-reached" => Some(StopReason::LocationReached),
            "watchpoint-scope" => Some(StopReason::WatchpointScope),
            "end-stepping-range" => Some(StopReason::EndSteppingRange),
            "exited-signalled" => Some(StopReason::ExitedSignalled),
            "exited" => Some(StopReason::Exited),
            "exited-normally" => Some(StopReason::ExitedNormally),
            "signal-received" => Some(StopReason::SignalReceived),
            "solib-event" => Some(StopReason::SolibEvent),
            "fork" => Some(StopReason::Fork),
            "vfork" => Some(StopReason::Vfork),
            "syscall-entry" => Some(StopReason::SyscallEntry),
            "syscall-return" => Some(StopReason::SyscallReturn),
            "exec" => Some(StopReason::Exec),
            "no-history" => Some(StopReason::NoHistory),
            _ => None,
        }
    }
}

impl std::fmt::Display for StopReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            StopReason::BreakpointHit => "breakpoint-hit",
            StopReason::WatchpointTrigger => "watchpoint-trigger",
            StopReason::ReadWatchpointTrigger => "read-watchpoint-trigger",
            StopReason::AccessWatchpointTrigger => "access-watchpoint-trigger",
            StopReason::FunctionFinished => "function-finished",
            StopReason::LocationReached => "location-reached",
            StopReason::WatchpointScope => "watchpoint-scope",
            StopReason::EndSteppingRange => "end-stepping-range",
            StopReason::ExitedSignalled => "exited-signalled",
            StopReason::Exited => "exited",
            StopReason::ExitedNormally => "exited-normally",
            StopReason::SignalReceived => "signal-received",
            StopReason::SolibEvent => "solib-event",
            StopReason::Fork => "fork",
            StopReason::Vfork => "vfork",
            StopReason::SyscallEntry => "syscall-entry",
            StopReason::SyscallReturn => "syscall-return",
            StopReason::Exec => "exec",
            StopReason::NoHistory => "no-history",
        };
        write!(f, "{}", s)
    }
}
