"""
Test: EcoCode SDK — Attached Mode
Trains a Transformer language model (real PyTorch training, CPU-heavy).
The CLI wraps this script, creates the run, and handles all energy sampling.
The SDK detects ECOCODE_MARKER_FD + ECOCODE_RUN_ID and sends markers via pipe.

Usage (run from repo root):
    ecocode run -o online -p "sdk-test" -- python python-sdk/test_attached.py

    Or with terminal output:
    ecocode run -p "sdk-test" -- python python-sdk/test_attached.py
"""

import math
import time

import torch
import torch.nn as nn
import torch.optim as optim
from torch.utils.data import DataLoader, TensorDataset

import ecocode


class TransformerLM(nn.Module):
    """Small Transformer language model (~5M params) for stress testing."""

    def __init__(self, vocab_size=5000, d_model=256, nhead=4, num_layers=4, max_len=128):
        super().__init__()
        self.d_model = d_model
        self.embedding = nn.Embedding(vocab_size, d_model)
        self.pos_encoding = nn.Embedding(max_len, d_model)
        encoder_layer = nn.TransformerEncoderLayer(
            d_model=d_model, nhead=nhead, dim_feedforward=512,
            dropout=0.1, batch_first=True,
        )
        self.transformer = nn.TransformerEncoder(encoder_layer, num_layers=num_layers)
        self.fc_out = nn.Linear(d_model, vocab_size)

    def forward(self, x):
        seq_len = x.size(1)
        positions = torch.arange(seq_len, device=x.device).unsqueeze(0)
        x = self.embedding(x) * math.sqrt(self.d_model) + self.pos_encoding(positions)

        # Causal mask for autoregressive training
        mask = nn.Transformer.generate_square_subsequent_mask(seq_len, device=x.device)
        x = self.transformer(x, mask=mask)
        return self.fc_out(x)


def generate_fake_text(n_samples=2000, seq_len=64, vocab_size=5000):
    """Random token sequences simulating a text corpus."""
    data = torch.randint(0, vocab_size, (n_samples, seq_len + 1))
    inputs = data[:, :-1]    # [n, seq_len]
    targets = data[:, 1:]    # [n, seq_len] — shifted by one
    return TensorDataset(inputs, targets)


def main():
    print("[test] Attached mode test — Transformer LM training")
    print(f"[test] PyTorch: {torch.__version__}")
    print()

    # --- Model ---
    VOCAB = 5000
    SEQ_LEN = 64
    model = TransformerLM(vocab_size=VOCAB, d_model=256, nhead=4, num_layers=4, max_len=SEQ_LEN)
    param_count = sum(p.numel() for p in model.parameters())
    print(f"[test] Model: TransformerLM ({param_count:,} params)")

    criterion = nn.CrossEntropyLoss()
    optimizer = optim.AdamW(model.parameters(), lr=3e-4, weight_decay=0.01)
    scheduler = optim.lr_scheduler.CosineAnnealingLR(optimizer, T_max=100)

    # --- Data ---
    train_dataset = generate_fake_text(n_samples=2000, seq_len=SEQ_LEN, vocab_size=VOCAB)
    train_loader = DataLoader(train_dataset, batch_size=32, shuffle=True, num_workers=2)

    EPOCHS = 3
    global_step = 0
    total_tokens = 0

    # =============================================
    # ATTACHED MODE: CLI already created the run
    # SDK detects ECOCODE_MARKER_FD + ECOCODE_RUN_ID
    # =============================================
    print("\n" + "=" * 60)
    print("  ATTACHED MODE TEST")
    print("=" * 60 + "\n")

    with ecocode.run() as run:  # no api_key needed — CLI manages the run

        print(f"[ecocode] Mode:   {run._mode}")
        print(f"[ecocode] Run ID: {run._run_id}")
        print()

        if run._mode != "attached":
            print("[WARN] Expected attached mode but got:", run._mode)
            print("[WARN] Make sure you run this via: ecocode run -- python test_attached.py")

        run.mark("phase", "data_loading")
        time.sleep(0.3)

        for epoch in range(EPOCHS):
            run.log_epoch(epoch)
            model.train()
            epoch_loss = 0.0

            for batch_idx, (inputs, targets) in enumerate(train_loader):
                run.mark("phase", "forward")
                logits = model(inputs)  # [B, seq_len, vocab]

                # Flatten for cross-entropy
                loss = criterion(
                    logits.reshape(-1, VOCAB),
                    targets.reshape(-1),
                )

                run.mark("phase", "backward")
                optimizer.zero_grad()
                loss.backward()

                # Gradient clipping (common in LM training)
                torch.nn.utils.clip_grad_norm_(model.parameters(), max_norm=1.0)

                run.mark("phase", "optimizer_step")
                optimizer.step()
                scheduler.step()

                global_step += 1
                batch_tokens = inputs.numel()
                total_tokens += batch_tokens

                run.log_step(global_step)
                run.log_loss(loss.item())
                run.log_lr(scheduler.get_last_lr()[0])
                run.log_tokens(total_tokens)
                run.log_sequence_length(SEQ_LEN)

                # Perplexity as custom metric
                ppl = math.exp(min(loss.item(), 20))  # cap to avoid overflow
                run.log_custom("perplexity", ppl)

                epoch_loss += loss.item()

                if batch_idx % 15 == 0:
                    print(
                        f"  Epoch {epoch+1}/{EPOCHS} "
                        f"[{batch_idx:>3d}/{len(train_loader)}] "
                        f"loss={loss.item():.4f} "
                        f"ppl={ppl:.1f} "
                        f"lr={scheduler.get_last_lr()[0]:.6f} "
                        f"step={global_step}"
                    )

            avg_loss = epoch_loss / len(train_loader)
            print(
                f"\n  Epoch {epoch+1} done — "
                f"avg_loss={avg_loss:.4f} "
                f"ppl={math.exp(min(avg_loss, 20)):.1f}\n"
            )

        run.mark("phase", "done")
        print(f"[ecocode] Training complete. {global_step} steps, {total_tokens:,} tokens")

    print("\n[test] Attached mode test PASSED — run completed cleanly.")


if __name__ == "__main__":
    main()
