from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path

import onnx
from onnx.external_data_helper import (
    ExternalDataInfo,
    _get_all_tensors,
    uses_external_data,
)

COPY_BUFFER_SIZE = 8 * 1024 * 1024
DEFAULT_MAX_CHUNK_BYTES = 1024 * 1024 * 1024


@dataclass(frozen=True)
class TensorSpan:
    tensor: onnx.TensorProto
    location: str
    offset: int
    length: int


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Rewrite oversized ONNX external data blobs into smaller chunk files."
    )
    parser.add_argument("--model", required=True, help="Path to the model.onnx file to rewrite.")
    parser.add_argument(
        "--max-chunk-bytes",
        type=int,
        default=DEFAULT_MAX_CHUNK_BYTES,
        help=f"Maximum size for each generated external data chunk. Default: {DEFAULT_MAX_CHUNK_BYTES}.",
    )
    return parser.parse_args()


def collect_external_spans(model: onnx.ModelProto) -> dict[str, list[TensorSpan]]:
    spans_by_location: dict[str, list[TensorSpan]] = {}

    for tensor in _get_all_tensors(model):
        if not uses_external_data(tensor):
            continue

        info = ExternalDataInfo(tensor)
        if not info.location:
            raise ValueError(
                f"Tensor '{tensor.name or '<unnamed>'}' uses external data but has no location."
            )
        if info.length is None:
            raise ValueError(
                f"Tensor '{tensor.name or '<unnamed>'}' is missing an external data length."
            )

        spans_by_location.setdefault(info.location, []).append(
            TensorSpan(
                tensor=tensor,
                location=info.location,
                offset=int(info.offset or 0),
                length=int(info.length),
            )
        )

    return spans_by_location


def validate_spans(source_path: Path, spans: list[TensorSpan]) -> None:
    file_size = source_path.stat().st_size
    previous_end = 0

    for span in sorted(spans, key=lambda item: item.offset):
        if span.length <= 0:
            raise ValueError(
                f"Tensor '{span.tensor.name or '<unnamed>'}' has an invalid external data length: {span.length}."
            )

        if span.offset < previous_end:
            raise ValueError(
                f"Tensor '{span.tensor.name or '<unnamed>'}' overlaps a previous tensor in '{source_path.name}'."
            )

        end = span.offset + span.length
        if end > file_size:
            raise ValueError(
                f"Tensor '{span.tensor.name or '<unnamed>'}' reads past the end of '{source_path.name}'."
            )

        previous_end = end


def clear_existing_part_files(source_path: Path) -> None:
    for existing in source_path.parent.glob(source_path.name + ".part*"):
        if existing.is_file():
            existing.unlink()


def update_external_reference(
    tensor: onnx.TensorProto,
    location: str,
    offset: int,
    length: int,
) -> None:
    del tensor.external_data[:]
    tensor.data_location = onnx.TensorProto.EXTERNAL

    for key, value in (
        ("location", location),
        ("offset", str(offset)),
        ("length", str(length)),
    ):
        entry = tensor.external_data.add()
        entry.key = key
        entry.value = value


def copy_range(source_handle, destination_handle, offset: int, length: int) -> None:
    source_handle.seek(offset)
    remaining = length

    while remaining > 0:
        chunk = source_handle.read(min(COPY_BUFFER_SIZE, remaining))
        if not chunk:
            raise EOFError("Unexpected end of file while copying ONNX external data.")

        destination_handle.write(chunk)
        remaining -= len(chunk)


def rewrite_location(
    model_dir: Path,
    source_location: str,
    spans: list[TensorSpan],
    max_chunk_bytes: int,
) -> int:
    source_path = (model_dir / source_location).resolve()
    if not source_path.is_file():
        raise FileNotFoundError(f"Missing external data file referenced by ONNX model: {source_path}")

    validate_spans(source_path, spans)
    source_size = source_path.stat().st_size
    if source_size <= max_chunk_bytes:
        return 0

    largest_tensor = max(span.length for span in spans)
    if largest_tensor > max_chunk_bytes:
        raise ValueError(
            f"The largest tensor in '{source_path.name}' is {largest_tensor} bytes, "
            f"which exceeds the configured max chunk size of {max_chunk_bytes} bytes."
        )

    clear_existing_part_files(source_path)

    sorted_spans = sorted(spans, key=lambda item: item.offset)
    chunk_index = -1
    current_chunk_size = 0
    current_chunk_location = ""
    current_chunk_handle = None
    written_chunks = 0

    try:
        with source_path.open("rb") as source_handle:
            for span in sorted_spans:
                if current_chunk_handle is None or current_chunk_size + span.length > max_chunk_bytes:
                    if current_chunk_handle is not None:
                        current_chunk_handle.close()

                    chunk_index += 1
                    current_chunk_size = 0
                    current_chunk_location = f"{source_location}.part{chunk_index:03d}"
                    chunk_path = model_dir / current_chunk_location
                    chunk_path.parent.mkdir(parents=True, exist_ok=True)
                    current_chunk_handle = chunk_path.open("wb")
                    written_chunks += 1

                copy_range(source_handle, current_chunk_handle, span.offset, span.length)
                update_external_reference(
                    tensor=span.tensor,
                    location=current_chunk_location,
                    offset=current_chunk_size,
                    length=span.length,
                )
                current_chunk_size += span.length
    finally:
        if current_chunk_handle is not None:
            current_chunk_handle.close()

    source_path.unlink()
    return written_chunks


def save_model(model_path: Path, model: onnx.ModelProto) -> None:
    temp_path = model_path.with_suffix(model_path.suffix + ".tmp")
    temp_path.write_bytes(model.SerializeToString())
    temp_path.replace(model_path)


def validate_model(model_path: Path) -> None:
    model = onnx.load_model(model_path, load_external_data=False)

    model_dir = model_path.parent
    missing_files: list[str] = []
    for tensor in _get_all_tensors(model):
        if not uses_external_data(tensor):
            continue

        info = ExternalDataInfo(tensor)
        candidate = model_dir / info.location
        if not candidate.is_file():
            missing_files.append(str(candidate))

    if missing_files:
        missing_list = "\n".join(sorted(set(missing_files)))
        raise FileNotFoundError(f"Missing ONNX external data files after rewrite:\n{missing_list}")


def main() -> int:
    args = parse_args()
    if args.max_chunk_bytes <= 0:
        raise ValueError("--max-chunk-bytes must be greater than zero.")

    model_path = Path(args.model).resolve()
    if not model_path.is_file():
        raise FileNotFoundError(f"Model file not found: {model_path}")

    model = onnx.load_model(model_path, load_external_data=False)
    spans_by_location = collect_external_spans(model)
    if not spans_by_location:
        print(f"No external data found for {model_path}")
        return 0

    rewritten_locations = 0
    generated_chunks = 0
    for location, spans in sorted(spans_by_location.items()):
        chunk_count = rewrite_location(
            model_dir=model_path.parent,
            source_location=location,
            spans=spans,
            max_chunk_bytes=args.max_chunk_bytes,
        )
        if chunk_count > 0:
            rewritten_locations += 1
            generated_chunks += chunk_count

    if rewritten_locations == 0:
        print(f"External data already fits the configured chunk limit for {model_path}")
        return 0

    save_model(model_path, model)
    validate_model(model_path)
    print(
        f"Rewrote {rewritten_locations} external data file(s) into "
        f"{generated_chunks} chunk file(s) for {model_path}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
