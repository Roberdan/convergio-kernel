---
version: "1.0"
last_updated: "2026-04-07"
author: "convergio-team"
tags: ["adr"]
---

# ADR-015: MLX Direct Inference for Kernel/Jarvis

**Status**: Accepted
**Date**: 2026-04-05
**Context**: Choosing the local inference backend and model for the Kernel/Jarvis assistant on M1 Pro 32GB.

## Decision

Use **Qwen 2.5 7B Instruct 4-bit** via **MLX direct subprocess** (no Ollama) for the Kernel/Jarvis PM role on M1 Pro.

## Context

Kernel/Jarvis is the chief of staff / PM that:
- Classifies Telegram messages (urgente/richiesta/informativo/spam)
- Decides: respond locally or delegate to Claude/Copilot
- Summarizes project activity
- Orchestrates agent spawning via daemon API

We need: fast classification (<2s), good Italian, reliable instruction following, zero API cost.

## Evaluation (M1 Pro 32GB, MLX 0.31.1)

### Models tested

| Model | classify | delegate | summarize | orchestrate | tok/s |
|-------|----------|----------|-----------|-------------|-------|
| **Qwen 7B Instruct 4bit** | OK | **Good** | **Good** | **Good** | 37 | 4.5GB |
| Qwen 14B Instruct 4bit | OK (verbose) | Good (repeats) | Good | Good | 19 | 9GB |
| Qwen 7B Coder 4bit | Perfect | Loops | Good | Good | 38 | 4.5GB |
| Qwen 7B Coder 8bit | Perfect | Loops | Good | OK | 21 | 8GB |
| Qwen 0.5B Instruct 4bit | Verbose | N/A | N/A | N/A | 222 | 0.3GB |
| Gemma 3 4B 4bit | **Infinite loop** | N/A | N/A | N/A | 48 | 2.5GB |

### TurboQuant (kv_bits) findings

Tested KV cache quantization on both 4-bit and 8-bit models:
- **4-bit model + kv_bits=4**: garbage output (dots and repetition)
- **4-bit model + kv_bits=2**: garbage output ("che che che...")
- **8-bit model + kv_bits=4**: garbage output ("pa pa pa...")
- **Conclusion**: KV cache quantization is incompatible with weight-quantized models. Only works on FP16/BF16.

### Rejected alternatives

- **Qwen 14B Instruct 4bit**: 2x slower (19 vs 38 t/s), 2x RAM (9GB vs 4.5GB), comparable quality. Self-critiques and repeats. Not worth the trade-off. M1 Pro needs RAM for Claude/Copilot agents.
- **Gemma 3 4B**: Classification produces infinite loops. Code quality poor. Rejected.
- **Qwen Coder variants**: Excellent at coding but loops on delegation decisions. Wrong model for PM role.
- **Ollama**: Additional process overhead, no advantage over direct MLX subprocess.
- **TurboQuant**: Does not work on quantized models (4-bit or 8-bit). Produces garbage. Would need FP16 (~14GB for 7B) which wastes memory.

### RAM budget (M1 Pro 32GB)

| Component | RAM |
|-----------|-----|
| macOS + daemon + tools | ~6-8 GB |
| **Qwen 7B Instruct 4bit (Jarvis)** | **~4.5 GB** |
| Claude/Copilot agent | ~2-4 GB |
| **Available headroom** | **~16-20 GB** |

## Consequences

- MLX venv at `~/.convergio/mlx-env/` with `mlx-lm` installed
- Model cached in `~/.cache/huggingface/hub/` (~4GB for 7B 4-bit)
- `CONVERGIO_PYTHON` env points to venv Python
- `CONVERGIO_MLX_MODEL` defaults to `mlx-community/Qwen2.5-7B-Instruct-4bit`
- TurboQuant disabled in backend_mlx.rs (documented as incompatible)
- Daemon keeps model warm in memory via long-running subprocess (future)
