# CLI example: MNIST training

Monitors a 5-epoch CNN training run on MNIST with no code changes.

```bash
pip install -r requirements.txt   # pinned for Linux + CUDA 12
ecocode run -o json -f out.json -- python script.py
```

`sample-output.json` is a 30-second excerpt of this run on a Linux machine
with one NVIDIA GPU (RAPL `cpu` domain only, no DRAM domain). It was recorded
with an earlier CLI version, so records do not include the `elapsed_secs` and
marker fields that current versions write.
