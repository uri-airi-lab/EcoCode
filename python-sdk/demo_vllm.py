"""
EcoCode Real vLLM Demo (Qwen 2.5 0.5B - Long Run)
"""
import os
import sys
import glob

# --- CRITICAL CUDA FIX ---
venv_base = os.path.dirname(os.path.abspath(__file__))
nvidia_libs = glob.glob(os.path.join(venv_base, "venv/lib/python*/site-packages/nvidia/*/lib"), recursive=True)
if nvidia_libs:
    existing_path = os.environ.get("LD_LIBRARY_PATH", "")
    os.environ["LD_LIBRARY_PATH"] = ":".join(nvidia_libs) + ":" + existing_path

import asyncio
import time
import ecocode
from ecocode.integrations.vllm import EcoCodeVLLMMiddleware
from vllm import AsyncLLMEngine, AsyncEngineArgs, SamplingParams

async def run_real_vllm_demo():
    model_id = "Qwen/Qwen2.5-0.5B-Instruct"
    run_name = "vllm-long-qwen-0.5b"
    api_key = "ecocode_SvB6ETEjqwdJVjd6x2QkfdBZqXObta0F"
    
    engine_args = AsyncEngineArgs(
        model=model_id,
        dtype="half",
        gpu_memory_utilization=0.5,
        max_model_len=1024,
        enforce_eager=True,
        trust_remote_code=True,
    )

    print(f"\n🚀 Initializing vLLM for a LONG run with {model_id}...")
    
    with ecocode.run(run_name, api_key=api_key, interval=0.5) as run:
        try:
            raw_engine = AsyncLLMEngine.from_engine_args(engine_args)
            engine = EcoCodeVLLMMiddleware(raw_engine, run)

            print("\n📥 Processing Request...")
            # More complex prompt to encourage a long response
            prompt = "Write a detailed essay (about 400 words) on the future of sustainable AI. Discuss hardware efficiency, algorithmic improvements, and the role of renewable energy in data centers."
            
            # Increase max_tokens to allow a long response
            sampling_params = SamplingParams(temperature=0.7, max_tokens=600)

            print("✍️  Generating LONG Response:\n")
            start_time = time.time()
            
            async for output in engine.generate(prompt, sampling_params, request_id="demo-long"):
                if output.finished:
                    print(f"\n\n✅ Finished in {time.time() - start_time:.2f}s")
                    print(f"Final Output:\n{output.outputs[0].text}")

            print("\n--------------------------------------------------")
            print(f"View the energy metrics at: http://localhost:3001\n")
        except Exception as e:
            print(f"\n❌ Error: {e}")

if __name__ == "__main__":
    asyncio.run(run_real_vllm_demo())
