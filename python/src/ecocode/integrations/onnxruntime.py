"""
ONNX Runtime inference integration.

Wraps ``onnxruntime.InferenceSession`` to mark inference phases and track
per-request metrics via ``log_custom()``.

Usage::

    import onnxruntime as ort
    import ecocode
    from ecocode.integrations.onnxruntime import EcoCodeORTWrapper

    session = ort.InferenceSession("model.onnx")
    with ecocode.run("ort-inference", api_key="eco_...") as eco_run:
        eco_session = EcoCodeORTWrapper(session, eco_run)
        result = eco_session.run(None, {"input": data})
"""
from __future__ import annotations

import time
from typing import TYPE_CHECKING, Any, Optional

if TYPE_CHECKING:
    from .. import EcoCodeRun


class EcoCodeORTWrapper:
    """
    Wrapper around ``onnxruntime.InferenceSession`` that marks inference phases.

    Phase lifecycle::

        model_load  →  prefill  →  [decode]  →  [decode]  →  ...

    There is no per-call or session-end ``done`` marker: ONNX Runtime's
    ``InferenceSession`` has no event that fires when the caller is finished
    issuing ``.run()`` calls, and marking ``done`` after every individual call
    would misuse the marker (elsewhere in this SDK ``done`` means "the whole
    run just finished," as in HuggingFace/Lightning/TRL's ``on_train_end``).

    The first call is always ``prefill``. Later calls are marked ``decode``
    when the first input tensor's sequence-length axis (``shape[1]``) is
    exactly 1, the standard signal for a KV-cache-based single-token decode
    step; otherwise they are marked ``prefill``. This is a heuristic, not a
    guarantee: a non-autoregressive model that happens to receive a
    single-token input on a later call would be misclassified as decode,
    since ONNX Runtime exposes no explicit signal distinguishing decode
    loops from one-shot small inputs.

    Also logs ``step`` (running count of inference calls), ``batch_size``
    (estimated from the first input tensor shape) and ``latency_ms``
    (wall-clock time for the ``.run()`` call).
    """

    def __init__(self, session: Any, run: "EcoCodeRun") -> None:
        self._session = session
        self._run = run
        self._total_inferences = 0
        run.mark("phase", "model_load")

    def run(
        self,
        output_names: Optional[list] = None,
        input_feed: Optional[dict] = None,
        run_options: Any = None,
    ):
        """Wraps ``InferenceSession.run()`` with phase markers and metrics."""
        self._total_inferences += 1
        self._run.log_step(self._total_inferences)

        # Estimate batch size from the first input tensor, and use its
        # sequence-length axis (if present) to distinguish prefill from
        # decode: shape[1] == 1 on a call after the first is treated as a
        # single-token KV-cache decode step (see class docstring). Some ONNX
        # backends return a symbolic or negative dimension for dynamic shapes,
        # so the batch size is logged only when it is a concrete positive
        # integer in a sensible range.
        is_decode = False
        if input_feed:
            first_val = next(iter(input_feed.values()), None)
            if first_val is not None and hasattr(first_val, "shape"):
                shape = first_val.shape
                batch_size = shape[0]
                if isinstance(batch_size, int) and 0 < batch_size < 1_000_000:
                    self._run.log_custom("batch_size", float(batch_size))
                if self._total_inferences > 1 and len(shape) >= 2 and shape[1] == 1:
                    is_decode = True

        self._run.mark("phase", "decode" if is_decode else "prefill")

        t0 = time.perf_counter()
        result = self._session.run(output_names, input_feed, run_options)
        elapsed_ms = (time.perf_counter() - t0) * 1000
        self._run.log_custom("latency_ms", elapsed_ms)

        return result

    def __getattr__(self, name: str):
        """Proxy all other attributes to the underlying session."""
        return getattr(self._session, name)
