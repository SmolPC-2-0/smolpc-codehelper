"""Quantize Qwen3-4B FP16 OpenVINO IR to INT8_SYM for NPU compatibility."""
import openvino as ov
import nncf
import shutil
import os
import sys
import time

SRC = r"C:\Users\Student\AppData\Local\SmolPC\models\qwen3-4b\openvino"
DST = r"C:\Users\Student\AppData\Local\SmolPC\models\qwen3-4b\openvino-int8"

if os.path.exists(DST):
    print(f"ERROR: {DST} already exists. Remove it first.")
    sys.exit(1)

print(f"Source: {SRC}")
print(f"Dest:   {DST}")

src_bin = os.path.join(SRC, "openvino_model.bin")
src_size_gb = os.path.getsize(src_bin) / (1024**3)
print(f"Source model size: {src_size_gb:.2f} GB")

# Read model
print("\n[1/4] Reading FP16 model...")
t0 = time.time()
core = ov.Core()
model = core.read_model(os.path.join(SRC, "openvino_model.xml"))
print(f"  Done in {time.time() - t0:.1f}s")

# Compress weights
print("\n[2/4] Compressing weights to INT8_SYM...")
t0 = time.time()
compressed = nncf.compress_weights(model, mode=nncf.CompressWeightsMode.INT8_SYM)
print(f"  Done in {time.time() - t0:.1f}s")

# Save
print("\n[3/4] Saving INT8 model...")
t0 = time.time()
os.makedirs(DST, exist_ok=True)
ov.save_model(compressed, os.path.join(DST, "openvino_model.xml"))
print(f"  Done in {time.time() - t0:.1f}s")

# Free RAM
del model, compressed

dst_bin = os.path.join(DST, "openvino_model.bin")
dst_size_gb = os.path.getsize(dst_bin) / (1024**3)
print(f"  INT8 model size: {dst_size_gb:.2f} GB ({dst_size_gb/src_size_gb*100:.0f}% of FP16)")

# Copy non-model files
print("\n[4/4] Copying tokenizer and config files...")
SKIP = {"openvino_model.xml", "openvino_model.bin", ".gitattributes", "README.md", ".cache"}
copied = []
for f in os.listdir(SRC):
    src_path = os.path.join(SRC, f)
    if f not in SKIP and os.path.isfile(src_path):
        shutil.copy2(src_path, os.path.join(DST, f))
        copied.append(f)

print(f"  Copied {len(copied)} files: {', '.join(sorted(copied))}")

print("\nQuantization complete!")
print(f"  FP16: {src_size_gb:.2f} GB -> INT8: {dst_size_gb:.2f} GB")
