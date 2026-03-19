#!/usr/bin/env python3
from __future__ import annotations

"""
Convert ONNX -> PyTorch -> CoreML (mlprogram) with support for dynamic input shapes.

This script converts ONNX models to CoreML format, similar to how Parakeet models work.
It supports variable-length audio inputs (like Parakeet's transcribe_samples pattern).

Usage patterns:
  # Fixed shapes (for models that require it)
  python convert_onnx_to_coreml.py --onnx model.onnx --out model.mlpackage \\
    --input "waveforms:1,16000" --input "waveforms_lens:1"

  # Dynamic shapes with range (Parakeet-like, recommended for audio)
  python convert_onnx_to_coreml.py --onnx model.onnx --out model.mlpackage \\
    --input-range "waveforms:1,800,-1" --input "waveforms_lens:1"

  # Unbounded dynamic shapes
  python convert_onnx_to_coreml.py --onnx model.onnx --out model.mlpackage \\
    --input-range "waveforms:1,800,-1" --input-range "waveforms_lens:1,1,-1"
"""
import argparse
import sys
from pathlib import Path

import numpy as np
import onnx
import torch
import coremltools as ct
from coremltools.converters.mil.input_types import RangeDim
from onnx2pytorch import ConvertModel


def parse_target(s: str):
    s = s.strip()
    try:
        return getattr(ct.target, s)
    except AttributeError:
        return ct.target.iOS16


def parse_shape_spec(spec: str) -> tuple[str, list[int]]:
    """Parse shape spec like 'name:1,128' or 'name:1,800' into (name, [1, 128])."""
    if ":" not in spec:
        raise ValueError(f"Invalid shape spec: {spec}. Expected format: 'name:dim1,dim2,...'")
    name, dims_str = spec.split(":", 1)
    dims = [int(x.strip()) for x in dims_str.split(",") if x.strip()]
    return name, dims


def parse_range_spec(spec: str) -> tuple[str, list[tuple[int, int, int]]]:
    """
    Parse range spec like 'name:1,800,-1' into (name, [(1, 1, 1), (800, -1, 800)]).
    Format: name:dim1_lower,dim1_upper,dim1_default,dim2_lower,dim2_upper,dim2_default,...
    If upper_bound is -1, it's unbounded.
    If default is omitted, uses lower_bound.
    """
    if ":" not in spec:
        raise ValueError(f"Invalid range spec: {spec}. Expected format: 'name:lower1,upper1,default1,lower2,upper2,default2,...'")
    name, dims_str = spec.split(":", 1)
    parts = [x.strip() for x in dims_str.split(",") if x.strip()]
    
    if len(parts) % 3 != 0:
        # Try to infer defaults: if only 2 values per dim, use lower as default
        if len(parts) % 2 == 0:
            inferred = []
            for i in range(0, len(parts), 2):
                inferred.extend([parts[i], parts[i+1], parts[i]])  # lower, upper, default=lower
            parts = inferred
        else:
            raise ValueError(f"Range spec must have 2 or 3 values per dimension. Got: {spec}")
    
    ranges = []
    for i in range(0, len(parts), 3):
        lower = int(parts[i])
        upper = int(parts[i+1])
        default = int(parts[i+2]) if i+2 < len(parts) else lower
        ranges.append((lower, upper, default))
    
    return name, ranges


def main():
    p = argparse.ArgumentParser(
        description="Convert ONNX -> PyTorch -> CoreML (mlprogram) with dynamic shape support.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )
    p.add_argument("--onnx", required=True, help="Path to .onnx file")
    p.add_argument("--out", required=True, help="Output .mlpackage path")
    p.add_argument("--minimum_deployment_target", default="iOS16")
    p.add_argument("--fp16", action="store_true")
    # Fixed input shapes (for models that require concrete shapes)
    p.add_argument(
        "--input",
        action="append",
        default=[],
        help='Fixed input shape like: --input "waveforms:1,16000" (repeat per input)',
    )
    # Dynamic input shapes with ranges (Parakeet-like, recommended for audio)
    p.add_argument(
        "--input-range",
        action="append",
        default=[],
        help='Dynamic input range like: --input-range "waveforms:1,800,-1" '
             '(format: name:lower,upper,default per dimension; use -1 for unbounded upper)',
    )
    args = p.parse_args()

    onnx_path = Path(args.onnx).expanduser().resolve()
    if not onnx_path.exists():
        print(f"ERROR: ONNX not found: {onnx_path}", file=sys.stderr)
        sys.exit(2)

    out_path = Path(args.out).expanduser().resolve()
    out_path.parent.mkdir(parents=True, exist_ok=True)
    if out_path.suffix.lower() != ".mlpackage":
        print("ERROR: --out must end with .mlpackage", file=sys.stderr)
        sys.exit(2)

    deployment_target = parse_target(args.minimum_deployment_target)

    # Load ONNX
    model_onnx = onnx.load(str(onnx_path))

    # Convert ONNX -> PyTorch module
    torch_model = ConvertModel(model_onnx, experimental=True)
    torch_model.eval()

    # Build example inputs in the right order:
    # ONNX input order is model_onnx.graph.input (excluding initializers)
    onnx_input_names = []
    initializer_names = {init.name for init in model_onnx.graph.initializer}
    for i in model_onnx.graph.input:
        if i.name not in initializer_names:
            onnx_input_names.append(i.name)

    if not args.input and not args.input_range:
        print(
            "ERROR: You must provide --input or --input-range specs because dynamic shapes are common in ONNX.\n"
            'Examples:\n'
            '  Fixed shapes: --input "waveforms:1,16000"\n'
            '  Dynamic shapes (Parakeet-like): --input-range "waveforms:1,800,-1"\n',
            file=sys.stderr,
        )
        sys.exit(2)

    # Parse fixed input shapes
    fixed_shape_map = {}
    for spec in args.input:
        try:
            name, dims = parse_shape_spec(spec)
            fixed_shape_map[name] = dims
        except ValueError as e:
            print(f"ERROR: bad --input spec: {e}", file=sys.stderr)
            sys.exit(2)

    # Parse dynamic input ranges
    range_map = {}
    for spec in args.input_range:
        try:
            name, ranges = parse_range_spec(spec)
            range_map[name] = ranges
        except ValueError as e:
            print(f"ERROR: bad --input-range spec: {e}", file=sys.stderr)
            sys.exit(2)

    # Check for conflicts
    overlapping = set(fixed_shape_map.keys()) & set(range_map.keys())
    if overlapping:
        print(
            f"ERROR: Input '{overlapping.pop()}' specified in both --input and --input-range. "
            "Use only one method per input.",
            file=sys.stderr,
        )
        sys.exit(2)

    # Create example tensors and CoreML input specs
    example_inputs = []
    coreml_inputs = []
    
    for name in onnx_input_names:
        # Determine dtype based on input name
        is_int = any(k in name.lower() for k in ["id", "ids", "token", "tokens", "mask", "len", "lens"])
        dtype = torch.int64 if is_int else torch.float32

        if name in fixed_shape_map:
            # Fixed shape
            dims = fixed_shape_map[name]
            t = torch.zeros(*dims, dtype=dtype)
            example_inputs.append(t)
            coreml_inputs.append(
                ct.TensorType(name=name, shape=dims)
            )
        elif name in range_map:
            # Dynamic shape with range
            ranges = range_map[name]
            # Build shape with default values for tracing
            default_dims = [r[2] for r in ranges]  # default values
            t = torch.zeros(*default_dims, dtype=dtype)
            example_inputs.append(t)
            
            # Build CoreML flexible shape spec
            shape_dims = []
            for lower, upper, default in ranges:
                if upper == -1:
                    # Unbounded upper limit
                    shape_dims.append(RangeDim(lower_bound=lower, upper_bound=-1, default=default))
                else:
                    # Bounded range
                    shape_dims.append(RangeDim(lower_bound=lower, upper_bound=upper, default=default))
            
            coreml_inputs.append(
                ct.TensorType(name=name, shape=shape_dims)
            )
        else:
            print(
                f"ERROR: missing input spec for ONNX input '{name}'.\n"
                f"Provide either:\n"
                f"  --input \"{name}:1,128\" (fixed shape)\n"
                f"  --input-range \"{name}:1,800,-1\" (dynamic shape, Parakeet-like)",
                file=sys.stderr,
            )
            sys.exit(2)

    # Trace for CoreML conversion
    with torch.no_grad():
        traced = torch.jit.trace(torch_model, tuple(example_inputs))

    # Convert PyTorch -> CoreML ML Program
    # Note: Output length is automatically handled by CoreML - no need to specify it
    # The model will generate outputs until completion (like Parakeet's transcribe_samples)
    print(f"Converting to CoreML with {len(coreml_inputs)} input(s)...")
    for inp in coreml_inputs:
        if inp.name in range_map:
            print(f"  {inp.name}: dynamic shape (flexible)")
        else:
            print(f"  {inp.name}: fixed shape {inp.shape}")
    
    mlmodel = ct.convert(
        traced,
        convert_to="mlprogram",
        minimum_deployment_target=deployment_target,
        compute_units=ct.ComputeUnit.ALL,
        inputs=coreml_inputs,
    )

    if args.fp16:
        # Optional FP16 compression (may fail for some ML Program graphs)
        try:
            mlmodel = ct.models.neural_network.quantization_utils.quantize_weights(mlmodel, nbits=16)
        except Exception as e:
            print(f"WARNING: FP16 compression failed; continuing. ({e})", file=sys.stderr)

    mlmodel.save(str(out_path))
    print(f"\n✅ Saved: {out_path}")
    
    # Check if dynamic shapes were used
    has_dynamic = len(range_map) > 0
    
    if has_dynamic:
        print("\n📝 Dynamic shapes enabled (Parakeet-like):")
        print("   - Model accepts variable-length audio inputs")
        print("   - Called whenever VAD detects speech (not on a timer)")
        print("   - No output length specification needed - model generates until completion")
        print("   - Similar to Parakeet's transcribe_samples() pattern")
        print("\n   Usage pattern:")
        print("   - Audio chunks: minimum 800 samples (50ms @ 16kHz), variable max length")
        print("   - Transcription triggered by VAD speech detection")
        print("   - Model processes each chunk independently")
    
    print("\nNext: compile to .mlmodelc:")
    print(f"  xcrun coremlcompiler compile {out_path} .")


if __name__ == "__main__":
    main()
