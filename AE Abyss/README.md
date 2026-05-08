# AE Abyss

AE Abyss is a high-performance, "vibe coding" focused AI coding engine that runs entirely on local models (DeepSeek, Qwen).

## 🚀 What Does This Engine Do?

AE Abyss provides developers with an intelligent coding assistant that runs completely on local hardware, requiring no internet connection.

- **GGUF Support:** Run models like DeepSeek and Qwen in quantized (GGUF) format.
- **Local & Secure:** Your code never leaves your device; privacy is the top priority.
- **Vibe Coding:** Features an interface designed to accelerate the coding process with automatic file operations and planning capabilities.
- **Low Latency:** High-speed performance on NVIDIA GPUs with native CUDA support.

## 🛠️ How to Install

To run the project, you must have Rust and the CUDA Toolkit (if using an NVIDIA GPU) installed on your system.

1.  **Clone the Repository:**
    ```bash
    git clone https://github.com/username/ae-abyss.git
    cd ae-abyss
    ```

2.  **Prepare the Models:**
    Place your models in the `models/` directory. Check [models/README.md](models/README.md) for details.

3.  **Run:**
    ```bash
    cargo run --release
    ```

## 🔑 Is an API Key Required?

**No.** AE Abyss is designed to work entirely with local models. You do not need any API keys from OpenAI, Anthropic, or Google. Simply download the model files and place them in the appropriate folder.

## 📦 Dependencies

The project primarily uses the following libraries:
- **Candle:** A lightweight machine learning framework by HuggingFace.
- **egui:** A fast and portable user interface.
- **Tokio:** Asynchronous runtime for Rust.

---

