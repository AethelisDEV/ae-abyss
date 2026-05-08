# AI Models

This directory hosts the local AI models used by the project. It is specifically configured for **DeepSeek** and **Qwen** models.

## Model Setup

To use the models, you need to place the relevant files according to the following folder structure:

### 1. DeepSeek
Copy DeepSeek models (e.g., in GGUF format) to the `models/deepseek/` folder.
- Recommended: `deepseek-coder-6.7b-instruct.Q4_K_M.gguf` or similar.
- **Critical:** You must also place the corresponding `tokenizer.json` file in the same folder.

### 2. Qwen
Copy Qwen models to the `models/qwen/` folder.
- Recommended: `qwen2.5-7b-instruct-q4_k_m.gguf` or similar.
- **Critical:** You must also place the corresponding `tokenizer.json` file in the same folder.

## Important Notes

- **Tokenizer:** The system expects a `tokenizer.json` file in the same directory as the model file. The model cannot be loaded without this file.
- **File Format:** Models are generally expected to be in `.gguf` format for local inference (using libraries like `candle` or `llama.cpp`).
- **HuggingFace:** You can download these models from [HuggingFace](https://huggingface.co/).
- **Naming:** If the code looks for a specific filename, remember to update the file name accordingly or specify the path in the configuration file.
