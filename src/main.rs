use katori_gui;
use gdbadapter::GdbAdapter;

fn main() {
    println!("Katori - GDB Frontend starting...");
    
    // Initialize the GDB adapter (will be used later)
    let _gdb = GdbAdapter::new();
    
    // Start the GUI application
    let exit_code = katori_gui::run_gui();
    
    println!("Application exited with code: {}", exit_code);
    std::process::exit(exit_code);
}
