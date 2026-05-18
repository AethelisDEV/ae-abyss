mod model_loader;
mod app;

use anyhow::{Result, anyhow};
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use model_loader::{ModelLoader, PromptTemplate};
use app::{ChatApp, AppMessage};
use std::sync::mpsc::{channel, Sender};
use std::thread;

fn main() -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1024.0, 768.0]),
        ..Default::default()
    };

    eframe::run_native(
        "AE Abyss",
        options,
        Box::new(|cc| {
            let (tx_to_model, rx_from_ui) = channel::<AppMessage>();
            let app = ChatApp::new(cc, tx_to_model);
            let tx_to_ui = app.tx.clone();
            
            // Background Inference Thread
            thread::spawn(move || {
                if let Err(e) = run_model_loop(tx_to_ui, rx_from_ui) {
                    eprintln!("[ERROR] Model loop stopped: {}", e);
                }
            });

            Ok(Box::new(app))
        }),
    ).map_err(|e| anyhow::anyhow!("Egui run error: {}", e))?;

    Ok(())
}

fn run_model_loop(tx_to_ui: Sender<AppMessage>, rx_from_ui: std::sync::mpsc::Receiver<AppMessage>) -> Result<()> {
    let model_path = "models/qwen/Qwen2.5-Coder-7B-Instruct-Q5_K_M.gguf";
    let template = PromptTemplate::qwen();
    
    let mut device = if candle_core::utils::cuda_is_available() {
        Device::new_cuda(0).unwrap_or(Device::Cpu)
    } else {
        Device::Cpu
    };

    let mut context_window = if device.is_cuda() { 1024 } else { 8192 };
    let mut loader = ModelLoader::new(device.clone()).with_context_window(context_window);

    let (mut model, tokenizer) = match loader.load_gguf(model_path) {
        Ok(m) => {
            let _ = tx_to_ui.send(AppMessage::DeviceStatus(format!("{} ({} ctx)", if device.is_cuda() { "GPU" } else { "CPU" }, context_window)));
            m
        },
        Err(e) if e.to_string().contains("out of memory") => {
            // Fallback to CPU if GPU VRAM is insufficient
            println!("[INFO] Not enough GPU VRAM, falling back to CPU (RAM)...");
            device = Device::Cpu;
            context_window = 8192;
            loader = ModelLoader::new(device.clone()).with_context_window(context_window);
            let m = loader.load_gguf(model_path).map_err(|e| anyhow::anyhow!("CPU Fallback also failed: {}", e))?;
            let _ = tx_to_ui.send(AppMessage::DeviceStatus(format!("CPU (Fallback, {} ctx)", context_window)));
            m
        },
        Err(e) => {
            let _ = tx_to_ui.send(AppMessage::Error(e.to_string()));
            return Err(e);
        }
    };

    while let Ok(msg) = rx_from_ui.recv() {
        if let AppMessage::Prompt(user_prompt) = msg {
            let formatted_prompt = template.format(&user_prompt);
            let tokens = loader.encode(&tokenizer, &formatted_prompt)?;
            let mut logits_processor = LogitsProcessor::new(1337, Some(0.7), Some(0.9));
            
            eprintln!("[DEBUG] Prompt tokens: {:?}", tokens);
            let mut current_tokens = tokens;
            let mut pos = 0;
            let max_tokens = context_window;
            
            eprintln!("[INFO] Inference starting. Model Device: {:?}", loader.device);
            eprintln!("[INFO] Initial token count: {}", current_tokens.len());

            for i in 0..max_tokens {
                if current_tokens.is_empty() { break; }
                
                let input = Tensor::new(&current_tokens[..], &loader.device)
                    .map_err(|e| anyhow!("Tensor creation error: {}", e))?
                    .unsqueeze(0)?;
                
                eprintln!("[DEBUG] Step {}: pos={}, input_shape={:?}", i, pos, input.shape());

                let logits = match model.forward(&input, pos) {
                    Ok(l) => l,
                    Err(e) if e.to_string().contains("out of memory") => {
                        let _ = tx_to_ui.send(AppMessage::Error("GPU OOM! Please restart the application.".to_string()));
                        break;
                    }
                    Err(e) => {
                        eprintln!("[ERROR] Error during forward (pos={}): {}", pos, e);
                        return Err(e.into());
                    }
                };

                // Update position for the next sequence chunk
                pos += current_tokens.len();

                let logits = logits.squeeze(0)?; // [seq_len, vocab_size]
                let logits_row = match logits.rank() {
                    2 => {
                        let n = logits.dim(0)?;
                        logits.get(n - 1)?
                    },
                    1 => logits,
                    _ => return Err(anyhow!("Unexpected logits rank: {}", logits.rank())),
                };

                let next_token = logits_processor.sample(&logits_row).map_err(|e| anyhow!("Sampling error: {}", e))?;
                
                // Qwen2.5 stop tokens: 151645 (<|im_end|>), 151643 (<|endoftext|>)
                if next_token == 151645 || next_token == 151643 {
                    eprintln!("[INFO] EOS caught.");
                    break;
                }

                if let Ok(AppMessage::CancelInference) = rx_from_ui.try_recv() {
                    eprintln!("[INFO] Stopped by user.");
                    break;
                }

                if let Ok(text) = loader.decode(&tokenizer, &[next_token]) {
                    eprintln!("[DEBUG] Generated: '{}' ({})", text, next_token);
                    
                    if text.contains("PLAN:") {
                        let plan_msg = text.replace("PLAN:", "").trim().to_string();
                        let _ = tx_to_ui.send(AppMessage::Plan(plan_msg));
                        
                        let mut cancelled = false;
                        loop {
                            match rx_from_ui.recv() {
                                Ok(AppMessage::ApprovePlan) => break, 
                                Ok(AppMessage::CancelInference) => {
                                    eprintln!("[INFO] Stopped during planning.");
                                    cancelled = true;
                                    break; 
                                },
                                _ => {}
                            }
                        }
                        if cancelled { break; }
                    } else {
                        let _ = tx_to_ui.send(AppMessage::GeneratedToken(text));
                    }
                }

                current_tokens = vec![next_token];
            }
            let _ = tx_to_ui.send(AppMessage::InferenceFinished);
        }
    }

    Ok(())
}
