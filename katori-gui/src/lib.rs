use eframe::egui;

pub fn run_gui() -> i32 {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
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

struct KatoriApp {
    gdb_session_active: bool,
    loaded_executable: String,
    console_output: Vec<String>,
    breakpoints: Vec<String>,
    current_status: String,
    breakpoint_input: String,
    executable_path: String,
}

impl KatoriApp {
    fn new() -> Self {
        Self {
            gdb_session_active: false,
            loaded_executable: String::new(),
            console_output: vec![
                "Welcome to Katori GDB Frontend".to_string(),
                "Start a GDB session to begin debugging".to_string(),
            ],
            breakpoints: Vec::new(),
            current_status: "Ready".to_string(),
            breakpoint_input: String::new(),
            executable_path: String::new(),
        }
    }
    
    fn start_gdb_session(&mut self) {
        self.gdb_session_active = true;
        self.current_status = "GDB session active".to_string();
        self.console_output.push("GDB session started".to_string());
    }
    
    fn stop_gdb_session(&mut self) {
        self.gdb_session_active = false;
        self.current_status = "Ready".to_string();
        self.console_output.push("GDB session stopped".to_string());
    }
    
    fn load_executable(&mut self) {
        if !self.executable_path.is_empty() {
            self.loaded_executable = self.executable_path.clone();
            self.console_output.push(format!("Loaded executable: {}", self.executable_path));
            self.executable_path.clear();
        }
    }
    
    fn set_breakpoint(&mut self) {
        if !self.breakpoint_input.is_empty() {
            self.breakpoints.push(self.breakpoint_input.clone());
            self.console_output.push(format!("Set breakpoint at: {}", self.breakpoint_input));
            self.breakpoint_input.clear();
        }
    }
    
    fn run_program(&mut self) {
        if self.loaded_executable.is_empty() {
            self.console_output.push("No executable loaded".to_string());
        } else {
            self.console_output.push("Running program...".to_string());
        }
    }
}

impl eframe::App for KatoriApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top menu bar
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open Executable").clicked() {
                        // TODO: Implement file dialog
                        ui.close_menu();
                    }
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                
                ui.menu_button("Debug", |ui| {
                    if ui.button("Start/Stop Session").clicked() {
                        if self.gdb_session_active {
                            self.stop_gdb_session();
                        } else {
                            self.start_gdb_session();
                        }
                        ui.close_menu();
                    }
                    
                    ui.separator();
                    
                    if ui.button("Run").clicked() {
                        self.run_program();
                        ui.close_menu();
                    }
                    
                    if ui.button("Step").clicked() {
                        self.console_output.push("Step instruction".to_string());
                        ui.close_menu();
                    }
                    
                    if ui.button("Continue").clicked() {
                        self.console_output.push("Continue execution".to_string());
                        ui.close_menu();
                    }
                });
            });
        });
        
        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Katori - GDB Frontend");
            
            ui.separator();
            
            // Session control
            ui.horizontal(|ui| {
                ui.label("GDB Session:");
                if !self.gdb_session_active {
                    if ui.button("Start Debug Session").clicked() {
                        self.start_gdb_session();
                    }
                } else {
                    if ui.button("Stop Debug Session").clicked() {
                        self.stop_gdb_session();
                    }
                    ui.colored_label(egui::Color32::GREEN, "Active");
                }
            });
            
            ui.separator();
            
            // Executable loading
            ui.horizontal(|ui| {
                ui.label("Executable:");
                ui.text_edit_singleline(&mut self.executable_path);
                if ui.button("Load").clicked() {
                    self.load_executable();
                }
            });
            
            if !self.loaded_executable.is_empty() {
                ui.label(format!("Loaded: {}", self.loaded_executable));
            }
            
            ui.separator();
            
            // Breakpoint management
            ui.horizontal(|ui| {
                ui.label("Breakpoint:");
                ui.text_edit_singleline(&mut self.breakpoint_input);
                if ui.button("Set").clicked() {
                    self.set_breakpoint();
                }
            });
            
            if !self.breakpoints.is_empty() {
                ui.collapsing("Breakpoints", |ui| {
                    let mut to_remove = None;
                    for (i, bp) in self.breakpoints.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}:", i + 1));
                            ui.label(bp);
                            if ui.button("Remove").clicked() {
                                to_remove = Some(i);
                            }
                        });
                    }
                    
                    if let Some(i) = to_remove {
                        let removed = self.breakpoints.remove(i);
                        self.console_output.push(format!("Removed breakpoint: {}", removed));
                    }
                });
            }
            
            ui.separator();
            
            // Control buttons
            ui.horizontal(|ui| {
                if ui.button("Run Program").clicked() {
                    self.run_program();
                }
                
                if ui.button("Step").clicked() {
                    if self.gdb_session_active {
                        self.console_output.push("Step instruction".to_string());
                    } else {
                        self.console_output.push("No active GDB session".to_string());
                    }
                }
                
                if ui.button("Continue").clicked() {
                    if self.gdb_session_active {
                        self.console_output.push("Continue execution".to_string());
                    } else {
                        self.console_output.push("No active GDB session".to_string());
                    }
                }
            });
            
            ui.separator();
            
            // Console output
            ui.label("Console Output:");
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    for line in &self.console_output {
                        ui.label(line);
                    }
                });
        });
        
        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Status:");
                if self.gdb_session_active {
                    ui.colored_label(egui::Color32::GREEN, &self.current_status);
                } else {
                    ui.colored_label(egui::Color32::RED, &self.current_status);
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Katori GDB Frontend v0.1.0");
                });
            });
        });
    }
}
