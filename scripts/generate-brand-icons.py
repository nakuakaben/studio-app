#!/usr/bin/env python3
"""Generate deterministic Studio App PNG iconset with standard-library only."""
import pathlib
import struct
import sys
import zlib

SIZES = {
    "icon_16x16.png": 16,
    "icon_16x16@2x.png": 32,
    "icon_32x32.png": 32,
    "icon_32x32@2x.png": 64,
    "icon_128x128.png": 128,
    "icon_128x128@2x.png": 256,
    "icon_256x256.png": 256,
    "icon_256x256@2x.png": 512,
    "icon_512x512.png": 512,
    "icon_512x512@2x.png": 1024,
}


def rounded(x, y, left, top, right, bottom, radius):
    cx = max(left + radius, min(right - radius, x))
    cy = max(top + radius, min(bottom - radius, y))
    return (x - cx) ** 2 + (y - cy) ** 2 <= radius ** 2


def pixel(x, y):
    if not rounded(x, y, 0, 0, 256, 256, 54):
        return (0, 0, 0, 0)
    if rounded(x, y, 34, 48, 222, 90, 12):
        return (162, 104, 232, 255)
    if rounded(x, y, 34, 110, 112, 208, 12):
        return (52, 144, 242, 255)
    if rounded(x, y, 134, 110, 222, 208, 12):
        return (127, 95, 229, 255)
    return (16, 17, 23, 255)


def chunk(tag, data):
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)


def png(size):
    rows = []
    for y in range(size):
        row = bytearray([0])
        for x in range(size):
            row.extend(pixel((x + 0.5) * 256 / size, (y + 0.5) * 256 / size))
        rows.append(bytes(row))
    return (
        b"\\x89PNG\\r\\n\\x1a\\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(b"".join(rows), 9))
        + chunk(b"IEND", b"")
    )


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("Usage: generate-brand-icons.py <Icon.iconset-directory>")
    output = pathlib.Path(sys.argv[1])
    output.mkdir(parents=True, exist_ok=True)
    for name, size in SIZES.items():
        (output / name).write_bytes(png(size))
