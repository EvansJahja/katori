# Katori GUI Rewrite - Status and TODO

## Project Overview
The Katori debugger GUI has been successfully refactored with a modular architecture while restoring the classic look and behavior of the original interface.

## Completed Features ✅

### Core Architecture
- **Modular Design**: Split into clean modules (`state`, `ui`, `app`, `commands`, `events`)
- **Async Communication**: Non-blocking command processing with tokio runtime
- **Event-Driven Updates**: Responsive UI updates triggered by backend events
- **Error Handling**: Proper error logging and user feedback throughout

### GUI Restoration
- **Default Settings**: Restored `localhost:1337` as default port, `GdbServer` as default mode
- **Enhanced Toolbar**: Complete set of debug controls (Start/Stop Session, Step Over/Into/Out, Continue, Interrupt)
- **Menu Panels**: Functional Attach, Breakpoints, and Console panels
- **Welcome Message**: Classic welcome message restored with proper formatting
- **Console Output**: Auto-scrolling console with proper message display

### Debug Functionality
- **Session Management**: Functional Start/Stop Session buttons with proper state handling
- **Attach Operations**: Working attach to GDB server and process with real backend communication
- **Step Commands**: All step operations (Over, Into, Out) properly wired to backend
- **Auto-Refresh**: Automatic refresh of debug info after attach and step operations
- **State Management**: Proper debug state tracking and UI synchronization

### Backend Integration
- **GdbAdapter Integration**: Full integration with existing GDB/MI adapter
- **Command Processing**: Robust command queue and processing system
- **Event Handling**: Complete event system for UI updates
- **Error Recovery**: Proper error handling and user feedback

## Remaining Tasks 🔧

### High Priority
1. **Use Instruction-Level Commands**
   - Replace `step` with `step_instruction` for assembly-level debugging
   - Similarly update `step_over` to `step_over_instruction`
   - Update `step_out` to use instruction-level variant
   - Ensure all step commands work at instruction granularity

2. **Implement Scroll IDs**
   - Add scroll ID tracking for console output
   - Implement proper scroll position management
   - Ensure console maintains scroll state during updates
   - Add scroll-to-bottom functionality for new messages

### Medium Priority
3. **Enhanced Debug Info Display**
   - Improve register display formatting and organization
   - Add proper stack frame visualization
   - Implement assembly code display with syntax highlighting
   - Add memory view panel

4. **UI Polish**
   - Fine-tune panel layouts and sizing
   - Add keyboard shortcuts for common operations
   - Improve visual feedback for command execution
   - Add progress indicators for long-running operations

5. **Testing and Validation**
   - Expand integration tests beyond basic construction
   - Add tests for command processing and event handling
   - Test error scenarios and recovery
   - Validate performance with large debug sessions

### Low Priority
6. **Advanced Features**
   - Add breakpoint management (set, remove, enable/disable)
   - Implement watchpoint support
   - Add expression evaluation
   - Support for multiple debug targets

7. **Code Quality**
   - Clean up remaining lint warnings (unused fields, dead code)
   - Add comprehensive documentation
   - Optimize performance for large codebases
   - Add configuration file support

## Technical Notes

### Current Architecture
```
katori-gui/
├── src/
│   ├── lib.rs          # Public API and exports
│   ├── app.rs          # Main application logic and event handling
│   ├── state.rs        # Application state management
│   ├── ui.rs           # UI rendering and panel management
│   ├── commands.rs     # Command definitions and event types
│   └── events.rs       # Event processing (future expansion)
```

### Key Design Decisions
- **Async-First**: All backend communication is non-blocking
- **Event-Driven**: UI updates are triggered by backend events, not polling
- **Modular**: Clear separation of concerns between UI, state, and logic
- **Maintainable**: Clean abstractions make future enhancements easier

### API Compatibility
- Full compatibility with existing `GdbAdapter` from `gdbadapter` crate
- Proper usage of `gdbadapter::types` for data structures
- Correct field mappings for register and stack frame data

## Testing Status
- ✅ Basic app construction tests
- ✅ AttachMode variant tests  
- ✅ Non-blocking creation tests
- ✅ Multiple instance tests
- ❌ Command processing tests (needed)
- ❌ Event handling tests (needed)
- ❌ UI interaction tests (needed)

## Build Status
- ✅ Compiles successfully with `cargo build`
- ⚠️ Minor warnings about unused fields (intentional, for future use)
- ✅ No critical errors or API mismatches
- ✅ Full integration with existing `gdbadapter` crate

## Next Session Goals
1. Implement instruction-level step commands
2. Add scroll ID management for console
3. Expand test coverage for command/event system
4. Polish UI layout and responsiveness

---
*Last updated: July 27, 2025*
*Status: Ready for instruction-level debugging and scroll management*
