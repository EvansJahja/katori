use gdbadapter::GdbAdapter;

#[tokio::main]
async fn main() {
    // Initialize the logger first
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .with_module_level("egui_extras", log::LevelFilter::Debug)
        .with_module_level("syntect", log::LevelFilter::Debug)
        .with_module_level("eframe", log::LevelFilter::Warn)
        .with_module_level("egui_glow", log::LevelFilter::Warn)
        .with_module_level("gdbadapter", log::LevelFilter::Trace)
        .init()
        .unwrap();
    
    log::info!("Katori - GDB Frontend starting...");
    
    // Initialize the GDB adapter (will be used later)
    let _gdb = GdbAdapter::new();
    
    // Start the GUI application
    let exit_code = katori_gui::run_gui();
    
    log::info!("Application exited with code: {exit_code}");
    std::process::exit(exit_code);
}
