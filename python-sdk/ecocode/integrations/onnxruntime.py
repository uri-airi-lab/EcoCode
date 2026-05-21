"""
EcoCode — ONNX Runtime Inference Integration.

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

    Phase lifecycle per ``.run()`` call::

        model_load  →  prefill  →  done

    Also logs:
      - ``step``         — running count of inference calls
      - ``batch_size``   — estimated from the first input tensor shape
      - ``latency_ms``   — wall-clock time for the ``.run()`` call
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
        self._run.mark("phase", "prefill")
        self._total_inferences += 1
        self._run.log_step(self._total_inferences)

        # Estimate batch size from the first input tensor.
        # CS-TYPE-002: Validate the shape value before forwarding. Some ONNX
        # backends may return a symbolic/negative dimension for dynamic shapes;
        # log only if the value is a concrete positive integer within a
        # reasonable range to avoid corrupting dashboard aggregates.
        if input_feed:
            first_val = next(iter(input_feed.values()), None)
            if first_val is not None and hasattr(first_val, "shape"):
                batch_size = first_val.shape[0]
                if isinstance(batch_size, int) and 0 < batch_size < 1_000_000:
                    self._run.log_custom("batch_size", float(batch_size))

        t0 = time.perf_counter()
        result = self._session.run(output_names, input_feed, run_options)
        elapsed_ms = (time.perf_counter() - t0) * 1000
        self._run.log_custom("latency_ms", elapsed_ms)

        self._run.mark("phase", "done")
        return result

    def __getattr__(self, name: str):
        """Proxy all other attributes to the underlying session."""
        return getattr(self._session, name)
