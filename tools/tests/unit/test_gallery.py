"""Decoding a captured gallery PNG and checking it against the colour it should show."""

import pytest
from conftest import build_rgba_png

from steg_tools.gallery import DecodedImage, decode_png_rgba, pixel_at, shows_marker

RED = (255, 0, 0, 255)
BLUE = (0, 0, 255, 255)


def two_by_one_png() -> bytes:
    """A 2x1 image: red at (0,0), blue at (1,0) - built independently of the decoder."""
    return build_rgba_png(2, 1, bytes(RED) + bytes(BLUE))


def test_decoding_a_png_this_module_did_not_write_recovers_its_pixels() -> None:
    image = decode_png_rgba(two_by_one_png())
    assert (image.width, image.height) == (2, 1)
    assert pixel_at(image, 0, 0) == RED
    assert pixel_at(image, 1, 0) == BLUE


def test_a_png_that_is_not_a_png_is_refused() -> None:
    with pytest.raises(ValueError, match="not a PNG"):
        decode_png_rgba(b"not a png at all")


def test_reading_outside_the_image_is_refused() -> None:
    image = decode_png_rgba(two_by_one_png())
    with pytest.raises(IndexError):
        pixel_at(image, 2, 0)


def test_shows_marker_is_true_within_tolerance() -> None:
    # Observed during TASK-1.91: screencapture's own colour management rounded 0xfceee6
    # to (252, 235, 226) - a diff of up to 4 per channel. Within the 8-channel default
    # tolerance, that capture is still a match.
    image = DecodedImage(1, 1, bytes((252, 235, 226, 255)))
    assert shows_marker(image, 0, 0, (252, 238, 230))


def test_shows_marker_is_false_for_the_wrong_colour() -> None:
    # The negative control the mechanism needs: a capture of the desktop background
    # (TASK-1.91's own throwaway experiment saw (36, 39, 45)) must not pass as the
    # gallery's red placeholder.
    image = DecodedImage(1, 1, bytes((36, 39, 45, 255)))
    assert not shows_marker(image, 0, 0, (255, 0, 0))


def test_shows_marker_tolerance_has_an_edge() -> None:
    image = DecodedImage(1, 1, bytes((108, 100, 100, 255)))
    assert shows_marker(image, 0, 0, (100, 100, 100), tolerance=8)
    assert not shows_marker(image, 0, 0, (100, 100, 100), tolerance=7)
