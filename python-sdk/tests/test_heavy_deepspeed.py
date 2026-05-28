"""
Heavy DeepSpeed Integration Test
- Model:  GPT-2 (124M params) wrapped with actual deepspeed.initialize()
- ZeRO:   Stage 3 (full parameter + gradient + optimizer sharding)
- Device: CUDA
- Callback: EcoCodeDeepSpeedCallback with real DS engine hooks
"""
import os, json, torch, glob
os.environ.setdefault("ECOCODE_API_URL", "http://localhost:3001")
os.environ["TOKENIZERS_PARALLELISM"] = "false"
os.environ.setdefault("PYTORCH_CUDA_ALLOC_CONF", "expandable_segments:True")

# --- ENVIRONMENT FIX ---
os.environ["MASTER_ADDR"] = "localhost"
os.environ["MASTER_PORT"] = "29502"
os.environ["RANK"] = "0"
os.environ["LOCAL_RANK"] = "0"
os.environ["WORLD_SIZE"] = "1"

# --- LIBRARY FIX ---
venv_base = os.path.dirname(os.path.abspath(__file__))
nvidia_libs = glob.glob(os.path.join(venv_base, "venv/lib/python*/site-packages/nvidia/*/lib"), recursive=True)
if nvidia_libs:
    os.environ["LD_LIBRARY_PATH"] = ":".join(nvidia_libs) + ":" + os.environ.get("LD_LIBRARY_PATH", "")

import deepspeed
from transformers import AutoTokenizer, AutoModelForCausalLM
from torch.utils.data import DataLoader, TensorDataset
import ecocode
from ecocode.integrations.deepspeed import EcoCodeDeepSpeedCallback

device = "cuda" if torch.cuda.is_available() else "cpu"
print(f"[DeepSpeed heavy] device={device}  deepspeed={deepspeed.__version__}")

MODEL = "gpt2"
print(f"[DeepSpeed heavy] Loading {MODEL}...")
tokenizer = AutoTokenizer.from_pretrained(MODEL)
tokenizer.pad_token = tokenizer.eos_token
model = AutoModelForCausalLM.from_pretrained(MODEL)
params = sum(p.numel() for p in model.parameters()) / 1e6
print(f"[DeepSpeed heavy] Model: {params:.1f}M params")

# DeepSpeed ZeRO stage 3 config
ds_config = {
    "train_micro_batch_size_per_gpu": 4,
    "gradient_accumulation_steps": 8,
    "optimizer": {
        "type": "AdamW",
        "params": {
            "lr": 1e-5,
            "weight_decay": 0.01,
            "torch_adam": True,
        }
    },
    "scheduler": {
        "type": "WarmupDecayLR",
        "params": {
            "warmup_min_lr": 0,
            "warmup_max_lr": 1e-5,
            "warmup_num_steps": 50,
            "total_num_steps": 3000,
        }
    },
    "zero_optimization": {
        "stage": 3,
        "allgather_partitions": True,
        "allgather_bucket_size": 2e8,
        "reduce_scatter": True,
        "reduce_bucket_size": 2e8,
        "overlap_comm": True,
        "contiguous_gradients": True,
    },
    "fp16": {
        "enabled": device == "cuda",
        "loss_scale": 0,
        "loss_scale_window": 100,
        "hysteresis": 2,
        "min_loss_scale": 1,
    },
    "steps_per_print": 20,
    "wall_clock_breakdown": False,
}

# Synthetic token dataset (2048 samples, seq=128)
VOCAB = tokenizer.vocab_size
SEQ   = 128
N     = 2048
input_ids = torch.randint(0, VOCAB, (N, SEQ))
labels    = input_ids.clone()
dataset   = TensorDataset(input_ids, labels)
loader    = DataLoader(
    dataset,
    batch_size=ds_config["train_micro_batch_size_per_gpu"],
    shuffle=True,
    num_workers=4,
)

# Initialize DeepSpeed engine
engine, optimizer, _, lr_scheduler = deepspeed.initialize(
    model=model,
    config=ds_config,
    model_parameters=model.parameters(),
)
print(f"[DeepSpeed heavy] Engine initialized with ZeRO stage {ds_config['zero_optimization']['stage']}")

api_key  = os.environ.get("ECOCODE_API_KEY", "ecocode_SvB6ETEjqwdJVjd6x2QkfdBZqXObta0F")
base_url = os.environ.get("ECOCODE_API_URL", "http://localhost:3001")

print(f"\n[DeepSpeed heavy] Starting ZeRO-3 training ({N} samples, seq={SEQ})...")
with ecocode.run("deepspeed-gpt2xl-zero3", api_key=api_key, base_url=base_url, interval=1.0) as eco_run:
    cb = EcoCodeDeepSpeedCallback(eco_run, zero_stage=3)
    cb.on_data_loading_begin()

    global_step = 0
    for epoch in range(5):
        epoch_loss = 0.0
        for step, (ids, lbls) in enumerate(loader):
            cb.on_step_begin(global_step)
            cb.on_forward_begin()

            ids  = ids.to(engine.device)
            lbls = lbls.to(engine.device)

            out  = engine(input_ids=ids, labels=lbls)
            loss = out.loss

            cb.on_backward_begin()
            engine.backward(loss)
            cb.on_backward_end()

            engine.step()
            cb.on_optimizer_step(global_step, loss.item())

            epoch_loss  += loss.item()
            global_step += 1

        avg = epoch_loss / len(loader)
        print(f"  Epoch {epoch+1}/5  avg_loss={avg:.4f}  steps={global_step}")

    cb.on_train_end()
    print(f"\n[DeepSpeed heavy] PASS  run_id={eco_run._run_id}  total_steps={global_step}  final_loss={avg:.4f}")
    if device == "cuda":
        print(f"  VRAM peak: {torch.cuda.max_memory_allocated()/1e9:.2f}GB")
