use anyhow::{Result, anyhow};
use candle_core::Device;
use candle_transformers::models::quantized_qwen2::ModelWeights;
use std::path::Path;
use tokenizers::Tokenizer;

/// Structure managing Prompt templates for models.
/// ChatML format is used for Qwen2.5-Coder.
pub struct PromptTemplate {
    pub name: String,
    pub prefix: String,
    pub suffix: String,
}

impl PromptTemplate {
    /// Qwen2.5 (ChatML) format: <|im_start|>user\n{prompt}<|im_end|>\n<|im_start|>assistant\n
    pub fn qwen() -> Self {
        Self {
            name: "Qwen2.5 (ChatML)".to_string(),
            prefix: "<|im_start|>system\nYou are AE Abyss, a high-density, no-fluff coding agent. \n\n### NO-YAPPING POLICY\n1. DO NOT use conversational filler like 'Sure', 'Of course', 'Doing it', 'Okay'.\n2. DO NOT explain what you are doing unless explicitly asked. \n3. Be terse. Be solution-oriented. Be elite.\n\n### AGENTIC PROTOCOL (CRITICAL)\nNEVER output standard markdown code blocks like ```rust. If you use markdown, the system will CRASH and TRIGGER A FATAL ERROR. You MUST exclusively use the custom file operation protocol for writing ALL code.\n\nFor any project file creation or modification (including Cargo.toml, build.rs, or any other necessary files), strictly use:\n[FILE_OP: src/main.rs]\nCode goes here...\n[END_FILE_OP]\n\nDO NOT wrap the above block in markdown. Only output raw [FILE_OP] blocks. If a Cargo.toml is missing, YOU MUST CREATE IT first. Never give up.<|im_end|>\n<|im_start|>user\n".to_string(),
            suffix: "<|im_end|>\n<|im_start|>assistant\n".to_string(),
        }
    }

    /// DeepSeek Coder format: <|user|>\n{prompt}<|assistant|>\n
    pub fn deepseek() -> Self {
        Self {
            name: "DeepSeek".to_string(),
            prefix: "<|user|>\n".to_string(),
            suffix: "<|assistant|>\n".to_string(),
        }
    }

    /// Plain text (No template)
    pub fn plain() -> Self {
        Self {
            name: "Plain Text".to_string(),
            prefix: "".to_string(),
            suffix: "".to_string(),
        }
    }

    /// Applies the template to the given prompt
    pub fn format(&self, prompt: &str) -> String {
        format!("{}{}{}", self.prefix, prompt, self.suffix)
    }
}

pub struct ModelLoader {
    pub device: Device,
    pub context_window: usize,
}

impl ModelLoader {
    pub fn new(device: Device) -> Self {
        Self {
            device,
            context_window: 2048, // Safer for 8GB VRAM than 8192
        }
    }

    pub fn with_context_window(mut self, size: usize) -> Self {
        self.context_window = size;
        self
    }

    /// Loads a GGUF model and its associated tokenizer from the given path.
    pub fn load_gguf<P: AsRef<Path>>(&self, path: P) -> Result<(ModelWeights, Tokenizer)> {
        let path = path.as_ref();
        
        let mut file = std::fs::File::open(path)
            .map_err(|e| anyhow!("GGUF file could not be opened: {}. Path: {:?}", e, path))?;
        
        println!("[INFO] Loading GGUF model: {:?}", path);
        
        let mut content = candle_core::quantized::gguf_file::Content::read(&mut file)
            .map_err(|e| anyhow!("GGUF content read error: {}", e))?;
            
        // --- Metadata Diagnostics (Audit Mode) ---
        {
            use std::io::Write;
            if let Ok(mut f) = std::fs::File::create("metadata_dump.txt") {
                for (k, v) in content.metadata.iter() {
                    let _ = writeln!(f, "{}: {:?}", k, v);
                }
            }
        }

        // --- NATIVE QWEN2 SUPPORT (CRITICAL FIX) ---
        // Force RoPE frequency (1.0M) for Qwen2.5-Coder to avoid nonsense output.
        content.metadata.insert(
            "qwen2.rope.freq_base".to_string(), 
            candle_core::quantized::gguf_file::Value::F32(1000000.0)
        );

        let model = ModelWeights::from_gguf(content, &mut file, &self.device)
            .map_err(|e| anyhow!("Native Qwen2 GGUF loading error: {}", e))?;

        let parent = path.parent().ok_or_else(|| anyhow!("Invalid file path"))?;
        let tokenizer_path = parent.join("tokenizer.json");
        
        if !tokenizer_path.exists() {
            return Err(anyhow!("'tokenizer.json' not found. Please place it in the same folder as the GGUF file."));
        }
        
        println!("[INFO] Loading tokenizer: {:?}", tokenizer_path);
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow!("Tokenizer loading error: {}", e))?;

        Ok((model, tokenizer))
    }

    /// Encodes a prompt into token IDs, handling ChatML tags as atomic units.
    pub fn encode(&self, tokenizer: &Tokenizer, prompt: &str) -> Result<Vec<u32>> {
        // Qwen2.5 ChatML Tags correspond to specific IDs
        // <|im_start|> = 151644
        // <|im_end|>   = 151645
        // <|endoftext|> = 151643
        
        let mut all_tokens = Vec::new();
        let parts = prompt.split("<|im_start|>").collect::<Vec<_>>();
        
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                all_tokens.push(151644); 
            }
            
            let subparts = part.split("<|im_end|>").collect::<Vec<_>>();
            for (j, subpart) in subparts.iter().enumerate() {
                if j > 0 {
                    all_tokens.push(151645);
                }
                
                if !subpart.is_empty() {
                    let part_tokens = tokenizer
                        .encode(*subpart, false)
                        .map_err(|e| anyhow!("Tokenize part error: {}", e))?;
                    all_tokens.extend(part_tokens.get_ids());
                }
            }
        }
        
        Ok(all_tokens)
    }

    /// Decodes token IDs back into a string.
    pub fn decode(&self, tokenizer: &Tokenizer, tokens: &[u32]) -> Result<String> {
        tokenizer
            .decode(tokens, true)
            .map_err(|e| anyhow!("Decode error: {}", e))
    }
}
