"""Real-framework tests: tiny randomly initialised models driven through the
real framework trainers, the real EcoCode callbacks and the real ecocode
binary (fake sensors, JSON exporter). No network access and no GPU needed.

Shared fixtures (``ecocode_bin``, ``marker_tap``, ``json_run``) live in
``tests/conftest.py``.
"""
from __future__ import annotations

import os

import pytest

# Must be set before keras / tokenizers are imported anywhere in the session.
os.environ.setdefault("KERAS_BACKEND", "torch")
os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")
os.environ.setdefault("WANDB_DISABLED", "true")


@pytest.fixture(autouse=True)
def _offline(monkeypatch):
    """These tests never download anything; make an accidental download fail fast."""
    monkeypatch.setenv("HF_HUB_OFFLINE", "1")

pytestmark = pytest.mark.frameworks

# A tiny closed vocabulary so the tokenizer never touches the network.
WORDS = (
    "the a an of and to in is for on with by at from that this it as be are "
    "model training loss gradient attention token layer learning rate data "
    "epoch step batch energy carbon green compute fast slow small large deep "
    "neural network transformer encoder decoder prefill decode inference"
).split()
SPECIAL = ["[PAD]", "[UNK]", "[BOS]", "[EOS]"]


def _torch():
    torch = pytest.importorskip("torch")
    torch.manual_seed(0)
    torch.set_num_threads(2)
    return torch


@pytest.fixture
def torch():
    return _torch()


def make_tokenizer():
    """A real ``PreTrainedTokenizerFast`` built from an in-memory word-level vocab."""
    transformers = pytest.importorskip("transformers")
    from tokenizers import Tokenizer, models, pre_tokenizers

    vocab = {w: i for i, w in enumerate(SPECIAL + WORDS)}
    tok = Tokenizer(models.WordLevel(vocab, unk_token="[UNK]"))
    tok.pre_tokenizer = pre_tokenizers.Whitespace()
    return transformers.PreTrainedTokenizerFast(
        tokenizer_object=tok,
        pad_token="[PAD]",
        unk_token="[UNK]",
        bos_token="[BOS]",
        eos_token="[EOS]",
        model_max_length=64,
    )


def make_tiny_gpt2(tokenizer):
    """A 2-layer GPT-2 with random weights sized to the tiny vocab (~30k params)."""
    transformers = pytest.importorskip("transformers")
    _torch()
    config = transformers.GPT2Config(
        vocab_size=len(tokenizer),
        n_positions=64,
        n_embd=32,
        n_layer=2,
        n_head=2,
        bos_token_id=tokenizer.bos_token_id,
        eos_token_id=tokenizer.eos_token_id,
        pad_token_id=tokenizer.pad_token_id,
    )
    return transformers.GPT2LMHeadModel(config)


def sentences(n: int) -> list[str]:
    import random

    rng = random.Random(0)
    return [" ".join(rng.choice(WORDS) for _ in range(rng.randint(6, 14))) for _ in range(n)]


@pytest.fixture
def tokenizer():
    return make_tokenizer()


@pytest.fixture
def tiny_gpt2(tokenizer):
    return make_tiny_gpt2(tokenizer)


@pytest.fixture
def corpus():
    return sentences(64)


@pytest.fixture
def tiny_gpt2_factory(tokenizer):
    """Build additional tiny models (e.g. a DPO reference model)."""
    return lambda: make_tiny_gpt2(tokenizer)
