import os
import stat
import unittest
from contextlib import redirect_stderr, redirect_stdout
from dataclasses import replace
from io import StringIO
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

from escpost.cli import main
from escpost.printers import (
    DiscoveredUsbPrinter,
    default_printers_config_path,
    load_usb_printer,
    save_usb_printer,
)


NETUM_PRINTER = DiscoveredUsbPrinter(
    vendor_id=0x0416,
    product_id=0x5011,
    bus=3,
    address=15,
    interface_number=0,
    out_endpoint=0x01,
    in_endpoint=0x81,
    manufacturer="YICHIP3121",
    product="USB Portable Printer",
    serial_number="B120300001",
)


class PrinterDiscoveryCliTest(unittest.TestCase):
    def test_discovery_lists_printer_without_writing_configuration(self):
        stdout = StringIO()

        with (
            patch(
                "escpost.cli.discover_usb_printers",
                return_value=[NETUM_PRINTER],
            ),
            redirect_stdout(stdout),
        ):
            exit_code = main(["printers", "discover"])

        self.assertEqual(exit_code, 0)
        self.assertIn("USB Portable Printer", stdout.getvalue())
        self.assertIn("0416:5011", stdout.getvalue())
        self.assertIn("out 0x01", stdout.getvalue())
        self.assertIn("serial: B120300001", stdout.getvalue())

    def test_discovery_writes_the_single_selected_printer(self):
        with TemporaryDirectory() as local_directory:
            config = Path(local_directory) / "printers.toml"

            with (
                patch(
                    "escpost.cli.discover_usb_printers",
                    return_value=[NETUM_PRINTER],
                ),
                redirect_stdout(StringIO()),
            ):
                exit_code = main(
                    [
                        "printers",
                        "discover",
                        "--name",
                        "netum-usb",
                        "--profile",
                        "NT-5890K",
                        "--vendor-id",
                        "0x0416",
                        "--product-id",
                        "0x5011",
                        "--serial",
                        "B120300001",
                        "--config",
                        str(config),
                    ]
                )

            saved = load_usb_printer(config, "netum-usb")

        self.assertEqual(exit_code, 0)
        self.assertEqual(saved.vendor_id, 0x0416)
        self.assertEqual(saved.product_id, 0x5011)
        self.assertEqual(saved.interface_number, 0)
        self.assertEqual(saved.out_endpoint, 0x01)
        self.assertEqual(saved.in_endpoint, 0x81)
        self.assertEqual(saved.serial_number, "B120300001")

    def test_discovery_requires_filters_when_multiple_printers_match(self):
        second_printer = replace(
            NETUM_PRINTER,
            address=16,
            serial_number="B120300002",
        )
        stderr = StringIO()

        with (
            patch(
                "escpost.cli.discover_usb_printers",
                return_value=[NETUM_PRINTER, second_printer],
            ),
            redirect_stdout(StringIO()),
            redirect_stderr(stderr),
        ):
            exit_code = main(
                [
                    "printers",
                    "discover",
                    "--name",
                    "netum-usb",
                    "--profile",
                    "NT-5890K",
                ]
            )

        self.assertEqual(exit_code, 2)
        self.assertIn("multiple USB printers match", stderr.getvalue())
        self.assertIn("--serial", stderr.getvalue())


class PrinterConfigTest(unittest.TestCase):
    def test_config_directory_override_is_shared_with_the_native_cli(self):
        with (
            TemporaryDirectory() as config_directory,
            patch.dict(
                "os.environ",
                {"ESCPOST_CONFIG_DIR": config_directory},
                clear=False,
            ),
        ):
            path = default_printers_config_path()

        self.assertEqual(path, Path(config_directory) / "printers.toml")

    def test_linux_default_respects_xdg_config_home(self):
        with (
            TemporaryDirectory() as config_directory,
            patch("escpost.printers.sys.platform", "linux"),
            patch("escpost.printers.os.name", "posix"),
            patch.dict(
                "os.environ",
                {
                    "ESCPOST_CONFIG_DIR": "",
                    "XDG_CONFIG_HOME": config_directory,
                },
                clear=False,
            ),
        ):
            path = default_printers_config_path()

        self.assertEqual(
            path,
            Path(config_directory) / "escpost" / "printers.toml",
        )

    @unittest.skipUnless(os.name == "posix", "Unix file modes apply")
    def test_first_save_creates_private_configuration(self):
        with TemporaryDirectory() as local_directory:
            config = Path(local_directory) / "nested" / "printers.toml"

            save_usb_printer(config, "netum-usb", "NT-5890K", NETUM_PRINTER)

            mode = stat.S_IMODE(config.stat().st_mode)

        self.assertEqual(mode, 0o600)

    def test_load_accepts_a_genuinely_unidirectional_printer(self):
        with TemporaryDirectory() as local_directory:
            config = Path(local_directory) / "printers.toml"
            config.write_text(
                """
[one-way-printer]
transport = "usb"
profile = "ONE-WAY"
vendor_id = "0x1234"
product_id = "0x5678"
interface_number = 0
out_endpoint = "0x01"
""".strip()
            )

            loaded = load_usb_printer(config, "one-way-printer")

        self.assertIsNone(loaded.in_endpoint)

    def test_save_updates_one_table_and_preserves_existing_content(self):
        with TemporaryDirectory() as local_directory:
            config = Path(local_directory) / "printers.toml"
            config.write_text(
                """
# Keep this developer note.

[other-printer]
transport = "usb"
profile = "OTHER"
vendor_id = "0x1234"
product_id = "0x5678"
interface_number = 0
out_endpoint = "0x01"
""".lstrip()
            )

            save_usb_printer(config, "netum-usb", "NT-5890K", NETUM_PRINTER)

            updated = config.read_text()
            loaded = load_usb_printer(config, "netum-usb")

        self.assertIn("# Keep this developer note.", updated)
        self.assertIn("[other-printer]", updated)
        self.assertEqual(loaded.profile, "NT-5890K")
        self.assertEqual(loaded.serial_number, "B120300001")


if __name__ == "__main__":
    unittest.main()
