import os
import time
import torch
import torch.nn as nn
import torch.optim as optim
import torchvision.models as models
import ecocode

json_path = "test_decorator.json"
if os.path.exists(json_path):
    os.remove(json_path)

@ecocode.track(
    name="decorator-pytorch-cuda-resnet50-heavy",
    exporter="online",
    interval=0.2
)
def train_model():
    device = "cuda" if torch.cuda.is_available() else "cpu"
    print(f"\n🚀 Starting ResNet-50 training on {device}...")
    if device != "cuda":
        print("⚠️ Warning: CUDA is not available. Running on CPU.")
    
    # Load ResNet-50 (approx 25.6 million parameters)
    print("  [setup] Instantiating ResNet-50 model...")
    model = models.resnet50()
    model.to(device)
    
    criterion = nn.CrossEntropyLoss()
    optimizer = optim.SGD(model.parameters(), lr=0.01, momentum=0.9, weight_decay=1e-4)
    
    # Define batch dimensions (reduced batch size to avoid CUDA OOM)
    batch_size = 32
    channels = 3
    height, width = 224, 224
    steps = 100
    
    print(f"  [setup] Workload: {steps} steps | batch_size={batch_size} | image={channels}x{height}x{width}")
    
    # Target label classes (1000 categories for standard ResNet ImageNet task)
    targets = torch.randint(0, 1000, (batch_size,), device=device)
    
    # Run training steps
    for step in range(steps):
        # New random batch each step to simulate data loading I/O
        inputs = torch.randn(batch_size, channels, height, width, device=device)
        
        optimizer.zero_grad()
        outputs = model(inputs)
        loss = criterion(outputs, targets)
        loss.backward()
        optimizer.step()
        
        # Short sleep to space out runs and capture multiple monitor intervals
        time.sleep(0.05)
        
        if step % 10 == 0 or step == steps - 1:
            vram_allocated = torch.cuda.memory_allocated(device) / 1e6 if device == "cuda" else 0
            vram_max = torch.cuda.max_memory_allocated(device) / 1e6 if device == "cuda" else 0
            print(
                f"  [step {step:03d}/{steps}] loss={loss.item():.4f} | "
                f"VRAM (Allocated/Peak): {vram_allocated:.1f}MB / {vram_max:.1f}MB"
            )
    
    print("\n✅ ResNet-50 heavy training completed successfully.")
    return True

if __name__ == "__main__":

    train_model()
