# Micro Train — MLX Fine-Tuning Research

**Target model:** qwen2.5-coder:0.5b (Micro Olympics champion, 91% accuracy)  
**Platform:** Apple Silicon (mlx-lm)  
**Training data:** `~/.kova/micro/training/dpo_chatml.jsonl`, `sft_chatml.jsonl`

---

## 1. MLX-LM LoRA Fine-Tuning

**Tool:** `mlx_lm.lora` (from `pip install "mlx-lm[train]"`)

**Basic command:**
```bash
mlx_lm.lora \
    --model <path_or_hf_repo> \
    --train \
    --data <path_to_train.jsonl> \
    --iters 600
```

**Key options:**
| Option | Description |
|--------|--------------|
| `--model` | Hugging Face repo or local path (e.g. `mlx-community/Qwen2.5-Coder-0.5B-Instruct-4bit`) |
| `--data` | Path to dir containing `train.jsonl` (optional: `valid.jsonl`, `test.jsonl`) |
| `--iters` | Training iterations |
| `--adapter-path` | Output for LoRA adapters (default: `adapters/`) |
| `--fine-tune-type` | `lora` (default), `dora`, or `full` |
| `--mask-prompt` | Compute loss only on completions |
| `--batch-size` | Batch size |
| `--learning-rate` | e.g. `1e-5` |

**Data format:** JSONL with `messages` array: `[{"role":"system","content":"..."},{"role":"user","content":"..."},{"role":"assistant","content":"..."}]`

**Kova export:** `sft_chatml.jsonl` and `dpo_chatml.jsonl` already use this format. For SFT, use `sft_chatml.jsonl` → copy/symlink to `train.jsonl`.

**Qwen2.5-Coder on MLX:** Use `mlx-community/Qwen2.5-Coder-0.5B-Instruct-4bit` (pre-converted for Apple Silicon) or `Qwen/Qwen2.5-Coder-0.5B-Instruct` (mlx_lm will convert).

---

## 2. LoRA → GGUF → Ollama

**Problem:** mlx_lm produces LoRA adapters (safetensors). Ollama expects GGUF.

**Paths:**

1. **mlx_lm.fuse** — Merge LoRA into base model, save full weights. Then convert to GGUF.
   ```bash
   mlx_lm.fuse --model <base> --adapter-path <adapters> -o fused/
   ```

2. **llama.cpp export-lora** — Merge base GGUF + LoRA → merged GGUF:
   ```bash
   ./export-lora -m model.gguf -l lora.safetensors -o merged.gguf
   ```

3. **GGUF-my-LoRA** (Hugging Face Space) — Upload PEFT LoRA + base, get GGUF.

4. **ollama create** — Use Modelfile with `FROM` pointing to merged GGUF or base + adapter.

**Recommended flow for kova:**
1. Train with `mlx_lm.lora` → adapters in `~/.kova/micro/adapters/`
2. Fuse: `mlx_lm.fuse --model mlx-community/Qwen2.5-Coder-0.5B-Instruct-4bit --adapter-path ~/.kova/micro/adapters/ -o ~/.kova/micro/fused/`
3. Convert fused MLX → GGUF (llama.cpp convert script or similar)
4. `ollama create kova-qwen0.5b -f Modelfile` with `FROM ./merged.gguf`

**Unsloth:** Alternative to mlx_lm; supports Apple Silicon but less native. mlx_lm is the Apple-first choice.

---

## 3. kova micro train — Draft CLI

**Command:** `kova micro train [OPTIONS]`

**Behavior:**
1. Ensure `~/.kova/micro/training/sft_chatml.jsonl` exists (or `dpo_chatml.jsonl` for DPO)
2. Prepare `train.jsonl` (copy or symlink; mlx_lm expects that name)
3. Run `mlx_lm.lora` with:
   - `--model mlx-community/Qwen2.5-Coder-0.5B-Instruct-4bit`
   - `--data ~/.kova/micro/training/`
   - `--adapter-path ~/.kova/micro/adapters/`
   - `--iters` from config or default 600
   - `--mask-prompt`

**Options:**
- `--format sft|dpo` — which ChatML file to use (default: sft)
- `--iters N` — training iterations
- `--dry-run` — print command, don't run

**Prereqs:** `pip install "mlx-lm[train]"`, tournament run + export first.

---

## 4. Config (optional)

Add to `~/.kova/config.toml`:

```toml
[micro.train]
model = "mlx-community/Qwen2.5-Coder-0.5B-Instruct-4bit"
iters = 600
batch_size = 4
learning_rate = 1e-5
```

---

## References

- [mlx-lm LORA.md](https://github.com/ml-explore/mlx-lm/blob/main/mlx_lm/LORA.md)
- [GGUF-my-LoRA](https://huggingface.co/spaces/ggml-org/gguf-my-lora)
- [llama.cpp export-lora](https://github.com/ggml-org/llama.cpp)
