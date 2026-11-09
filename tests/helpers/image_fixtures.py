"""AUTHR_OS — storefront cover fixtures (pure-stdlib PNG builder).

The KDP pipeline validates covers from headers only (PNG IHDR / JPEG SOF),
so tests can synthesize exact-dimension images without an imaging library.
"""

import struct
import zlib
from pathlib import Path


def make_png(path: Path, width: int, height: int, color: tuple = (200, 60, 90)) -> Path:
    """Write a minimal valid 8-bit RGB PNG of `width` × `height` pixels.

    Deterministic and tiny: one solid-color row, DEFLATE-compressed, so even
    a 2560×1600 fixture stays a few hundred bytes.
    """
    def chunk(tag: bytes, data: bytes) -> bytes:
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)  # 8-bit RGB
    row = b"\x00" + bytes(color) * width  # filter byte 0 + RGB pixels
    idat = zlib.compress(row * height)
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(sig + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b""))
    return path
