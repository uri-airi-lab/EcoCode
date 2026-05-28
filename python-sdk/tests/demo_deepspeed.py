"""
EcoCode DeepSpeed Training Demo (Heavier + Warmup)
"""
import os
import torch
import glob
import time
import deepspeed
from torch import nn
import ecocode
from ecocode.integrations.deepspeed import EcoCodeDeepSpeedCallback

# --- ENVIRONMENT FIX ---
os.environ["MASTER_ADDR"] = "localhost"
os.environ["MASTER_PORT"] = "29501"
os.environ["RANK"] = "0"
os.environ["LOCAL_RANK"] = "0"
os.environ["WORLD_SIZE"] = "1"

# --- LIBRARY FIX ---
venv_base = os.path.dirname(os.path.abspath(__file__))
nvidia_libs = glob.glob(os.path.join(venv_base, "venv/lib/python*/site-packages/nvidia/*/lib"), recursive=True)
if nvidia_libs:
    os.environ["LD_LIBRARY_PATH"] = ":".join(nvidia_libs) + ":" + os.environ.get("LD_LIBRARY_PATH", "")

# HEAVIER MODEL for DeepSpeed
class HeavyModel(nn.Module):
    def __init__(self):
        super().__init__()
        # Multiple large layers to stress the GPU
        self.network = nn.Sequential(
            nn.Linear(1024, 2048),
            nn.ReLU(),
            nn.Linear(2048, 2048),
            nn.ReLU(),
            nn.Linear(2048, 2048),
            nn.ReLU(),
            nn.Linear(2048, 1)
        )
    def forward(self, x):
        return self.network(x)

def run_deepspeed_demo():
    run_name = "deepspeed-heavy-live"
    api_key = "ecocode_SvB6ETEjqwdJVjd6x2QkfdBZqXObta0F"
    
    print(f"\n🚀 Initializing HEAVY DeepSpeed Training...")
    
    model = HeavyModel()
    
    ds_config = {
        "train_batch_size": 32, # Increased batch size for more load
        "optimizer": {
            "type": "Adam",
            "params": {"lr": 1e-4}
        },
        "fp16": {"enabled": True},
        "zero_optimization": {
            "stage": 2
        }
    }

    model_engine, optimizer, _, _ = deepspeed.initialize(
        model=model,
        config=ds_config,
        model_parameters=model.parameters(),
        dist_init_required=True
    )

    with ecocode.run(run_name, api_key=api_key, interval=0.5) as run:
        print(f"\n⏱ DeepSpeed Run started! You have 20 seconds to open the dashboard...")
        print(f"URL: http://localhost:3001")
        time.sleep(20) # WARMUP for browser navigation
        
        ds_cb = EcoCodeDeepSpeedCallback(run, zero_stage=2)
        
        print("\n🛠 Starting DeepSpeed Step Loop (2,000 steps)...")
        for step in range(2000):
            ds_cb.on_step_begin(step)
            
            # --- FORWARD PHASE ---
            ds_cb.on_forward_begin()
            x = torch.randn(32, 1024).to(model_engine.device).half()
            y = torch.randn(32, 1).to(model_engine.device).half()
            outputs = model_engine(x)
            loss = nn.functional.mse_loss(outputs, y)
            ds_cb.on_forward_end()
            
            # --- BACKWARD PHASE ---
            ds_cb.on_backward_begin()
            model_engine.backward(loss)
            ds_cb.on_backward_end()
            
            # --- OPTIMIZER PHASE ---
            model_engine.step()
            ds_cb.on_optimizer_step(step, loss.item())
            
            if step % 200 == 0:
                print(f"Step {step} | Loss: {loss.item():.4f}")

        ds_cb.on_train_end()
        
        print("\n⏳ Cooling down...")
        time.sleep(10)
        
        print("\n✅ DeepSpeed Run Complete!")

if __name__ == "__main__":
    run_deepspeed_demo()
