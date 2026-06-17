"""
EcoCode HuggingFace Training Demo (Heavier Model)
"""
import os
import torch
import glob
from datasets import Dataset
from transformers import AutoModelForCausalLM, AutoTokenizer, TrainingArguments, Trainer
import ecocode
from ecocode.integrations.huggingface import EcoCodeCallback

# --- LIBRARY FIX ---
venv_base = os.path.dirname(os.path.abspath(__file__))
nvidia_libs = glob.glob(os.path.join(venv_base, "venv/lib/python*/site-packages/nvidia/*/lib"), recursive=True)
if nvidia_libs:
    os.environ["LD_LIBRARY_PATH"] = ":".join(nvidia_libs) + ":" + os.environ.get("LD_LIBRARY_PATH", "")

def run_hf_training_demo():
    # Switching back to Qwen 0.5B (Heavier than GPT2)
    model_id = "Qwen/Qwen2.5-0.5B-Instruct"
    run_name = "hf-train-qwen-heavy"
    api_key = os.environ.get("ECOCODE_API_KEY")
    
    print(f"\n🚀 Initializing HuggingFace Training with {model_id}...")
    
    tokenizer = AutoTokenizer.from_pretrained(model_id)
    tokenizer.pad_token = tokenizer.eos_token
    
    # Load model (let Trainer handle precision)
    model = AutoModelForCausalLM.from_pretrained(model_id, device_map="auto")
    
    # Synthetic dataset
    data = {"text": ["EcoCode tracks AI energy usage precisely.", "Sustainable AI development is crucial.", "Green computing reduces carbon footprints."] * 100}
    dataset = Dataset.from_dict(data)
    
    def tokenize_function(examples):
        tokens = tokenizer(examples["text"], padding="max_length", truncation=True, max_length=128)
        tokens["labels"] = tokens["input_ids"].copy()
        return tokens
    tokenized_dataset = dataset.map(tokenize_function, batched=True)

    # Memory-optimized training for 4GB VRAM
    training_args = TrainingArguments(
        output_dir="./hf_results",
        per_device_train_batch_size=1, 
        num_train_epochs=1,
        max_steps=3000,                # 3000 steps = several minutes of compute
        logging_steps=100,
        fp16=True,
        gradient_checkpointing=True,
        optim="adafactor",
        report_to="none",
    )

    with ecocode.run(run_name, exporter="online", api_key=api_key) as run:
        print(f"\n⏱ Run started! You have 20 seconds to open the dashboard...")
        print(f"URL: http://localhost:3001")
        import time
        time.sleep(20) # WARMUP: Time for you to open the browser
        
        print("\n🛠 Starting HEAVY Training Loop (Qwen 0.5B)...")
        trainer = Trainer(
            model=model,
            args=training_args,
            train_dataset=tokenized_dataset,
            callbacks=[EcoCodeCallback(run)],
        )
        
        trainer.train()
        
        print("\n⏳ Cooling down...")
        time.sleep(10)
        
        print("\n✅ Training Complete!")
        print(f"View metrics at: http://localhost:3001")

if __name__ == "__main__":
    run_hf_training_demo()
