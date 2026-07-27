import json
import struct
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

from escpos2png import render, render_result
from escpos2png.cases import Case, CaseError
from escpos2png.cli import main


REPOSITORY = Path(__file__).parents[2]
CASE_DIRECTORY = (
    REPOSITORY
    / "tests"
    / "cases"
    / "graphics"
    / "esc-star-8dot-double-density"
)
CALIBRATION_INPUT = REPOSITORY / "calibration" / "input.hex"


class RenderBindingTest(unittest.TestCase):
    def test_render_returns_complete_png_sheets(self):
        data = bytes.fromhex((CASE_DIRECTORY / "input.hex").read_text())

        sheets = render(data, profile="NT-5890K")

        self.assertEqual(len(sheets), 1)
        self.assertIsInstance(sheets[0], bytes)
        self.assertEqual(_read_png_header(sheets[0]), (384, 30, 1, 0))

    def test_render_result_preserves_approximations_and_rendering_identity(self):
        rendered = render_result(b"\n", profile="NT-5890K")

        self.assertEqual(len(rendered["sheets"]), 1)
        self.assertEqual(rendered["device_events"], [])
        self.assertTrue(
            any(
                approximation["field"] == "fonts.resident_glyph_shapes"
                for approximation in rendered["approximations"]
            )
        )
        self.assertEqual(rendered["metadata"]["profile_id"], "NT-5890K")
        self.assertEqual(
            len(rendered["metadata"]["canonical_profile_sha256"]),
            64,
        )


class CaseRenderCliTest(unittest.TestCase):
    def test_case_render_loads_input_and_writes_png(self):
        stdout = StringIO()
        with TemporaryDirectory() as output_directory:
            with redirect_stdout(stdout):
                exit_code = main(
                    [
                        "case",
                        "render",
                        str(CASE_DIRECTORY),
                        "--output-dir",
                        output_directory,
                    ]
                )

            output = Path(output_directory) / "actual-001.png"
            self.assertEqual(exit_code, 0)
            self.assertEqual(_read_png_header(output.read_bytes()), (384, 30, 1, 0))
            self.assertEqual(
                json.loads((Path(output_directory) / "manifest.json").read_text()),
                {"sheets": ["actual-001.png"]},
            )
            self.assertNotIn("input sha256", stdout.getvalue())

    def test_case_render_writes_all_sheets_in_preview_order(self):
        rendered = {
            "sheets": [b"first sheet", b"second sheet"],
            "metadata": {"canonical_profile_sha256": "0" * 64},
        }
        with TemporaryDirectory() as output_directory:
            with patch("escpos2png.cli.render_result", return_value=rendered):
                with redirect_stdout(StringIO()):
                    exit_code = main(
                        [
                            "case",
                            "render",
                            str(CASE_DIRECTORY),
                            "--output-dir",
                            output_directory,
                        ]
                    )

            output_directory = Path(output_directory)
            self.assertEqual(exit_code, 0)
            self.assertEqual(
                json.loads((output_directory / "manifest.json").read_text()),
                {"sheets": ["actual-001.png", "actual-002.png"]},
            )
            self.assertEqual(
                (output_directory / "actual-001.png").read_bytes(),
                b"first sheet",
            )
            self.assertEqual(
                (output_directory / "actual-002.png").read_bytes(),
                b"second sheet",
            )


class CaseLoaderTest(unittest.TestCase):
    def test_case_loader_accepts_a_versioned_fixture_without_a_duplicate_hash(self):
        with TemporaryDirectory() as case_directory:
            case_directory = Path(case_directory)
            (case_directory / "input.hex").write_text("1b 40")
            (case_directory / "case.toml").write_text(
                """
schema_version = 1
name = "Minimal case"
profile = "NT-5890K"
""".strip()
            )

            case = Case.load(case_directory)

        self.assertEqual(case.profile, "NT-5890K")
        self.assertEqual(case.input_bytes, b"\x1b@")

    def test_case_loader_rejects_the_retired_input_hash_field(self):
        with TemporaryDirectory() as case_directory:
            case_directory = Path(case_directory)
            (case_directory / "input.hex").write_text("1b 40")
            (case_directory / "case.toml").write_text(
                """
schema_version = 1
name = "Old case"
profile = "NT-5890K"
input_sha256 = "0fcb71d8b3b3b965f4d75d20e8d4bca56c4d13a44de0a9ac2899181d8d9b7abf"
""".strip()
            )

            with self.assertRaisesRegex(CaseError, "unknown case field 'input_sha256'"):
                Case.load(case_directory)


class CasePrintCliTest(unittest.TestCase):
    def test_case_print_sends_the_input_bytes_unchanged(self):
        printer = FakePrinter()
        stdout = StringIO()
        with TemporaryDirectory() as local_directory:
            config = Path(local_directory) / "printers.toml"
            config.write_text(
                """
[netum-usb]
transport = "usb"
profile = "NT-5890K"
vendor_id = "0x1234"
product_id = "0x5678"
interface_number = 0
out_endpoint = "0x01"
in_endpoint = "0x81"
""".strip()
            )

            with (
                patch(
                    "escpos2png.cli._open_usb_printer",
                    return_value=printer,
                ),
                redirect_stdout(stdout),
            ):
                exit_code = main(
                    [
                        "case",
                        "print",
                        str(CASE_DIRECTORY),
                        "--printer",
                        "netum-usb",
                        "--config",
                        str(config),
                    ]
                )

        expected = bytes.fromhex((CASE_DIRECTORY / "input.hex").read_text())
        self.assertEqual(exit_code, 0)
        self.assertEqual(printer.writes, [expected])
        self.assertTrue(printer.closed)
        self.assertIn(f"bytes: {len(expected)}", stdout.getvalue())

    def test_case_calibrate_renders_and_prints_one_loaded_stream(self):
        printer = FakePrinter()
        with TemporaryDirectory() as local_directory:
            local_directory = Path(local_directory)
            config = local_directory / "printers.toml"
            config.write_text(
                """
[netum-usb]
transport = "usb"
profile = "NT-5890K"
vendor_id = "0x1234"
product_id = "0x5678"
interface_number = 0
out_endpoint = "0x01"
in_endpoint = "0x81"
""".strip()
            )
            output_directory = local_directory / "rendered"

            with (
                patch(
                    "escpos2png.cli._open_usb_printer",
                    return_value=printer,
                ),
                redirect_stdout(StringIO()),
            ):
                exit_code = main(
                    [
                        "case",
                        "calibrate",
                        str(CASE_DIRECTORY),
                        "--printer",
                        "netum-usb",
                        "--config",
                        str(config),
                        "--output-dir",
                        str(output_directory),
                    ]
                )

            rendered = (output_directory / "actual-001.png").read_bytes()

        expected = bytes.fromhex((CASE_DIRECTORY / "input.hex").read_text())
        self.assertEqual(exit_code, 0)
        self.assertEqual(_read_png_header(rendered), (384, 30, 1, 0))
        self.assertEqual(printer.writes, [expected])


class CalibrationCliTest(unittest.TestCase):
    def test_calibration_render_uses_the_shared_stream_and_selected_profile(self):
        with TemporaryDirectory() as output_directory:
            with redirect_stdout(StringIO()):
                exit_code = main(
                    [
                        "calibration",
                        "render",
                        "NT-5890K",
                        "--output-dir",
                        output_directory,
                    ]
                )

            output = Path(output_directory) / "actual-001.png"
            self.assertEqual(exit_code, 0)
            self.assertEqual(
                _read_png_header(output.read_bytes()),
                (384, 1632, 1, 0),
            )

    def test_calibration_calibrate_infers_the_profile_from_the_printer(self):
        printer = FakePrinter()
        with TemporaryDirectory() as local_directory:
            local_directory = Path(local_directory)
            config = local_directory / "printers.toml"
            config.write_text(
                """
[netum-usb]
transport = "usb"
profile = "NT-5890K"
vendor_id = "0x1234"
product_id = "0x5678"
interface_number = 0
out_endpoint = "0x01"
in_endpoint = "0x81"
""".strip()
            )
            output_directory = local_directory / "rendered"

            with (
                patch(
                    "escpos2png.cli._open_usb_printer",
                    return_value=printer,
                ),
                redirect_stdout(StringIO()),
            ):
                exit_code = main(
                    [
                        "calibration",
                        "calibrate",
                        "--printer",
                        "netum-usb",
                        "--config",
                        str(config),
                        "--output-dir",
                        str(output_directory),
                    ]
                )

        expected = bytes.fromhex(CALIBRATION_INPUT.read_text())
        self.assertEqual(exit_code, 0)
        self.assertEqual(printer.writes, [expected])
        self.assertTrue(printer.closed)


class FakePrinter:
    def __init__(self):
        self.writes = []
        self.closed = False

    def _raw(self, data):
        self.writes.append(data)

    def close(self):
        self.closed = True


def _read_png_header(png):
    if png[:8] != b"\x89PNG\r\n\x1a\n":
        raise AssertionError("render result is not a PNG")

    length, chunk_type = struct.unpack(">I4s", png[8:16])
    if length != 13 or chunk_type != b"IHDR":
        raise AssertionError("PNG does not begin with an IHDR chunk")

    width, height, bit_depth, color_type, _, _, _ = struct.unpack(
        ">IIBBBBB", png[16:29]
    )
    return width, height, bit_depth, color_type


if __name__ == "__main__":
    unittest.main()
