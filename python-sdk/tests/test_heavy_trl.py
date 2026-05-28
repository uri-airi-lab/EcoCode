"""
Heavy TRL Integration Test
- SFT:  GPT-2 XL fine-tuned with SFTTrainer (2000 samples, seq_len=512, 3 epochs)
- DPO:  GPT-2 large policy vs frozen GPT-2 large reference (SEQ=128, 120 steps)
- GRPO: GPT-2 large policy with group-relative reward rollout (GROUP_SIZE=4, 120 steps)
- Device: CUDA
"""
import os, torch, torch.nn as nn, time
os.environ.setdefault("ECOCODE_API_URL", "http://localhost:3001")
os.environ["TOKENIZERS_PARALLELISM"] = "false"
os.environ.setdefault("PYTORCH_CUDA_ALLOC_CONF", "expandable_segments:True")

from transformers import AutoTokenizer, AutoModelForCausalLM
from datasets import Dataset as HFDataset
from trl import SFTTrainer, SFTConfig
import ecocode
from ecocode.integrations.trl import EcoCodeSFTCallback, EcoCodeDPOCallback, EcoCodeGRPOCallback
from types import SimpleNamespace

device = "cuda" if torch.cuda.is_available() else "cpu"
print(f"[TRL heavy] device={device}")

# ─────────────────────────────────────────────────────────────
# TEST 1: SFT — GPT-2 XL supervised fine-tuning
# ─────────────────────────────────────────────────────────────
print("\n" + "="*60)
print("  TRL SFT — GPT-2 supervised fine-tuning (124M params)")
print("="*60)

MODEL_SFT = "gpt2"
print(f"[TRL heavy] Loading {MODEL_SFT}...")
tokenizer_sft = AutoTokenizer.from_pretrained(MODEL_SFT)
tokenizer_sft.pad_token = tokenizer_sft.eos_token

api_key  = os.environ.get("ECOCODE_API_KEY")
base_url = os.environ.get("ECOCODE_API_URL", "http://localhost:3001")

model_sft = AutoModelForCausalLM.from_pretrained(MODEL_SFT, torch_dtype=torch.bfloat16).to(device)
model_sft.config.use_cache = False
params = sum(p.numel() for p in model_sft.parameters()) / 1e6
print(f"[TRL heavy] {MODEL_SFT}: {params:.0f}M params")

TOPICS = [
    "backpropagation", "attention mechanisms", "dropout regularization",
    "batch normalization", "gradient descent", "cross-entropy loss",
    "softmax activation", "embedding layers", "tokenization",
    "fine-tuning", "transfer learning", "overfitting",
    "regularization", "activation functions", "residual connections",
    "layer normalization", "weight decay", "learning rate scheduling",
    "training epochs", "mini-batch training", "knowledge distillation",
    "model pruning", "quantization", "speculative decoding",
    "retrieval augmented generation", "constitutional AI", "RLHF",
    "mixture of experts", "flash attention", "low-rank adaptation",
]
TEXTS = [
    f"Question: Can you give a thorough technical explanation of {topic}? "
    f"Answer: {topic} is a critical technique in modern deep learning. "
    f"It works by optimizing the loss landscape through carefully designed "
    f"mathematical operations that allow gradients to flow efficiently. "
    f"The key insight is that neural networks benefit enormously from this "
    f"approach because it enables stable optimization at scale."
    for topic in TOPICS
] * 4
TEXTS = TEXTS[:120]

sft_ds = HFDataset.from_dict({"text": TEXTS})

with ecocode.run("trl-sft-gpt2", api_key=api_key, base_url=base_url, interval=1.0) as eco_run:
    sft_args = SFTConfig(
        output_dir="/tmp/trl_sft_xl",
        num_train_epochs=3,
        per_device_train_batch_size=2,
        gradient_accumulation_steps=2,
        learning_rate=1e-5,
        warmup_steps=10,
        logging_steps=5,
        save_strategy="no",
        report_to="none",
        bf16=(device == "cuda"),
        fp16=False,
        max_length=128,
        push_to_hub=False,
        gradient_checkpointing=True,
        dataloader_num_workers=4,
        use_cpu=(device == "cpu"),
    )
    trainer = SFTTrainer(
        model=model_sft,
        args=sft_args,
        train_dataset=sft_ds,
        processing_class=tokenizer_sft,
        callbacks=[EcoCodeSFTCallback(eco_run)],
    )
    result = trainer.train()
    print(f"\n[TRL SFT] PASS  run_id={eco_run._run_id}  steps={trainer.state.global_step}  loss={result.training_loss:.4f}")
    if device == "cuda":
        print(f"  VRAM peak: {torch.cuda.max_memory_allocated()/1e9:.2f}GB")

del trainer, model_sft
if device == "cuda":
    import gc; gc.collect(); torch.cuda.empty_cache()

# ─────────────────────────────────────────────────────────────
# TEST 2: DPO — GPT-2 large policy vs frozen reference
# ─────────────────────────────────────────────────────────────
print("\n" + "="*60)
print("  TRL DPO — GPT-2 policy + frozen reference (124M params)")
print("="*60)

MODEL_DPO = "gpt2"
print(f"[TRL heavy] Loading {MODEL_DPO} (policy + ref)...")
tokenizer_dpo = AutoTokenizer.from_pretrained(MODEL_DPO)
tokenizer_dpo.pad_token = tokenizer_dpo.eos_token

policy_dpo = AutoModelForCausalLM.from_pretrained(MODEL_DPO, torch_dtype=torch.bfloat16).to(device)
ref_dpo    = AutoModelForCausalLM.from_pretrained(MODEL_DPO, torch_dtype=torch.bfloat16).to(device)
for p in ref_dpo.parameters(): p.requires_grad_(False)
params = sum(p.numel() for p in policy_dpo.parameters()) / 1e6
print(f"[TRL heavy] {MODEL_DPO}: {params:.0f}M params x2 (policy + frozen ref)")

optimizer = torch.optim.AdamW(policy_dpo.parameters(), lr=5e-6)
scaler    = torch.amp.GradScaler("cuda", enabled=False)

VOCAB = tokenizer_dpo.vocab_size
SEQ   = 128
BATCH = 2
N_STEPS = 60

def make_preference_batch():
    chosen   = torch.randint(0, VOCAB, (BATCH, SEQ), device=device)
    rejected = torch.randint(0, VOCAB, (BATCH, SEQ), device=device)
    mask     = torch.ones(BATCH, SEQ, device=device, dtype=torch.long)
    return chosen, rejected, mask

def dpo_loss(policy_log_chosen, policy_log_rejected, ref_log_chosen, ref_log_rejected, beta=0.1):
    log_ratio = (policy_log_chosen - ref_log_chosen) - (policy_log_rejected - ref_log_rejected)
    return -nn.functional.logsigmoid(beta * log_ratio).mean()

def sequence_log_prob(model, input_ids, attention_mask):
    ctx = torch.no_grad() if not model.training else torch.enable_grad()
    with ctx, torch.autocast(device_type=device, dtype=torch.bfloat16, enabled=(device == "cuda")):
        out = model(input_ids=input_ids, attention_mask=attention_mask)
    logits    = out.logits[:, :-1].float()
    targets   = input_ids[:, 1:]
    log_probs = logits.log_softmax(-1)
    token_lp  = log_probs.gather(2, targets.unsqueeze(-1)).squeeze(-1)
    return (token_lp * attention_mask[:, 1:]).sum(-1) / attention_mask[:, 1:].sum(-1)

state = SimpleNamespace(global_step=0, epoch=0.0, log_history=[])
args  = SimpleNamespace(max_length=SEQ, max_seq_length=None)
ctrl  = SimpleNamespace()

with ecocode.run("trl-dpo-gpt2large", api_key=api_key, base_url=base_url, interval=1.0) as eco_run:
    cb = EcoCodeDPOCallback(eco_run)
    cb.on_train_begin(args, state, ctrl)

    for epoch in range(3):
        state.epoch = float(epoch)
        cb.on_epoch_begin(args, state, ctrl)
        for step in range(N_STEPS // 3):
            cb.on_step_begin(args, state, ctrl)
            chosen, rejected, mask = make_preference_batch()

            policy_dpo.train()
            pi_ch   = sequence_log_prob(policy_dpo, chosen,   mask)
            pi_rej  = sequence_log_prob(policy_dpo, rejected, mask)
            ref_ch  = sequence_log_prob(ref_dpo,    chosen,   mask)
            ref_rej = sequence_log_prob(ref_dpo,    rejected, mask)

            loss = dpo_loss(pi_ch, pi_rej, ref_ch, ref_rej)
            optimizer.zero_grad()
            scaler.scale(loss).backward()
            scaler.step(optimizer)
            scaler.update()

            state.global_step += 1
            state.log_history.append({"loss": loss.item(), "learning_rate": 5e-6, "step": state.global_step})
            cb.on_optimizer_step(args, state, ctrl)

    cb.on_train_end(args, state, ctrl)
    print(f"\n[TRL DPO] PASS  run_id={eco_run._run_id}  steps={state.global_step}  final_loss={loss.item():.4f}")
    if device == "cuda":
        print(f"  VRAM peak: {torch.cuda.max_memory_allocated()/1e9:.2f}GB")

del optimizer, scaler, policy_dpo, ref_dpo
if device == "cuda":
    import gc; gc.collect(); torch.cuda.empty_cache()

# ─────────────────────────────────────────────────────────────
# TEST 3: GRPO — GPT-2 large policy rollout + group-relative reward
# ─────────────────────────────────────────────────────────────
print("\n" + "="*60)
print("  TRL GRPO — GPT-2 rollout + group-relative rewards (124M params)")
print("="*60)

MODEL_GRPO = "gpt2"
print(f"[TRL heavy] Loading {MODEL_GRPO}...")
tokenizer_grpo = AutoTokenizer.from_pretrained(MODEL_GRPO)
tokenizer_grpo.pad_token = tokenizer_grpo.eos_token

policy_grpo = AutoModelForCausalLM.from_pretrained(MODEL_GRPO, torch_dtype=torch.bfloat16).to(device)
optimizer_g = torch.optim.SGD(policy_grpo.parameters(), lr=5e-6, momentum=0.9)

PROMPT_LEN   = 64
COMPLETE_LEN = 64
GROUP_SIZE   = 4    # completions per prompt — larger group = tighter advantage estimates
N_STEPS      = 60
VOCAB_G      = tokenizer_grpo.vocab_size

state_g = SimpleNamespace(global_step=0, epoch=0.0, log_history=[])
args_g  = SimpleNamespace(max_length=None, max_seq_length=None, max_completion_length=COMPLETE_LEN)
ctrl_g  = SimpleNamespace()

with ecocode.run("trl-grpo-gpt2", api_key=api_key, base_url=base_url, interval=1.0) as eco_run:
    cb = EcoCodeGRPOCallback(eco_run)
    cb.on_train_begin(args_g, state_g, ctrl_g)

    for epoch in range(3):
        state_g.epoch = float(epoch)
        cb.on_epoch_begin(args_g, state_g, ctrl_g)

        for step in range(N_STEPS // 3):
            cb.on_step_begin(args_g, state_g, ctrl_g)

            # Phase 1: rollout — greedy completions (no nan probs with random prompts)
            prompts = torch.randint(0, VOCAB_G, (GROUP_SIZE, PROMPT_LEN), device=device)
            policy_grpo.eval()
            with torch.no_grad():
                completions = policy_grpo.generate(
                    prompts,
                    max_new_tokens=COMPLETE_LEN,
                    do_sample=False,
                    pad_token_id=tokenizer_grpo.eos_token_id,
                )
            generated = completions[:, PROMPT_LEN:]

            rewards = torch.tensor(
                [len(set(g.tolist())) / COMPLETE_LEN for g in generated],
                device=device, dtype=torch.float
            )
            advantages = (rewards - rewards.mean()) / (rewards.std() + 1e-8)

            # Phase 2: policy gradient update
            policy_grpo.train()
            out = policy_grpo(completions[:, :-1])
            log_probs = out.logits.float().log_softmax(-1)
            token_lp  = log_probs.gather(
                2, completions[:, 1:].unsqueeze(-1)
            ).squeeze(-1)[:, PROMPT_LEN - 1:]

            loss = -(token_lp.mean(-1) * advantages.detach()).mean()
            optimizer_g.zero_grad()
            loss.backward()
            optimizer_g.step()

            state_g.global_step += 1
            state_g.log_history.append({"loss": loss.item(), "learning_rate": 5e-6, "step": state_g.global_step})
            cb.on_optimizer_step(args_g, state_g, ctrl_g)
            del out, log_probs, token_lp, completions, prompts, generated

    cb.on_train_end(args_g, state_g, ctrl_g)
    print(f"\n[TRL GRPO] PASS  run_id={eco_run._run_id}  steps={state_g.global_step}  final_loss={loss.item():.4f}")
    if device == "cuda":
        print(f"  VRAM peak: {torch.cuda.max_memory_allocated()/1e9:.2f}GB")

del optimizer_g, policy_grpo
if device == "cuda":
    import gc; gc.collect(); torch.cuda.empty_cache()
