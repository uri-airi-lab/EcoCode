"""
Heavy HuggingFace Integration Test
- Model:  GPT-2 (124M params)
- Task:   causal LM fine-tuning on synthetic instruction data
- Device: CUDA
- Callback: EcoCodeCallback (phases, loss, lr, step, tokens)
"""
import os, torch
os.environ["TOKENIZERS_PARALLELISM"] = "false"

# Auto-configure CUDA_HOME with local mock if developer CUDA Toolkit is missing
if "CUDA_HOME" not in os.environ:
    possible_mock = os.path.abspath(os.path.join(os.path.dirname(__file__), "venv", "cuda_mock"))
    if os.path.exists(possible_mock):
        os.environ["CUDA_HOME"] = possible_mock


from transformers import (
    AutoTokenizer, AutoModelForCausalLM,
    TrainingArguments, DataCollatorForLanguageModeling,
    Trainer,
)
from datasets import Dataset
import ecocode
from ecocode.integrations.huggingface import EcoCodeCallback

device = "cuda" if torch.cuda.is_available() else "cpu"
print(f"[HF heavy] device={device}  GPU={torch.cuda.get_device_name(0) if device=='cuda' else 'none'}")

# --- Load GPT-2 (124M params) ---
MODEL = "gpt2"
print(f"[HF heavy] Loading {MODEL}...")
tokenizer = AutoTokenizer.from_pretrained(MODEL)
tokenizer.pad_token = tokenizer.eos_token
model = AutoModelForCausalLM.from_pretrained(MODEL, torch_dtype=torch.bfloat16)
model.to(device)
params = sum(p.numel() for p in model.parameters()) / 1e6
print(f"[HF heavy] Model: {MODEL} ({params:.1f}M params)  VRAM: {torch.cuda.memory_allocated()/1e9:.2f}GB")

# --- Small synthetic instruction dataset (40 samples, seq_len=128) ---
TOPICS = [
    "the theory of relativity",
    "attention mechanisms in transformers",
    "backpropagation through time",
    "reinforcement learning from human feedback",
    "mixture of experts architecture",
    "rotary positional embeddings",
    "grouped query attention",
    "speculative decoding for LLM inference",
    "constitutional AI alignment",
    "retrieval-augmented generation",
    "tokenizer vocabulary design",
    "low-rank adaptation fine-tuning",
    "knowledge distillation",
    "contrastive learning objectives",
    "diffusion model score matching",
    "neural architecture search",
    "gradient checkpointing",
    "pipeline parallelism",
    "tensor parallelism",
    "flash attention",
]

INSTRUCTIONS = [
    f"Explain {topic} in depth, covering the mathematical formulation, intuition, key advantages, "
    f"practical limitations, and recent research directions. Include concrete examples where helpful."
    for topic in TOPICS
] * 10  # 200 samples

def tokenize(examples):
    return tokenizer(examples["text"], truncation=True, max_length=128, padding="max_length")

raw_ds = Dataset.from_dict({"text": INSTRUCTIONS})
ds = raw_ds.map(tokenize, batched=True, remove_columns=["text"])
ds = ds.with_format("torch")

collator = DataCollatorForLanguageModeling(tokenizer=tokenizer, mlm=False)

log_path = "test_heavy_hf.log"
if os.path.exists(log_path):
    os.remove(log_path)

print(f"\n[HF heavy] Starting training run ({len(ds)} samples, seq_len=128)...")
with ecocode.run("hf-gpt2-finetune", exporter="online", interval=1.0) as eco_run:
    training_args = TrainingArguments(
        output_dir="/tmp/hf_heavy_xl",
        num_train_epochs=10,
        per_device_train_batch_size=2,
        gradient_accumulation_steps=2,
        learning_rate=2e-5,
        warmup_steps=20,
        lr_scheduler_type="cosine",
        logging_steps=10,
        save_strategy="no",
        report_to="none",
        bf16=(device == "cuda"),
        fp16=False,
        dataloader_num_workers=4,
        push_to_hub=False,
        gradient_checkpointing=True,
    )
    trainer = Trainer(
        model=model,
        args=training_args,
        train_dataset=ds,
        data_collator=collator,
        callbacks=[EcoCodeCallback(eco_run)],
    )
    result = trainer.train()
    print(f"\n[HF heavy] PASS  run_id={eco_run._run_id}")
    print(f"  steps={trainer.state.global_step}  loss={result.training_loss:.4f}")
    if device == "cuda":
        print(f"  VRAM peak: {torch.cuda.max_memory_allocated()/1e9:.2f}GB")

if os.path.exists(log_path):
    os.remove(log_path)
