"""`steg-gallery-check`'s CLI behaviour: a real file on disk, the one real boundary it has."""

from pathlib import Path

from conftest import build_rgba_png

from steg_tools.gallery import main

RED = (255, 0, 0, 255)


def write_png(path: Path, colour: tuple[int, int, int, int]) -> None:
    path.write_bytes(build_rgba_png(1, 1, bytes(colour)))


def test_a_capture_that_shows_the_expected_colour_passes(tmp_path: Path) -> None:
    path = tmp_path / "light.png"
    write_png(path, RED)
    assert main([str(path), "0", "0", "255", "0", "0"]) == 0


def test_a_capture_of_the_wrong_thing_fails_and_names_screen_recording(tmp_path: Path, capsys: object) -> None:
    # The negative control the recipe needs (team-lead's review of this plan): a capture
    # that shows the desktop, not the gallery, must fail loudly, not attach a blank image
    # to a pull request.
    path = tmp_path / "wrong.png"
    write_png(path, (36, 39, 45, 255))
    assert main([str(path), "0", "0", "255", "0", "0"]) == 1
    captured = capsys.readouterr()  # type: ignore[attr-defined]
    assert "Screen Recording" in captured.err
