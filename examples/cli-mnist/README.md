# CLI example: MNIST training

Monitors a 5-epoch CNN training run on MNIST with no code changes. The script keeps both the
CPU (data-loader transforms) and the GPU (extra forward passes and a large matmul) busy so that
every sensor has something to show.

```bash
pip install -r requirements.txt   # pinned for Linux + CUDA 12
ecocode run -o json -f out.json -- python script.py
```

`out.json` holds one record per sampling interval; see [docs/cli.md](../../docs/cli.md) for the
exporters and their fields.
