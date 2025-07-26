/// Katori GUI - A modern GDB frontend
/// 
/// This crate provides a GUI interface for debugging applications using GDB.
/// It uses the eframe/egui toolkit for the UI and communicates with GDB through
/// the gdbadapter crate.

// Re-export the main application
pub use app::KatoriApp;
pub use commands::AttachMode;

// Module declarations
mod app;
mod commands;
mod state;
mod ui;

/// Entry point for the GUI application
pub fn run_gui() -> i32 {
    // Create a tokio runtime for async operations
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = rt.enter();
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Katori - GDB Frontend"),
        ..Default::default()
    };
    
    match eframe::run_native(
        "Katori",
        options,
        Box::new(|_cc| Box::new(KatoriApp::new())),
    ) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("Error running GUI: {}", e);
            1
        }
    }
}
