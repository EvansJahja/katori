/// Commands that can be sent to the GDB adapter
/// 
/// This module defines the command interface for controlling GDB operations
/// from the GUI.

#[derive(Debug, Clone)]
pub enum GdbCommand {
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

#[derive(Debug, Clone, PartialEq)]
pub enum AttachMode {
    Process,
    GdbServer,
}

/// Events that come from the GDB adapter or debugging operations
#[derive(Debug)]
pub enum DebugEvent {
    RegistersUpdated(Vec<gdbadapter::Register>),
    StackFramesUpdated(Vec<gdbadapter::StackFrame>),
    AssemblyUpdated(Vec<gdbadapter::AssemblyLine>),
    ConsoleMessage(String),
    AttachSuccess(Option<u32>), // PID for process attach, None for gdbserver
    AttachFailed(String),
    DetachSuccess,
    // Command completion events
    CommandCompleted(GdbCommand),
    CommandFailed(GdbCommand, String),
    GdbConnectionLost,
    TargetStateChanged(TargetState),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TargetState {
    Running,
    Stopped,
    Detached,
}
