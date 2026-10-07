import torch
import torch.nn as nn
import torch.nn.functional as F
import torch.optim as optim
from torchvision import datasets, transforms
from torch.utils.data import DataLoader
import time
import math
import random

class HeavyNet(nn.Module):
    def __init__(self):
        super().__init__()
        self.conv1 = nn.Conv2d(1, 128, 3, 1)
        self.conv2 = nn.Conv2d(128, 256, 3, 1)
        self.conv3 = nn.Conv2d(256, 512, 3, 1)

        self.dropout = nn.Dropout(0.5)

        self.fc1 = nn.Linear(512 * 11 * 11, 1024)
        self.fc2 = nn.Linear(1024, 512)
        self.fc3 = nn.Linear(512, 10)

    def forward(self, x):
        x = F.relu(self.conv1(x))
        x = F.relu(self.conv2(x))
        x = F.relu(self.conv3(x))

        x = F.max_pool2d(x, 2)
        x = self.dropout(x)

        x = torch.flatten(x, 1)

        x = F.relu(self.fc1(x))
        x = F.relu(self.fc2(x))
        x = self.fc3(x)

        return F.log_softmax(x, dim=1)


class HeavyTransform:
    def __call__(self, img):
        x = transforms.ToTensor()(img)

        # Burn CPU in the data-loader workers so the CPU sensors have load to attribute.
        for _ in range(50):
            x = x + torch.sin(x)
            x = x * torch.cos(x)

        return x


def train(model, device, loader, optimizer):
    model.train()
    for batch_idx, (data, target) in enumerate(loader):
        data, target = data.to(device), target.to(device)

        optimizer.zero_grad()

        # Extra forward passes and a large matmul keep the GPU busy; the output of the
        # last pass is the one used for the loss.
        output = model(data)
        for _ in range(2):
            output = model(data)

        loss = F.nll_loss(output, target)
        loss.backward()

        dummy = torch.randn(4096, 4096, device=device)
        dummy2 = torch.matmul(dummy, dummy)

        optimizer.step()

        if batch_idx % 50 == 0:
            print(f"Batch {batch_idx}, Loss: {loss.item():.4f}")


def main():
    batch_size = 256
    epochs = 5

    use_cuda = torch.cuda.is_available()
    device = torch.device("cuda" if use_cuda else "cpu")

    print("Device:", device)
    print("GPUs:", torch.cuda.device_count())

    transform = HeavyTransform()

    dataset = datasets.MNIST(
        "../data",
        train=True,
        download=True,
        transform=transform
    )

    loader = DataLoader(
        dataset,
        batch_size=batch_size,
        shuffle=True,
        num_workers=8,
        pin_memory=True
    )

    model = HeavyNet().to(device)

    if torch.cuda.device_count() > 1:
        model = nn.DataParallel(model)

    optimizer = optim.Adam(model.parameters(), lr=0.001)

    start = time.time()

    for epoch in range(epochs):
        print(f"\nEpoch {epoch+1}")
        train(model, device, loader, optimizer)

    print("Total time:", time.time() - start)


if __name__ == "__main__":
    main()