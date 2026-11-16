"""Confirm a captured gallery image actually shows the gallery, not a blank or wrong capture.

`just capture-gallery` (justfile) opens the gallery window, finds its `CGWindowID`
(`crates/ui/gallery/window_id.swift`), and runs `screencapture -l<id>` - which succeeds
(exit 0) whether or not the calling terminal holds the Screen Recording permission the
capture needs: a denied capture can still produce an empty or desktop PNG with a clean
exit code. This module reads one pixel back out of the PNG `screencapture` wrote and
checks it against the gallery's own known placeholder colour (TASK-1.91's plan), so a
missing permission - or any other reason the capture shows the wrong thing - fails the
recipe loudly instead of attaching a blank image to a pull request.

Usage: steg-gallery-check <path> <x> <y> <r> <g> <b>
"""

import struct
import sys
import zlib
from dataclasses import dataclass


@dataclass(frozen=True)
class DecodedImage:
    """A decoded 8-bit RGBA, non-interlaced PNG: its size and its pixels, row-major."""

    width: int
    height: int
    pixels: bytes


def decode_png_rgba(data: bytes) -> DecodedImage:
    """Decode an 8-bit RGBA, non-interlaced PNG into its raw pixels.

    `screencapture` always writes this exact form for a window capture, so this is not a
    general-purpose PNG decoder - it is precisely as general as the one input it has to
    read, with no image-crate dependency (steg-ui's own TASK-1.91 plan: the capture
    mechanism, including checking it, adds no new licence surface).

    Raises:
        ValueError: `data` is not a PNG, or not 8-bit RGBA, non-interlaced.
    """
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG")
    width = height = None
    idat = b""
    pos = 8
    while pos < len(data):
        length = struct.unpack(">I", data[pos : pos + 4])[0]
        kind = data[pos + 4 : pos + 8]
        chunk = data[pos + 8 : pos + 8 + length]
        if kind == b"IHDR":
            width, height, bit_depth, colour_type = struct.unpack(">IIBB", chunk[:10])
            if bit_depth != 8 or colour_type != 6:
                raise ValueError(f"unsupported PNG: bit depth {bit_depth}, colour type {colour_type}")
        elif kind == b"IDAT":
            idat += chunk
        elif kind == b"IEND":
            break
        pos += 8 + length + 4
    if width is None or height is None:
        raise ValueError("no IHDR chunk")
    return DecodedImage(width, height, _unfilter(zlib.decompress(idat), width, height))


def _paeth(a: int, b: int, c: int) -> int:
    """PNG's Paeth predictor: the neighbour (left, above, or above-left) closest to a+b-c."""
    p = a + b - c
    distances = (abs(p - a), abs(p - b), abs(p - c))
    closest = min(distances)
    if distances[0] == closest:
        return a
    if distances[1] == closest:
        return b
    return c


def _unfilter(raw: bytes, width: int, height: int) -> bytes:
    """Reverse PNG's per-scanline filtering (stdlib only - no `image`/`Pillow`)."""
    bpp = 4
    stride = width * bpp
    out = bytearray(width * height * bpp)
    previous = bytearray(stride)
    offset = 0
    for row in range(height):
        filter_type = raw[offset]
        offset += 1
        line = bytearray(raw[offset : offset + stride])
        offset += stride
        for i in range(stride):
            left = line[i - bpp] if i >= bpp else 0
            up = previous[i]
            up_left = previous[i - bpp] if i >= bpp else 0
            match filter_type:
                case 0:
                    pass
                case 1:
                    line[i] = (line[i] + left) & 0xFF
                case 2:
                    line[i] = (line[i] + up) & 0xFF
                case 3:
                    line[i] = (line[i] + (left + up) // 2) & 0xFF
                case 4:
                    line[i] = (line[i] + _paeth(left, up, up_left)) & 0xFF
                case _:
                    raise ValueError(f"unsupported PNG filter type {filter_type}")
        out[row * stride : row * stride + stride] = line
        previous = line
    return bytes(out)


def pixel_at(image: DecodedImage, x: int, y: int) -> tuple[int, int, int, int]:
    """Read one pixel as (r, g, b, a).

    Raises:
        IndexError: `(x, y)` is outside the image.
    """
    if not (0 <= x < image.width and 0 <= y < image.height):
        raise IndexError(f"({x}, {y}) is outside a {image.width}x{image.height} image")
    start = (y * image.width + x) * 4
    r, g, b, a = image.pixels[start : start + 4]
    return (r, g, b, a)


def shows_marker(image: DecodedImage, x: int, y: int, expected: tuple[int, int, int], tolerance: int = 8) -> bool:
    """Say whether pixel `(x, y)` is within `tolerance` of `expected` on every RGB channel.

    A tolerance, not an exact match: `screencapture`'s own colour management rounds a
    few values off a GPUI-rendered window's exact literal (observed during TASK-1.91:
    0xfceee6 captured as (252, 235, 226), a diff of up to 4 per channel).
    """
    r, g, b, _alpha = pixel_at(image, x, y)
    expected_r, expected_g, expected_b = expected
    return abs(r - expected_r) <= tolerance and abs(g - expected_g) <= tolerance and abs(b - expected_b) <= tolerance


def main(argv: list[str] | None = None) -> int:
    """Check one captured image file's pixel against an expected colour.

    `argv` defaults to the real command line (`sys.argv[1:]`); a test passes its own list
    instead, so nothing here has to patch `sys.argv` to exercise the real parsing and the
    real file read together.
    """
    args = sys.argv[1:] if argv is None else argv
    if len(args) != 6:
        print("usage: steg-gallery-check <path> <x> <y> <r> <g> <b>", file=sys.stderr)
        return 2
    path, x, y, r, g, b = args
    with open(path, "rb") as handle:
        data = handle.read()
    image = decode_png_rgba(data)
    expected = (int(r), int(g), int(b))
    if shows_marker(image, int(x), int(y), expected):
        print(f"ok      {path} shows the gallery at ({x},{y})")
        return 0
    actual = pixel_at(image, int(x), int(y))
    print(
        f"steg-gallery-check: {path} pixel ({x},{y}) is {actual[:3]}, not within tolerance of "
        f"{expected} - the gallery window was not what got captured. Most likely cause: the "
        "terminal that ran `just capture-gallery` has never been granted Screen Recording "
        "permission (System Settings -> Privacy & Security -> Screen Recording) - grant it "
        "once and re-run.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
