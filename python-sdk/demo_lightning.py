"""
EcoCode PyTorch Lightning Demo (Fixed + Heavy + Warmup)
"""
import os
import glob
import time
import torch
from torch import nn
from torch.utils.data import DataLoader, Dataset
import pytorch_lightning as pl
import ecocode
from ecocode.integrations.lightning import EcoCodeCallback

# --- ENVIRONMENT FIX (Same as DeepSpeed) ---
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

# HEAVIER MODEL
class HeavyLightningModel(pl.LightningModule):
    def __init__(self):
        super().__init__()
        self.network = nn.Sequential(
            nn.Linear(1024, 2048),
            nn.ReLU(),
            nn.Linear(2048, 2048),
            nn.ReLU(),
            nn.Linear(2048, 1)
        )
    def forward(self, x):
        return self.network(x)
    def training_step(self, batch, batch_idx):
        x, y = batch
        loss = nn.functional.mse_loss(self(x), y)
        return loss
    def configure_optimizers(self):
        return torch.optim.Adam(self.parameters(), lr=1e-4)

# Dummy data
class HeavyDataset(Dataset):
    def __len__(self): return 5000
    def __getitem__(self, idx):
        return torch.randn(1024), torch.randn(1)

def run_lightning_demo():
    run_name = "lightning-heavy-live"
    api_key = "ecocode_SvB6ETEjqwdJVjd6x2QkfdBZqXObta0F"
    
    print(f"\n🚀 Initializing PyTorch Lightning Training...")
    
    model = HeavyLightningModel()
    train_loader = DataLoader(HeavyDataset(), batch_size=32)

    with ecocode.run(run_name, api_key=api_key, interval=0.1) as run:
        print(f"\n⏱ Lightning Run started! You have 20 seconds to open the dashboard...")
        print(f"URL: http://localhost:3001")
        time.sleep(20) # WARMUP
        
        print("\n🛠 Starting Lightning Trainer (2,000 steps)...")
        from pytorch_lightning.plugins.environments import LightningEnvironment
        
        trainer = pl.Trainer(
            max_steps=2000,
            callbacks=[EcoCodeCallback(run)],
            accelerator="gpu",
            devices=1,
            plugins=[LightningEnvironment()], # Bypass MPI detection
            logger=False,
            enable_checkpointing=False,
            precision="16-mixed",
        )
        
        trainer.fit(model, train_loader)
        
        print("\n⏳ Cooling down...")
        time.sleep(10)
        print("\n✅ Lightning Run Complete!")

if __name__ == "__main__":
    run_lightning_demo()
