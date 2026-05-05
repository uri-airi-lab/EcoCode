"""
EcoCode + Unsloth Demo (Real Unsloth Execution)
===================================================

This script uses the actual `unsloth` library to load a 4-bit model, 
apply LoRA, and train it, using our `EcoCodeSFTCallback` to monitor the run.

Requirements
------------
    pip install "unsloth[colab-new] @ git+https://github.com/unslothai/unsloth.git"
    pip install trl transformers datasets torch
"""
from __future__ import annotations

import os
import random

# ── Config ────────────────────────────────────────────────────────────
API_KEY     = os.environ.get("API_KEY", "")
RUN_NAME    = "unsloth-sft-demo"
MODEL_NAME  = "unsloth/Llama-3.2-1B-bnb-4bit" # 4-bit pre-quantized tiny LLaMA
MAX_STEPS   = 30
BATCH_SIZE  = 2
SEQ_LEN     = 128
LR          = 2e-4

INSTRUCTIONS = [
    ("Translate to French: Hello!", "Bonjour !"),
    ("What is the capital of France?", "Paris."),
    ("Summarise: The cat sat on the mat.", "A cat on a mat."),
    ("What is 2 + 2?", "4."),
]

def build_dataset():
    from datasets import Dataset
    rows = []
    for _ in range(200):
        inst, resp = random.choice(INSTRUCTIONS)
        rows.append({
            "text": f"### Instruction:\n{inst}\n\n### Response:\n{resp}"
        })
    return Dataset.from_list(rows)

def run_unsloth_demo():
    print("\n🦥  EcoCode + Real Unsloth SFT Demo")
    print("=" * 60)

    from unsloth import FastLanguageModel
    from trl import SFTTrainer, SFTConfig
    import ecocode
    from ecocode.integrations.trl import EcoCodeSFTCallback

    print(f"📦 Loading {MODEL_NAME} with Unsloth...")
    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=MODEL_NAME,
        max_seq_length=SEQ_LEN,
        load_in_4bit=True,
    )

    print("🧠 Applying LoRA adapters...")
    model = FastLanguageModel.get_peft_model(
        model,
        r=16,
        target_modules=["q_proj", "k_proj", "v_proj", "o_proj"],
        lora_alpha=16,
        lora_dropout=0,
        bias="none",
        use_gradient_checkpointing="unsloth",
        random_state=3407,
    )

    dataset = build_dataset()

    sft_args = SFTConfig(
        output_dir="./unsloth_output",
        max_steps=MAX_STEPS,
        per_device_train_batch_size=BATCH_SIZE,
        learning_rate=LR,
        warmup_steps=5,
        logging_steps=5,
        dataset_text_field="text",
        max_length=SEQ_LEN,
        report_to="none",
    )

    print("\n🚀 Starting EcoCode-tracked Unsloth SFT training…")
    with ecocode.run(RUN_NAME, api_key=API_KEY, interval=1.0) as eco_run:
        trainer = SFTTrainer(
            model=model,
            args=sft_args,
            train_dataset=dataset,
            callbacks=[EcoCodeSFTCallback(eco_run)],
        )
        train_result = trainer.train()

    print("\n✅ Training complete!")
    print(f"\n   View energy metrics → http://localhost:3001")

if __name__ == "__main__":
    run_unsloth_demo()
