# Move to Async Architecture - Implementation Checklist

## Overview
Migrate from blocking GDB operations to a non-blocking, channel-based async architecture that keeps the GUI responsive while allowing long-running operations and user interruptions.

## Architecture Goals
- ✅ GUI thread never blocks on GDB operations
- ✅ User can interact with GUI while target is running (continue/step)
- ✅ User can press Break to interrupt running target
- ✅ Background operations communicate via channels
- ✅ Graceful error handling with logging (continue on errors if still connected)
- ✅ Maintain current functionality and UI layout

## Phase 1: Channel Infrastructure ✅ (Priority: High)
### 1.1 Command Channel Setup
- [ ] Create `GdbCommand` enum for all debug operations
  - [ ] `Continue`
  - [ ] `StepOver`
  - [ ] `StepInto` 
  - [ ] `StepOut`
  - [ ] `Interrupt`
  - [ ] `SetBreakpoint(String)`
  - [ ] `RefreshDebugInfo`
  - [ ] `ReadMemory(String, u32)`
- [ ] Create command sender/receiver channels
- [ ] Add command channel to `KatoriApp`

### 1.2 Background Task Manager
- [ ] Create background task that processes `GdbCommand`s
- [ ] Implement command processing loop with timeout handling
- [ ] Add graceful shutdown for background task
- [ ] Test basic command flow (send command -> process -> send result)

### 1.3 Enhanced Event System
- [ ] Extend `DebugEvent` enum:
  - [ ] `CommandCompleted(GdbCommand)`
  - [ ] `CommandFailed(GdbCommand, String)`
  - [ ] `GdbConnectionLost`
  - [ ] `TargetStateChanged(running/stopped)`
- [ ] Update event processing in `update()` method

## Phase 2: Non-blocking Command Implementation ✅ (Priority: High)
### 2.1 Step Operations
- [ ] Convert `step_over()` to send command via channel
- [ ] Convert `step_into()` to send command via channel  
- [ ] Convert `step_out()` to send command via channel
- [ ] Remove all `block_on()` calls from step functions
- [ ] Test step operations don't freeze GUI

### 2.2 Continue/Interrupt Operations
- [ ] Convert `continue_execution()` to non-blocking
- [ ] Convert `interrupt_execution()` to non-blocking
- [ ] Ensure Break button works while target is running
- [ ] Test continue -> interrupt cycle

### 2.3 Debug Info Refresh
- [ ] Convert `refresh_debug_info()` to non-blocking
- [ ] Convert `auto_refresh_debug_info()` to use command channel
- [ ] Remove timeout-based refresh methods
- [ ] Test debug info updates after step operations

## Phase 3: Advanced Features ✅ (Priority: Medium)
### 3.1 Breakpoint Management
- [ ] Convert `set_breakpoint()` to non-blocking
- [ ] Add `RemoveBreakpoint(String)` command
- [ ] Add `ListBreakpoints` command with response
- [ ] Update breakpoint UI to handle async responses

### 3.2 Memory Operations
- [ ] Convert `read_memory()` to non-blocking
- [ ] Add proper memory display updates via events
- [ ] Add memory read progress indication

### 3.3 Session Management
- [ ] Convert attach operations to use command channel
- [ ] Convert detach operations to use command channel
- [ ] Handle connection state changes properly

## Phase 4: Error Resilience ✅ (Priority: Medium)  
### 4.1 Error Handling Strategy
- [ ] Log all GDB errors but continue operation if connection intact
- [ ] Implement connection health checking
- [ ] Add automatic reconnection for lost connections
- [ ] Display transient errors in console without stopping operation

### 4.2 Timeout Handling
- [ ] Set appropriate timeouts for different command types:
  - [ ] Step operations: 5-10 seconds
  - [ ] Continue: No timeout (until interrupt)
  - [ ] Debug info refresh: 3-5 seconds
  - [ ] Memory operations: 10 seconds
- [ ] Handle timeout gracefully (log and continue)

### 4.3 Connection Recovery
- [ ] Detect when GDB connection is lost
- [ ] Provide user option to reconnect
- [ ] Clear state appropriately on connection loss

## Phase 5: UI Responsiveness ✅ (Priority: Low)
### 5.1 Progress Indication
- [ ] Add spinner for long-running operations
- [ ] Show "Running..." status when target is executing
- [ ] Disable inappropriate controls during operations

### 5.2 Real-time Updates
- [ ] Implement periodic debug info refresh while stopped
- [ ] Add configurable refresh interval
- [ ] Stop automatic refresh when target is running

### 5.3 User Experience
- [ ] Ensure all buttons remain clickable
- [ ] Add keyboard shortcuts for common operations
- [ ] Improve status messages and feedback

## Phase 6: Testing & Validation ✅ (Priority: High)
### 6.1 Unit Tests
- [ ] Test command channel functionality
- [ ] Test background task processing
- [ ] Test error handling paths
- [ ] Test timeout scenarios

### 6.2 Integration Tests
- [ ] Test full debug session workflow
- [ ] Test step -> continue -> interrupt cycles
- [ ] Test error recovery scenarios
- [ ] Test GUI responsiveness during operations

### 6.3 Performance Testing
- [ ] Measure GUI responsiveness during heavy operations
- [ ] Test with large programs and many breakpoints
- [ ] Verify memory usage stays reasonable

## Implementation Notes

### Key Design Decisions
1. **Channel-based Communication**: Use `std::sync::mpsc` for command channel, existing channel for events
2. **Background Task**: Single async task processes all GDB commands sequentially
3. **Error Philosophy**: Log errors, continue if connection intact, fail gracefully if connection lost
4. **Timeout Strategy**: Different timeouts per operation type, no timeout for continue (until interrupt)

### Migration Strategy
1. Implement infrastructure first (channels, background task)
2. Migrate one command type at a time
3. Test thoroughly at each step
4. Keep existing blocking code as fallback during development
5. Remove blocking code only after async version is proven

### Testing Approach
- Test each phase independently
- Use both unit tests and manual testing
- Verify GUI never freezes during any operation
- Test error scenarios and edge cases

## Current Status
- **Last Updated**: 2025-01-26
- **Phase**: Planning
- **Next Steps**: Begin Phase 1.1 - Command Channel Setup

## Notes
- All blocking `block_on()` calls identified in current codebase
- Parser bug fixed, error handling improved
- Logging infrastructure in place
- Ready to begin async migration
