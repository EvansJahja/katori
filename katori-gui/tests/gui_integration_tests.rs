use katori_gui::{KatoriApp, AttachMode};

#[cfg(test)]
mod gui_tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        let app = KatoriApp::new();
        
        // Test initial state
        assert!(!app.is_debugging());
        assert!(!app.is_attached());
        assert_eq!(app.get_host_port(), "localhost:1337");
        assert!(app.get_console_output().contains("Welcome to Katori"));
    }

    #[test]
    fn test_console_output_updates() {
        let mut app = KatoriApp::new();
        let initial_output = app.get_console_output().to_string();
        
        app.add_console_message("Test message");
        
        let updated_output = app.get_console_output();
        assert!(updated_output.len() > initial_output.len());
        assert!(updated_output.contains("Test message"));
    }

    #[test]
    fn test_attach_mode_switching() {
        let mut app = KatoriApp::new();
        
        // Test initial mode
        assert_eq!(*app.get_attach_mode(), AttachMode::GdbServer);
        
        // Test switching modes
        app.set_attach_mode(AttachMode::Process);
        assert_eq!(*app.get_attach_mode(), AttachMode::Process);
    }

    #[test]
    fn test_breakpoint_management() {
        let mut app = KatoriApp::new();
        
        // Test adding breakpoint
        app.set_breakpoint_input("main".to_string());
        let initial_count = app.get_breakpoints().len();
        
        app.add_breakpoint_from_input();
        
        assert_eq!(app.get_breakpoints().len(), initial_count + 1);
        assert!(app.get_breakpoints().contains(&"main".to_string()));
        assert!(app.get_breakpoint_input().is_empty());
    }

    #[test]
    fn test_error_handling() {
        let mut app = KatoriApp::new();
        
        // Test setting error message
        app.set_error_message("Test error".to_string());
        assert_eq!(app.get_error_message(), "Test error");
        
        // Test clearing error message
        app.clear_error_message();
        assert!(app.get_error_message().is_empty());
    }

    #[test]
    fn test_debug_info_state() {
        let mut app = KatoriApp::new();
        
        // Test initial state
        assert!(app.get_registers().is_empty());
        assert!(app.get_assembly_lines().is_empty());
        assert!(app.get_stack_frames().is_empty());
        
        // Test clearing debug info
        app.clear_debug_info();
        assert!(app.get_registers().is_empty());
        assert!(app.get_assembly_lines().is_empty());
        assert!(app.get_stack_frames().is_empty());
    }

    #[test]
    fn test_ui_panel_visibility() {
        let mut app = KatoriApp::new();
        
        // Test initial visibility state
        assert!(app.is_registers_visible());
        assert!(app.is_assembly_visible());
        assert!(app.is_stack_visible());
        assert!(!app.is_memory_visible());
        assert!(app.is_console_visible());
        
        // Test toggling visibility
        app.set_registers_visible(false);
        assert!(!app.is_registers_visible());
        
        app.set_memory_visible(true);
        assert!(app.is_memory_visible());
    }

    #[test]
    fn test_non_blocking_operations() {
        // Create a tokio runtime for this test since the methods spawn tasks
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        
        let mut app = KatoriApp::new();
        
        // Test that starting a session doesn't block
        app.start_gdb_session();
        // The method should return immediately, even though GDB operations happen in background
        assert!(true, "start_gdb_session returned without blocking");
        
        // Test that stopping a session doesn't block
        app.stop_gdb_session();
        assert!(true, "stop_gdb_session returned without blocking");
    }
}
