"""
EcoCode + Axolotl Demo (Real Axolotl Execution)
===================================================

This script uses the actual `axolotl` library. It creates a tiny
configuration file programmatically, and launches an Axolotl training
job with the `EcoCodeCallback` injected.

Requirements
------------
    pip install axolotl ecocode
"""
from __future__ import annotations

import os
import yaml

# ── Config ────────────────────────────────────────────────────────────
API_KEY     = os.environ.get("ECOCODE_API_KEY", "")
RUN_NAME    = "axolotl-sft-demo"

YAML_PATH = "axolotl_demo_config.yml"

def run_axolotl_demo():
    print("\n🦎  EcoCode + Real Axolotl SFT Demo")
    print("=" * 60)

    from axolotl.cli.train import do_train
    import ecocode
    from ecocode.integrations.huggingface import EcoCodeCallback

    # 1. Create a tiny Axolotl YAML configuration for a fast test
    config_dict = {
        "base_model": "HuggingFaceM4/tiny-random-LlamaForCausalLM",
        "model_type": "AutoModelForCausalLM",
        "tokenizer_type": "AutoTokenizer",
        "load_in_8bit": False,
        "sequence_len": 128,
        "sample_packing": False,
        "adapter": "lora",
        "lora_r": 8,
        "lora_alpha": 16,
        "lora_dropout": 0.05,
        "lora_target_modules": ["q_proj", "v_proj"],
        "datasets": [
            {
                "path": "mhenrichsen/alpaca_2k_test",
                "type": "alpaca"
            }
        ],
        "dataset_prepared_path": "last_run_prepared",
        "val_set_size": 0,
        "output_dir": "./axolotl_outputs",
        "micro_batch_size": 2,
        "gradient_accumulation_steps": 1,
        "num_epochs": 1,
        "max_steps": 20,
        "learning_rate": 0.0002,
        "optimizer": "adamw_torch",
        "lr_scheduler": "cosine",
        "warmup_steps": 5,
        "logging_steps": 5,
        "bf16": False,
        "fp16": False,
        "tf32": False,
        "gradient_checkpointing": False,
    }

    print(f"📝 Writing Axolotl config to {YAML_PATH}...")
    with open(YAML_PATH, "w") as f:
        yaml.dump(config_dict, f)

    # 2. Run Axolotl with EcoCode tracking
    print("\n🚀 Starting EcoCode-tracked Axolotl training…")
    with ecocode.run(RUN_NAME, exporter="online", api_key=API_KEY, interval=1.0) as eco_run:
        cb = EcoCodeCallback(eco_run)
        print("   Injecting EcoCodeCallback into Axolotl's do_train() via monkeypatch...")
        
        import transformers
        original_init = transformers.Trainer.__init__
        def patched_init(self, *args, **kwargs):
            callbacks = kwargs.get("callbacks")
            if callbacks is None:
                callbacks = []
            elif not isinstance(callbacks, list):
                callbacks = list(callbacks)
            callbacks.append(cb)
            kwargs["callbacks"] = callbacks
            original_init(self, *args, **kwargs)
        
        transformers.Trainer.__init__ = patched_init
        
        try:
            from axolotl.cli.train import do_cli
            do_cli(YAML_PATH)
        finally:
            transformers.Trainer.__init__ = original_init

    print("\n✅ Axolotl Training complete!")
    print(f"\n   View energy metrics → http://localhost:3001")

if __name__ == "__main__":
    run_axolotl_demo()
