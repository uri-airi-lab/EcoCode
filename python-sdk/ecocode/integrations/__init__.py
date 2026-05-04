# Integrations are imported lazily to avoid hard dependencies at top level.
# Import the integration you need explicitly:
#
#   from ecocode.integrations.huggingface import EcoCodeCallback
#   from ecocode.integrations.trl import EcoCodeSFTCallback, EcoCodeDPOCallback, EcoCodeGRPOCallback
#   from ecocode.integrations.lightning import EcoCodeCallback
#   from ecocode.integrations.deepspeed import EcoCodeDeepSpeedCallback
#   from ecocode.integrations.vllm import EcoCodeVLLMMiddleware
