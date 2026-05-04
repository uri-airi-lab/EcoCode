"""
EcoCode SDK test suite (Rust-powered version).

This suite verifies the Python-side logic and integrations.
Since the core measurement logic is now in Rust, we mock the `ecocode.EcoCodeRun`
class to verify that decorators and callbacks call the correct methods.
"""
from __future__ import annotations

import os
import sys
import unittest
import warnings
from unittest.mock import MagicMock, patch

# ── make sure the local package is importable ────────────────────────────────
sys.path.insert(0, os.path.dirname(__file__))

import ecocode

# ─────────────────────────────────────────────────────────────────────────────
# 1. Lifecycle & Decorators
# ─────────────────────────────────────────────────────────────────────────────

class TestSDKLifecycle(unittest.TestCase):

    @patch("ecocode.EcoCodeRun")
    def test_run_context_manager(self, mock_run_class):
        mock_instance = mock_run_class.return_value
        mock_instance.__enter__.return_value = mock_instance
        
        with ecocode.run("test", api_key="k", project_id=1) as run:
            run.mark("phase", "test")
            
        mock_instance.__enter__.assert_called_once()
        mock_instance.mark.assert_called_with("phase", "test")
        mock_instance.__exit__.assert_called_once()

    @patch("ecocode.EcoCodeRun")
    def test_track_decorator(self, mock_run_class):
        mock_instance = mock_run_class.return_value
        mock_instance.__enter__.return_value = mock_instance
        called = []

        @ecocode.track("decorated", api_key="k", project_id=1)
        def my_train():
            called.append(True)

        my_train()

        self.assertTrue(called)
        mock_instance.__enter__.assert_called_once()
        mock_instance.__exit__.assert_called_once()

    def test_track_decorator_preserves_name(self):
        @ecocode.track(api_key="k", project_id=1)
        def my_function():
            pass
        self.assertEqual(my_function.__name__, "my_function")


# ─────────────────────────────────────────────────────────────────────────────
# 2. Integration Callbacks (HuggingFace, TRL, etc.)
# ─────────────────────────────────────────────────────────────────────────────

class TestIntegrations(unittest.TestCase):

    def _state(self, step=0, epoch=0.0):
        state = MagicMock()
        state.global_step = step
        state.epoch = epoch
        state.log_history = [{"loss": 1.0, "learning_rate": 1e-4}]
        state.num_input_tokens_seen = 1000
        return state

    def test_huggingface_callback(self):
        from ecocode.integrations.huggingface import EcoCodeCallback
        run = MagicMock()
        cb = EcoCodeCallback(run)
        
        cb.on_train_begin(MagicMock(), self._state(), MagicMock())
        run.mark.assert_called_with("phase", "data_loading")
        
        cb.on_optimizer_step(MagicMock(), self._state(step=10), MagicMock())
        run.log_step.assert_called_with(10)

    def test_trl_sft_callback(self):
        from ecocode.integrations.trl import EcoCodeSFTCallback
        run = MagicMock()
        cb = EcoCodeSFTCallback(run)
        cb.on_step_begin(None, None, None)
        run.mark.assert_called_with("phase", "forward")

    def test_lightning_callback(self):
        from ecocode.integrations.lightning import EcoCodeCallback
        run = MagicMock()
        cb = EcoCodeCallback(run)
        trainer = MagicMock()
        trainer.current_epoch = 5
        cb.on_train_epoch_start(trainer, None)
        run.log_epoch.assert_called_with(5)

    def test_deepspeed_callback(self):
        from ecocode.integrations.deepspeed import EcoCodeDeepSpeedCallback
        run = MagicMock()
        cb = EcoCodeDeepSpeedCallback(run, zero_stage=2)
        cb.on_backward_begin()
        run.mark.assert_called_with("phase", "backward")


# ─────────────────────────────────────────────────────────────────────────────
# 3. vLLM & Async
# ─────────────────────────────────────────────────────────────────────────────

class TestVLLM(unittest.TestCase):

    def test_inference_tracker(self):
        from ecocode.integrations.vllm import InferencePhaseTracker
        run = MagicMock()
        tracker = InferencePhaseTracker(run)
        tracker.on_prefill_begin(prompt_tokens=128)
        run.mark.assert_called_with("phase", "prefill")
        run.log_sequence_length.assert_called_with(128)


if __name__ == "__main__":
    unittest.main(verbosity=2)
