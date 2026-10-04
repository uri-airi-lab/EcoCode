"""
EcoCode TRL SFT Training Demo
"""
import os
import torch
from datasets import Dataset
from transformers import AutoModelForCausalLM, AutoTokenizer, TrainingArguments
from trl import SFTTrainer, SFTConfig
import ecocode
from ecocode.integrations.trl import EcoCodeSFTCallback

def run_trl_demo():
    model_id = "Qwen/Qwen2.5-0.5B-Instruct"
    run_name = "trl-sft-qwen-0.5b"
    api_key = os.environ.get("ECOCODE_API_KEY")
    
    print(f"\n🚀 Initializing TRL SFT Training with {model_id}...")
    
    tokenizer = AutoTokenizer.from_pretrained(model_id)
    tokenizer.pad_token = tokenizer.eos_token
    model = AutoModelForCausalLM.from_pretrained(model_id, device_map="cpu")
    
    # Tiny dataset
    data = {"text": ["Question: What is EcoCode? Answer: A tool for green AI.", "Question: Why sustainable? Answer: To save energy."] * 5}
    dataset = Dataset.from_dict(data)

    training_args = SFTConfig(
        output_dir="./trl_results",
        max_steps=10,
        logging_steps=1,
        report_to="none",
        dataset_text_field="text",
        max_length=128,
        use_cpu=True,
    )

    with ecocode.run(run_name, exporter="online", api_key=api_key) as run:
        print("\n🛠 Starting SFT Trainer...")
        trainer = SFTTrainer(
            model=model,
            args=training_args,
            train_dataset=dataset,
            callbacks=[EcoCodeSFTCallback(run)],
        )
        
        trainer.train()
        print("\n✅ SFT Complete!")
        print(f"View metrics at: http://localhost:3001")

if __name__ == "__main__":
    run_trl_demo()
