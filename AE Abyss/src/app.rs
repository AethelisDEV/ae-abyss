use eframe::egui;
use std::sync::mpsc::{Sender, Receiver, channel};
use tokio::runtime::Runtime;
use rfd::FileDialog;
use sysinfo::System;
use std::path::PathBuf;
use similar::{TextDiff, ChangeTag};

#[derive(Clone, PartialEq)]
pub enum DiffLineType {
    Unchanged,
    Added,
    Deleted,
}

#[derive(Clone)]
pub struct DiffLine {
    pub line_type: DiffLineType,
    pub content: String,
    pub is_accepted: bool,
}

#[derive(Clone)]
pub struct FileDiff {
    pub path: PathBuf,
    pub lines: Vec<DiffLine>,
    pub is_side_by_side: bool,
}

pub enum AppMessage {
    Prompt(String),
    Plan(String), 
    ApprovePlan,  
    CancelInference, 
    TerminalResult(String, bool),
    GeneratedToken(String),
    InferenceFinished,
    DeviceStatus(String),
    Error(String),
}

#[derive(Clone)]
pub struct FileNode {
    pub name: String,
    pub path: std::path::PathBuf,
    pub is_dir: bool,
    pub children: Vec<FileNode>,
}

pub struct ChatApp {
    pub device_info: String,
    pub folder_path: Option<std::path::PathBuf>,
    pub file_tree: Vec<FileNode>,
    pub file_path: Option<std::path::PathBuf>,
    pub file_content: String,
    pub chat_history: Vec<(String, String)>,
    pub current_prompt: String,
    pub is_loading: bool,
    pub tx: Sender<AppMessage>,
    pub rx: Receiver<AppMessage>,
    pub model_tx: Sender<AppMessage>,
    pub rt: Runtime,
    
    // Resource Monitor
    pub sys: System,
    pub cpu_usage: f32,
    pub ram_used: f64,
    pub ram_total: f64,
    pub vram_info: String,
    
    // Planning Mode State
    pub pending_plan: Option<String>,
    pub is_planning_active: bool,
    pub selected_model: String,
    pub cargo_toml: String,
    pub agent_pending_edits: Vec<(String, String)>,
    pub active_diff: Option<FileDiff>,
}

impl ChatApp {
    fn init(cc: &eframe::CreationContext<'_>, model_tx: Sender<AppMessage>) -> Self {
        let (tx, rx) = channel();
        let rt = Runtime::new().unwrap();
        
        let mut visuals = egui::Visuals::dark();
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(10, 11, 16); 
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(26, 27, 38));
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(37, 39, 58);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(45, 46, 60);
        visuals.window_fill = egui::Color32::from_rgb(8, 9, 13);
        visuals.panel_fill = egui::Color32::from_rgb(10, 11, 16);
        visuals.selection.bg_fill = egui::Color32::from_rgb(180, 140, 255); 
        cc.egui_ctx.set_visuals(visuals);

        Self {
            device_info: "Abyss Initialized".to_string(),
            folder_path: None,
            file_tree: Vec::new(),
            file_path: None,
            file_content: "// AE Abyss - Ready for Vibe Coding\n// Open a folder to begin.\n\nfn main() {\n    println!(\"Hello World\");\n}".to_string(),
            chat_history: Vec::new(),
            current_prompt: "".to_string(),
            is_loading: false,
            tx,
            rx,
            model_tx,
            rt,
            sys: System::new_all(),
            cpu_usage: 0.0,
            ram_used: 0.0,
            ram_total: 0.0,
            vram_info: "N/A".to_string(),
            pending_plan: None,
            is_planning_active: true,
            selected_model: "Qwen 2.5 Coder".to_string(),
            cargo_toml: "".to_string(),
            agent_pending_edits: Vec::new(),
            active_diff: None,
        }
    }

    pub fn new(cc: &eframe::CreationContext<'_>, model_tx: Sender<AppMessage>) -> Self {
        let app = Self::init(cc, model_tx);
        Self::setup_custom_fonts(&cc.egui_ctx);
        app
    }

    /// Basic syntax highlighting logic for the code editor.
    fn syntax_highlight(ui: &egui::Ui, text: &str) -> egui::text::LayoutJob {
        use egui::text::{LayoutJob, TextFormat};
        let mut job = LayoutJob::default();
        let default_color = egui::Color32::from_rgb(220, 220, 250);
        let keyword_color = egui::Color32::from_rgb(255, 120, 100); 
        let type_color = egui::Color32::from_rgb(100, 200, 255);    
        let string_color = egui::Color32::from_rgb(230, 230, 150);  

        for word in text.split_inclusive(|c: char| !c.is_alphanumeric() && c != '_') {
            let format = if ["fn", "let", "pub", "impl", "struct", "enum", "match", "use", "mod", "crate", "Self", "return", "if", "else", "for", "while", "loop"]
                .contains(&word.trim()) {
                TextFormat::simple(egui::FontId::monospace(14.0), keyword_color)
            } else if word.starts_with('"') || (word.ends_with('"') && word.len() > 1) {
                TextFormat::simple(egui::FontId::monospace(14.0), string_color)
            } else if word.chars().next().map_or(false, |c| c.is_uppercase()) {
                TextFormat::simple(egui::FontId::monospace(14.0), type_color)
            } else {
                TextFormat::simple(egui::FontId::monospace(14.0), default_color)
            };
            job.append(word, 0.0, format);
        }
        job
    }

    fn setup_custom_fonts(ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();
        let nerd_path = "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf";
        if let Ok(nerd_data) = std::fs::read(nerd_path) {
            fonts.font_data.insert("nerd_font".to_owned(), std::sync::Arc::new(egui::FontData::from_owned(nerd_data)));
            fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "nerd_font".to_owned());
            fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap().insert(0, "nerd_font".to_owned());
        }
        ctx.set_fonts(fonts);
    }

    /// Recursively scans a directory to build a file tree.
    fn scan_folder_recursive(path: &std::path::Path) -> Vec<FileNode> {
        let mut nodes = Vec::new();
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                let is_dir = p.is_dir();
                let children = if is_dir { Self::scan_folder_recursive(&p) } else { Vec::new() };
                nodes.push(FileNode { name, path: p, is_dir, children });
            }
        }
        nodes.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
        nodes
    }

    /// Renders the explorer file tree sidebar.
    fn render_file_tree(ui: &mut egui::Ui, nodes: &[FileNode], selected_path: &mut Option<std::path::PathBuf>, content: &mut String) {
        for node in nodes {
            let icon = if node.is_dir { "\u{f07b} " } else { 
                if node.name.ends_with(".rs") { "\u{e7a8} " } else { "\u{f15b} " }
            };
            let color = if node.is_dir { egui::Color32::from_rgb(255, 230, 150) } 
                        else if node.name.ends_with(".rs") { egui::Color32::from_rgb(215, 120, 80) } 
                        else { egui::Color32::from_rgb(180, 180, 200) };

            if node.is_dir {
                egui::CollapsingHeader::new(egui::RichText::new(format!("{}{}", icon, node.name)).strong().color(color))
                    .show(ui, |ui| { Self::render_file_tree(ui, &node.children, selected_path, content); });
            } else {
                if ui.selectable_label(selected_path.as_ref() == Some(&node.path), egui::RichText::new(format!("{}{}", icon, node.name)).color(color)).clicked() {
                    if let Ok(c) = std::fs::read_to_string(&node.path) {
                        *content = c;
                        *selected_path = Some(node.path.clone());
                    }
                }
            }
        }
    }

    fn extract_code_blocks(text: &str) -> Vec<String> {
        let mut blocks = Vec::new();
        let mut current_block = String::new();
        let mut in_block = false;
        for line in text.lines() {
            if line.starts_with("```") {
                if in_block { blocks.push(current_block.trim().to_string()); current_block = String::new(); }
                in_block = !in_block;
            } else if in_block { current_block.push_str(line); current_block.push('\n'); }
        }
        blocks
    }

    fn update_stats(&mut self) {
        self.sys.refresh_specifics(sysinfo::RefreshKind::nothing().with_cpu(sysinfo::CpuRefreshKind::nothing().with_cpu_usage()).with_memory(sysinfo::MemoryRefreshKind::nothing().with_ram()));
        self.cpu_usage = self.sys.global_cpu_usage();
        self.ram_used = (self.sys.used_memory() as f64) / 1024.0 / 1024.0 / 1024.0;
    }

    /// Renders the diff visualization for pending code changes.
    fn render_diff_view(&mut self, ui: &mut egui::Ui) {
        let mut discard_all = false;
        let mut finalize_content = None;

        if let Some(diff) = &mut self.active_diff {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("\u{f440} DIFF: {}", diff.path.display())).strong().color(egui::Color32::from_rgb(100, 255, 100)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Side-by-Side").clicked() { diff.is_side_by_side = !diff.is_side_by_side; }
                    if ui.button("❌ Discard All").clicked() { discard_all = true; }
                    if ui.add(egui::Button::new("✅ Finalize & Save").fill(egui::Color32::from_rgb(37, 120, 77))).clicked() {
                        let mut final_content = String::new();
                        for line in &diff.lines {
                            match line.line_type {
                                DiffLineType::Unchanged => final_content.push_str(&line.content),
                                DiffLineType::Added if line.is_accepted => final_content.push_str(&line.content),
                                DiffLineType::Deleted if !line.is_accepted => final_content.push_str(&line.content),
                                _ => {}
                            }
                        }
                        finalize_content = Some(final_content);
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for line in &mut diff.lines {
                    let bg = match line.line_type {
                        DiffLineType::Added => egui::Color32::from_rgba_unmultiplied(0, 255, 0, 30),
                        DiffLineType::Deleted => egui::Color32::from_rgba_unmultiplied(255, 0, 0, 30),
                        DiffLineType::Unchanged => egui::Color32::TRANSPARENT,
                    };
                    egui::Frame::none().fill(bg).inner_margin(2.0).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if line.line_type != DiffLineType::Unchanged {
                                if ui.add(egui::Button::new(if line.is_accepted { "✅" } else { "⭕" }).frame(false).small()).clicked() {
                                    line.is_accepted = !line.is_accepted;
                                }
                            } else { ui.label("  "); }
                            ui.label(egui::RichText::new(&line.content).font(egui::FontId::monospace(13.0)));
                        });
                    });
                }
            });
        }

        if discard_all {
            self.active_diff = None;
        }

        if let Some(content) = finalize_content {
            if let Some(diff) = &self.active_diff {
                let _ = std::fs::write(&diff.path, &content);
                self.file_content = content;
                
                // Immediately refresh the file tree to show new files in Explorer
                if let Some(root) = &self.folder_path {
                    self.file_tree = Self::scan_folder_recursive(root);
                }
            }
            self.active_diff = None;
        }
    }

    /// Parses special [FILE_OP] blocks from the AI stream to determine file modifications.
    fn parse_agent_ops(text: &str) -> Vec<(String, String)> {
        let mut ops = Vec::new();
        let parts: Vec<&str> = text.split("[FILE_OP: ").collect();
        for part in parts.iter().skip(1) {
            if let Some(end_header) = part.find(']') {
                let mut filename = part[..end_header].trim().to_string();
                
                // SMART IDE FEATURE: Force raw .rs files into src/ path to prevent AI path mistakes
                if filename.ends_with(".rs") && !filename.contains('/') && filename != "build.rs" {
                    filename = format!("src/{}", filename);
                }

                let body = &part[end_header + 1..];
                if let Some(end_op) = body.find("[END_FILE_OP]") {
                    let content = body[..end_op].trim().to_string();
                    ops.push((filename, content));
                } else {
                    // Partial stream parsing for real-time diff rendering!
                    let content = body.trim().to_string();
                    ops.push((filename, content));
                }
            }
        }
        if ops.is_empty() {
            let mut start_idx = 0;
            while let Some(block_start) = text[start_idx..].find("```") {
                let actual_start = start_idx + block_start;
                let after_ticks = &text[actual_start + 3..];
                if let Some(newline_pos) = after_ticks.find('\n') {
                    let block_content_start = actual_start + 3 + newline_pos + 1;
                    if let Some(block_end) = text[block_content_start..].find("```") {
                        let content = text[block_content_start..block_content_start + block_end].trim().to_string();
                        let pre_text = &text[..actual_start];
                        let inferred_name = pre_text.lines().last().and_then(|line| {
                            line.split_whitespace().find(|w| w.contains('.') && (w.ends_with(".rs") || w.ends_with(".toml")))
                                .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '/').to_string())
                        });
                        ops.push((inferred_name.unwrap_or_default(), content));
                        start_idx = block_content_start + block_end + 3;
                    } else { break; }
                } else { break; }
            }
        }
        ops
    }
}

impl eframe::App for ChatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_stats();
        ctx.request_repaint();

        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                AppMessage::DeviceStatus(info) => self.device_info = info,
                AppMessage::GeneratedToken(token) => {
                    if let Some(last) = self.chat_history.last_mut() {
                        last.1.push_str(&token);
                        if token.contains("[END_FILE_OP]") || token.contains("```") || last.1.contains("[FILE_OP:") {
                           self.agent_pending_edits = Self::parse_agent_ops(&last.1);
                           if let Some((mut fname, new_content)) = self.agent_pending_edits.last().cloned() {
                               if fname.is_empty() {
                                   fname = self.file_path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "main.rs".to_string());
                               }
                               let path = self.folder_path.as_ref().map(|p| p.join(&fname)).unwrap_or_else(|| PathBuf::from(&fname));
                               if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
                               let current_content = if path.exists() { std::fs::read_to_string(&path).unwrap_or_default() } else { "".to_string() };
                               let diff = TextDiff::from_lines(&current_content, &new_content);
                               let mut diff_lines = Vec::new();
                               for change in diff.iter_all_changes() {
                                   let line_type = match change.tag() { ChangeTag::Equal => DiffLineType::Unchanged, ChangeTag::Delete => DiffLineType::Deleted, ChangeTag::Insert => DiffLineType::Added };
                                   let is_accepted = line_type == DiffLineType::Added;
                                   diff_lines.push(DiffLine { line_type, content: change.value().to_string(), is_accepted });
                               }
                               self.active_diff = Some(FileDiff { path: path.clone(), lines: diff_lines, is_side_by_side: false });
                               self.file_path = Some(path);
                               self.file_content = current_content;
                           }
                        }
                    }
                }
                AppMessage::TerminalResult(output, is_err) => { self.chat_history.push((if is_err { "ERROR" } else { "TERMINAL" }.to_string(), output)); self.is_loading = false; }
                AppMessage::InferenceFinished => self.is_loading = false,
                AppMessage::Plan(plan_text) => { self.pending_plan = Some(plan_text); self.is_loading = false; }
                AppMessage::Error(e) => { self.is_loading = false; self.chat_history.push(("System".to_string(), format!("Error: {}", e))); }
                _ => {}
            }
        }

        // --- TOP PANEL ---
        egui::TopBottomPanel::top("top").frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(26, 27, 38)).inner_margin(4.0)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                if ui.add(egui::Button::new(egui::RichText::new("\u{f07c}")).frame(false)).clicked() {
                    if let Some(path) = FileDialog::new().pick_folder() {
                        self.file_tree = Self::scan_folder_recursive(&path);
                        self.folder_path = Some(path.clone());
                        let cargo = path.join("Cargo.toml");
                        if cargo.exists() { self.cargo_toml = std::fs::read_to_string(cargo).unwrap_or_default(); }
                    }
                }
                ui.separator();
                if let Some(path) = &self.file_path {
                    ui.label(egui::RichText::new(path.file_name().unwrap_or_default().to_string_lossy()).color(egui::Color32::from_rgb(187, 154, 247)));
                    if ui.add(egui::Button::new(egui::RichText::new("\u{f0c7}")).frame(false)).clicked() { let _ = std::fs::write(path, &self.file_content); }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(&self.device_info).small().color(egui::Color32::from_rgb(187, 154, 247)));
                });
            });
        });

        // --- BOTTOM PANEL ---
        egui::TopBottomPanel::bottom("bottom").frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(26, 27, 38)).inner_margin(2.0)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                ui.label(egui::RichText::new(format!("CPU: {:.1}% | RAM: {:.1}GB", self.cpu_usage, self.ram_used)).small().color(egui::Color32::from_rgb(150, 150, 170)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("ABYSS ENGINE v1.9.7").small().strong().color(egui::Color32::from_rgb(187, 154, 247)));
                });
            });
        });

        // --- LEFT SIDEBAR (Explorer) ---
        egui::SidePanel::left("explorer").resizable(true).default_width(200.0).frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(15, 16, 25)).inner_margin(8.0)).show(ctx, |ui| {
            ui.label(egui::RichText::new("EXPLORER").small().strong().color(egui::Color32::from_rgb(187, 154, 247)));
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                if let Some(_) = &self.folder_path { Self::render_file_tree(ui, &self.file_tree, &mut self.file_path, &mut self.file_content); }
                else { ui.centered_and_justified(|ui| { ui.label("Ready to code."); }); }
            });
        });

        // --- RIGHT SIDEBAR (Chat) ---
        egui::SidePanel::right("chat_sidebar")
            .resizable(true)
            .default_width(380.0)
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(13, 14, 20)).inner_margin(egui::Margin { left: 12, right: 12, top: 8, bottom: 8 }))
            .show(ctx, |ui| {
                // HEADER — fixed top
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("ABYSS CHAT").small().strong().color(egui::Color32::from_rgb(140, 140, 160)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("✔ Review").clicked() {}
                    });
                });
                ui.separator();
                ui.add_space(4.0);

                // Calculate available space for chat vs input
                let text_style = egui::TextStyle::Body;
                let line_height = ui.text_style_height(&text_style);
                let line_count = self.current_prompt.lines().count().max(1).min(10);
                let input_frame_height = (line_count as f32 * line_height) + 55.0; 

                let available = ui.available_height();
                let scroll_height = (available - input_frame_height - 16.0).max(50.0);

                egui::ScrollArea::vertical()
                    .id_salt("chat_scroll")
                    .max_height(scroll_height)
                    .stick_to_bottom(true)
                    .auto_shrink([false, false]) 
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.add_space(4.0);
                            for (user, ai) in &self.chat_history {
                                let is_ai = user == "AI";
                                let bg = if is_ai {
                                    egui::Color32::from_rgb(20, 21, 32)
                                } else {
                                    egui::Color32::from_rgb(33, 34, 50)
                                };
                                
                                // CHAT HIJACKER: Intercept [FILE_OP] so it doesn't pollute the chat bubble
                                let mut display_ai = ai.clone();
                                if is_ai {
                                    if let Some(start_idx) = display_ai.find("[FILE_OP:") {
                                        let pre_text = &display_ai[..start_idx];
                                        let rest = &display_ai[start_idx + 9..];
                                        let filename = rest.find(']').map(|i| &rest[..i]).unwrap_or("file");
                                        
                                        if ai.contains("[END_FILE_OP]") {
                                            display_ai = format!("{}  \n\n*(✅ Successfully transmitted to Editor: {})*\n", pre_text, filename);
                                        } else {
                                            display_ai = format!("{}  \n\n*(🚀 Streaming to Editor: {})*\n", pre_text, filename);
                                        }
                                    } else if let Some(start_idx) = display_ai.find("```") {
                                        let pre_text = &display_ai[..start_idx];
                                        display_ai = format!("{}  \n\n*(🚀 Writing code block to Editor)*\n", pre_text);
                                    }
                                }

                                egui::Frame::NONE.fill(bg).corner_radius(14.0).inner_margin(12.0).show(ui, |ui| {
                                    ui.label(egui::RichText::new(user).small().strong().color(egui::Color32::from_rgb(150, 150, 180)));
                                    ui.add_space(4.0);
                                    ui.label(egui::RichText::new(&display_ai).color(egui::Color32::from_rgb(220, 220, 240)));
                                });
                                ui.add_space(8.0);
                            }
                        });
                    });

                ui.add_space(8.0);

                // --- INPUT AREA ---
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(22, 23, 34))
                    .corner_radius(16.0)
                    .inner_margin(10.0)
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 46, 68)))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("input_text_scroll")
                                .max_height(line_count as f32 * line_height + 10.0)
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::multiline(&mut self.current_prompt)
                                            .hint_text("Ask anything...")
                                            .desired_width(ui.available_width())
                                            .desired_rows(line_count)
                                            .frame(false)
                                            .margin(egui::vec2(4.0, 4.0)),
                                    );
                                });
                            
                            ui.add_space(8.0);
                            
                            ui.horizontal(|ui| {
                                if ui.add(egui::Button::new(egui::RichText::new("+ Attach").small().color(egui::Color32::from_rgb(120, 120, 140))).frame(false)).clicked() {}
                                if ui.add(egui::Button::new(
                                    egui::RichText::new(if self.is_planning_active { "∧ Planning" } else { "∧ Normal" })
                                        .small().color(egui::Color32::from_rgb(140, 140, 160))
                                ).frame(false)).clicked() { self.is_planning_active = !self.is_planning_active; }

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if self.is_loading {
                                        if ui.add(
                                            egui::Button::new(egui::RichText::new("■ Stop").strong().color(egui::Color32::WHITE))
                                                .fill(egui::Color32::from_rgb(160, 40, 40))
                                                .corner_radius(20.0),
                                        ).clicked() {
                                            self.is_loading = false;
                                        }
                                    } else {
                                        let send_btn = egui::Button::new(
                                            egui::RichText::new("→").size(18.0).strong()
                                        )
                                        .fill(egui::Color32::from_rgb(60, 80, 180))
                                        .corner_radius(20.0);
                                        if ui.add(send_btn).clicked()
                                            || ctx.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift)
                                        {
                                            if !self.current_prompt.is_empty() {
                                                self.chat_history.push(("User".to_string(), self.current_prompt.clone()));
                                                self.chat_history.push(("AI".to_string(), "".to_string()));
                                                let _ = self.model_tx.send(AppMessage::Prompt(self.current_prompt.clone()));
                                                self.current_prompt.clear();
                                                self.is_loading = true;
                                            }
                                        }
                                    }
                                });
                            });
                        });
                    });
            });
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(8, 9, 13)).inner_margin(16.0)).show(ctx, |ui| {
            if self.active_diff.is_some() {
                self.render_diff_view(ui);
            } else {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("\u{f121} EDITOR").strong().color(egui::Color32::from_rgb(187, 154, 247)));
                    
                    let mut should_delete = None;
                    if let Some(path) = &self.file_path {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let delete_btn = egui::Button::new(egui::RichText::new("\u{f1f8} Delete").strong().color(egui::Color32::WHITE))
                                .fill(egui::Color32::from_rgb(180, 50, 50))
                                .corner_radius(4.0);
                                
                            if ui.add(delete_btn).clicked() {
                                should_delete = Some(path.clone());
                            }
                            
                            ui.label(egui::RichText::new(path.file_name().unwrap_or_default().to_string_lossy()).small().color(egui::Color32::from_rgb(120, 120, 140)));
                        });
                    }
                    
                    if let Some(path_to_del) = should_delete {
                        let _ = std::fs::remove_file(path_to_del);
                        self.file_path = None;
                        self.file_content = "// File deleted.".to_string();
                        
                        if let Some(root) = &self.folder_path {
                            self.file_tree = Self::scan_folder_recursive(root);
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().id_salt("editor_scroll").show(ui, |ui| {
                    let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                        let mut job = Self::syntax_highlight(ui, string);
                        job.wrap.max_width = wrap_width;
                        ui.fonts(|f| f.layout_job(job))
                    };

                    let output = egui::TextEdit::multiline(&mut self.file_content)
                        .font(egui::FontId::monospace(14.0))
                        .desired_width(ui.available_width())
                        .desired_rows(40)
                        .frame(false)
                        .layouter(&mut layouter);
                    ui.add(output);
                });
            }
        });
    }
}
