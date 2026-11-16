"""Shared, stdlib-only test helpers (tools.md: the tools use only the standard library)."""

import struct
import zlib


def build_rgba_png(width: int, height: int, pixels: bytes) -> bytes:
    """Encode `pixels` (row-major RGBA, 4 bytes per pixel, unfiltered) as a minimal PNG.

    The inverse of `steg_tools.gallery.decode_png_rgba`, built independently (filter type
    0 on every row, no PNG library) so a round trip through both proves the decoder
    against bytes it didn't produce itself.
    """

    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    stride = width * 4
    raw = b"".join(b"\x00" + pixels[row * stride : (row + 1) * stride] for row in range(height))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")
