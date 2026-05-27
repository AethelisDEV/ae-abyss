# AE Abyss 🌌

AE Abyss is a high-performance, "vibe coding" focused local AI coding engine written in Rust. It runs entirely on local models (DeepSeek, Qwen) using quantized formats, ensuring ultimate privacy and millisecond-level responsiveness.

🚀 What Does This Engine Do?

AE Abyss provides developers with an intelligent coding assistant that runs completely on local hardware, requiring no internet connection.

* **GGUF Support:** Run state-of-the-art models like DeepSeek and Qwen in highly optimized quantized (GGUF) formats.
* **Local & Secure:** Zero telemetry. Your codebase never leaves your local NVMe; absolute privacy is the top priority.
* **Vibe Coding Architecture:** Features a non-blocking interface designed to accelerate the modern "vibe coding" development workflow with automatic file tree operations and interactive multi-step planning capabilities.
* **Low Latency:** High-speed token generation performance on NVIDIA GPUs with native CUDA acceleration via the Candle framework.

🛠️ How to Install

To compile and run the project, make sure you have Rust (Cargo) and the CUDA Toolkit (if using an NVIDIA GPU) installed on your system.

### 1. Clone the Repository

git clone https://github.com/AethelisDEV/ae-abyss.git
cd ae-abyss

### 2. Prepare the Models

Place your .gguf model files inside the models/ directory. Check models/README.md for specific naming conventions and configurations.

### 3. Build & Run (Optimized)

cargo run --release

🔑 Is an API Key Required?

**No.** AE Abyss is designed to work entirely offline. You do not need any API keys from OpenAI, Anthropic, or Google. Simply download the weight files and let your GPU do the heavy lifting.

📦 Core Dependencies

The project is built on top of the modern Rust ecosystem following absolute safe standards:
* **Candle:** A lightweight, minimalist machine learning framework by HuggingFace for tensor operations.
* **egui:** An extremely fast, immediate-mode GUI library for zero-stutter rendering.
* **Tokio:** Production-grade asynchronous runtime for handling model loading and stream orchestration.

📄 License

This project is licensed under the **Apache License 2.0**. For more details, see the `LICENSE` and `NOTICE` files included in this repository.

Developed with 🦀 by [AethelisDEV](https://github.com/AethelisDEV)
