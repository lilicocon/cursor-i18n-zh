#!/usr/bin/env python3
"""Pack PNG files into a Vista-style ICO (PNG-compressed images)."""
from __future__ import annotations

import struct
import sys
from pathlib import Path


def png_size(data: bytes) -> tuple[int, int]:
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG")
    width, height = struct.unpack(">II", data[16:24])
    return width, height


def write_ico(png_paths: list[Path], dest: Path) -> None:
    images = []
    for path in png_paths:
        data = path.read_bytes()
        width, height = png_size(data)
        images.append((width, height, data))
    images.sort(key=lambda item: item[0])

    offset = 6 + 16 * len(images)
    header = struct.pack("<HHH", 0, 1, len(images))
    entries = b""
    payload = b""
    for width, height, data in images:
        entries += struct.pack(
            "<BBBBHHII",
            width if width < 256 else 0,
            height if height < 256 else 0,
            0,
            0,
            1,
            32,
            len(data),
            offset,
        )
        payload += data
        offset += len(data)
    dest.write_bytes(header + entries + payload)
    print(f"wrote {dest} ({len(images)} images)")


if __name__ == "__main__":
    dest = Path(sys.argv[1])
    write_ico([Path(item) for item in sys.argv[2:]], dest)
