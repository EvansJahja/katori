# GDB Adapter Usage Guide

## Overview

The `gdbadapter` crate provides a comprehensive Rust interface to GDB using the GDB/MI (Machine Interface) protocol. It handles process management, command execution, and output parsing automatically.

## Quick Start

### Basic Usage

```rust
use gdbadapter::{GdbAdapter, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let mut adapter = GdbAdapter::new();
    
    // Start GDB session
    adapter.start_session().await?;
    
    // Load an executable
    adapter.load_executable("./my_program").await?;
    
    // Set a breakpoint
    adapter.set_breakpoint("main").await?;
    
    // Run the program
    adapter.run_program().await?;
    
    // Stop the session
    adapter.stop_session().await?;
    
    Ok(())
}
```

### Attach to GDB Server

```rust
use gdbadapter::{GdbAdapter, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let mut adapter = GdbAdapter::new();
    
    // Start GDB session
    adapter.start_session().await?;
    
    // Attach to GDB server (e.g., gdbserver running on localhost:1337)
    adapter.attach_to_gdbserver("localhost:1337").await?;
    
    // Set breakpoints by address (useful when no symbols available)
    adapter.set_breakpoint_at_address("0x12345678").await?;
    
    // Continue execution
    adapter.continue_execution().await?;
    
    // Get assembly at current location
    let disasm = adapter.disassemble_current(20).await?;
    
    // Get register values
    let registers = adapter.get_registers().await?;
    
    // Detach when done
    adapter.detach().await?;
    adapter.stop_session().await?;
    
    Ok(())
}
```

### Event Handling

```rust
use gdbadapter::{GdbAdapter, GdbEvent, AsyncClass};

#[tokio::main]
async fn main() -> Result<()> {
    let mut adapter = GdbAdapter::new();
    adapter.start_session().await?;
    
    // Check for events (non-blocking)
    while let Some(event) = adapter.try_recv_event() {
        match event {
            GdbEvent::Async(async_record) => {
                match async_record.class {
                    AsyncClass::Stopped => {
                        if let Some(reason) = async_record.results.get("reason") {
                            println!("Program stopped: {:?}", reason);
                        }
                    }
                    AsyncClass::Running => {
                        println!("Program is running");
                    }
                    _ => {}
                }
            }
            GdbEvent::Stream(stream) => {
                println!("GDB output: {}", stream.content);
            }
            _ => {}
        }
    }
    
    Ok(())
}
```

## API Reference

### Core Types

#### `GdbAdapter`
Main interface for GDB communication.

**Methods:**
- `new() -> Self` - Create a new adapter instance
- `start_session() -> Result<()>` - Start GDB process
- `stop_session() -> Result<()>` - Stop GDB process
- `send_command(cmd: &str) -> Result<GdbResult>` - Send raw GDB/MI command
- `is_running() -> bool` - Check if GDB is running
- `try_recv_event() -> Option<GdbEvent>` - Get next event (non-blocking)

**Debugging Commands:**
- `load_executable(path: &str) -> Result<GdbResult>` - Load executable file
- `attach_to_process(pid: u32) -> Result<GdbResult>` - Attach to running process
- `attach_to_gdbserver(host_port: &str) -> Result<GdbResult>` - Attach to GDB server
- `detach() -> Result<GdbResult>` - Detach from current target
- `interrupt() -> Result<GdbResult>` - Interrupt execution (break)
- `set_breakpoint(location: &str) -> Result<GdbResult>` - Set breakpoint
- `set_breakpoint_at_address(address: &str) -> Result<GdbResult>` - Set breakpoint at address
- `remove_breakpoint(number: u32) -> Result<GdbResult>` - Remove breakpoint
- `list_breakpoints() -> Result<GdbResult>` - List all breakpoints
- `run_program() -> Result<GdbResult>` - Start program execution
- `continue_execution() -> Result<GdbResult>` - Continue execution
- `step() -> Result<GdbResult>` - Step one instruction
- `next() -> Result<GdbResult>` - Step over one instruction
- `step_instruction() -> Result<GdbResult>` - Step one assembly instruction
- `next_instruction() -> Result<GdbResult>` - Step over one assembly instruction
- `step_out() -> Result<GdbResult>` - Step out of current function
- `get_registers() -> Result<GdbResult>` - Get register values
- `get_register_names() -> Result<GdbResult>` - Get register names
- `disassemble_current(lines: u32) -> Result<GdbResult>` - Disassemble at current location
- `disassemble_at_address(address: &str, lines: u32) -> Result<GdbResult>` - Disassemble at address
- `get_stack_frames() -> Result<GdbResult>` - Get stack frames
- `read_memory(address: &str, size: u32) -> Result<GdbResult>` - Read memory

#### `GdbEvent`
Events received from GDB.

```rust
pub enum GdbEvent {
    Result(GdbResult),      // Command results
    Async(AsyncRecord),     // Async notifications
    Stream(StreamRecord),   // Console/target/log output
}
```

#### `GdbResult`
Result of a GDB command.

```rust
pub struct GdbResult {
    pub token: Option<u32>,                    // Command token
    pub class: ResultClass,                    // Result type
    pub results: HashMap<String, Value>,       // Result data
}

pub enum ResultClass {
    Done,       // Command successful
    Running,    // Program started running
    Connected,  // Connected to target
    Error,      // Command failed
    Exit,       // GDB exiting
}
```

#### `AsyncRecord`
Asynchronous notifications from GDB.

```rust
pub struct AsyncRecord {
    pub token: Option<u32>,
    pub class: AsyncClass,
    pub results: HashMap<String, Value>,
}

pub enum AsyncClass {
    // Execution state changes
    Running,
    Stopped,
    
    // Thread/process management
    ThreadGroupAdded,
    ThreadGroupStarted,
    ThreadCreated,
    ThreadSelected,
    
    // Breakpoint changes
    BreakpointCreated,
    BreakpointModified,
    BreakpointDeleted,
    
    // And many more...
}
```

#### `Value`
Represents GDB/MI values.

```rust
pub enum Value {
    String(String),
    List(Vec<Value>),
    Tuple(HashMap<String, Value>),
}
```

**Helper methods:**
- `as_string() -> Option<&str>` - Get as string
- `as_list() -> Option<&Vec<Value>>` - Get as list
- `as_tuple() -> Option<&HashMap<String, Value>>` - Get as tuple

## GDB/MI Protocol Parsing

The adapter automatically parses all GDB/MI output formats:

### Result Records
```
^done
^running
^error,msg="Error message"
123^done,bkpt={number="1",type="breakpoint"}
```

### Async Records
```
*running,thread-id="all"
*stopped,reason="breakpoint-hit",thread-id="1"
=breakpoint-created,bkpt={number="1"}
=thread-group-started,id="i1",pid="12345"
```

### Stream Records
```
~"Console output\n"
@"Target output"
&"Log message"
```

## Configuration

### GDB Executable Path
Currently hardcoded to `C:\msys64\mingw64\bin\gdb-multiarch.exe`. This can be modified in the source code or made configurable in future versions.

### MI Version
Uses GDB/MI version 3 (`--interpreter=mi3`).

## Error Handling

All operations return `Result<T, GdbError>`:

```rust
pub enum GdbError {
    ProcessStartError(std::io::Error),  // Failed to start GDB
    CommandError(String),               // GDB command failed
    ParseError(String),                 // Failed to parse output
    ProcessTerminated,                  // GDB process died
    CommunicationError(String),         // I/O error
}
```

## Testing

The crate includes comprehensive tests:

```bash
# Run all tests
cargo test -p gdbadapter

# Run specific test categories
cargo test -p gdbadapter test_parse_
cargo test -p gdbadapter --test integration_tests
```

### Test Coverage

- **Parser Tests**: Verify correct parsing of all GDB/MI output formats
- **Integration Tests**: Test complex scenarios with real GDB/MI data
- **Unit Tests**: Test individual components and edge cases

## Examples

### Setting Multiple Breakpoints

```rust
let locations = ["main", "foo.c:42", "*0x12345"];
for location in &locations {
    match adapter.set_breakpoint(location).await {
        Ok(result) => {
            if let Some(bkpt) = result.results.get("bkpt") {
                println!("Set breakpoint: {:?}", bkpt);
            }
        }
        Err(e) => eprintln!("Failed to set breakpoint at {}: {}", location, e),
    }
}
```

### Handling Program Execution

```rust
// Start the program
adapter.run_program().await?;

// Wait for stop events
loop {
    if let Some(GdbEvent::Async(async_record)) = adapter.try_recv_event() {
        if async_record.class == AsyncClass::Stopped {
            if let Some(Value::String(reason)) = async_record.results.get("reason") {
                match reason.as_str() {
                    "breakpoint-hit" => {
                        println!("Hit breakpoint!");
                        // Continue execution
                        adapter.continue_execution().await?;
                    }
                    "exited-normally" => {
                        println!("Program finished");
                        break;
                    }
                    _ => println!("Stopped: {}", reason),
                }
            }
        }
    }
}
```

### Custom Command Execution

```rust
// Send custom GDB/MI command
let result = adapter.send_command("stack-list-frames").await?;

if result.class == ResultClass::Done {
    if let Some(Value::List(frames)) = result.results.get("stack") {
        for frame in frames {
            println!("Frame: {:?}", frame);
        }
    }
}
```

## Future Enhancements

- [ ] Configurable GDB executable path
- [ ] File dialog integration for GUI
- [ ] Variable inspection support
- [ ] Memory debugging features
- [ ] Remote debugging support
- [ ] Core dump analysis
- [ ] Performance profiling integration
