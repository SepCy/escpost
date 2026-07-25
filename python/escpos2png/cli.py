"""Command-line workflows for rendering and physically calibrating cases."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Sequence

from ._native import render
from .cases import Case, CaseError
from .printers import UsbPrinter, load_usb_printer


def main(argv: Sequence[str] | None = None) -> int:
    parser = _create_parser()
    arguments = parser.parse_args(argv)

    try:
        return arguments.handler(arguments)
    except (CaseError, OSError, RuntimeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


def _render_case(arguments: argparse.Namespace) -> int:
    case = Case.load(arguments.case)
    _announce_case(case)
    _write_rendered_sheets(case, arguments.output_dir)
    return 0


def _print_case(arguments: argparse.Namespace) -> int:
    case = Case.load(arguments.case)
    _announce_case(case)
    _send_to_printer(case, arguments.config, arguments.printer)
    return 0


def _calibrate_case(arguments: argparse.Namespace) -> int:
    case = Case.load(arguments.case)
    _announce_case(case)
    _write_rendered_sheets(case, arguments.output_dir)
    _send_to_printer(case, arguments.config, arguments.printer)
    return 0


def _announce_case(case: Case) -> None:
    print(f"case: {case.directory}")
    print(f"input sha256: {case.input_sha256}")
    print(f"profile: {case.profile}")
    print(f"bytes: {len(case.input_bytes)}")


def _write_rendered_sheets(case: Case, output_dir: str | Path) -> None:
    sheets = render(case.input_bytes, profile=case.profile)
    output_directory = Path(output_dir)
    output_directory.mkdir(parents=True, exist_ok=True)

    for sheet_number, png in enumerate(sheets, start=1):
        output = output_directory / f"actual-{sheet_number:03}.png"
        output.write_bytes(png)
        print(f"wrote: {output}")


def _send_to_printer(case: Case, config: str | Path, printer: str) -> None:
    printer_config = load_usb_printer(config, printer)
    if printer_config.profile != case.profile:
        raise ValueError(
            f"case profile {case.profile!r} does not match printer profile "
            f"{printer_config.profile!r}"
        )

    print(f"printer: {printer_config.name}")
    print(
        f"usb: {printer_config.vendor_id:#06x}:{printer_config.product_id:#06x} "
        f"endpoint {printer_config.out_endpoint:#04x}"
    )

    printer = _open_usb_printer(printer_config)
    try:
        printer._raw(case.input_bytes)
    finally:
        printer.close()


def _open_usb_printer(config: UsbPrinter):
    try:
        from escpos.printer import Usb
    except ImportError as error:
        raise RuntimeError(
            "physical printing requires the escpos2png[printer] extra"
        ) from error

    return Usb(
        config.vendor_id,
        config.product_id,
        out_ep=config.out_endpoint,
        profile=config.profile,
    )


def _create_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="escpos2png")
    commands = parser.add_subparsers(dest="command", required=True)
    case_parser = commands.add_parser("case")
    case_commands = case_parser.add_subparsers(dest="case_command", required=True)

    render_parser = case_commands.add_parser("render")
    render_parser.add_argument("case")
    render_parser.add_argument("--output-dir", required=True)
    render_parser.set_defaults(handler=_render_case)

    print_parser = case_commands.add_parser("print")
    print_parser.add_argument("case")
    _add_printer_arguments(print_parser)
    print_parser.set_defaults(handler=_print_case)

    calibrate_parser = case_commands.add_parser("calibrate")
    calibrate_parser.add_argument("case")
    calibrate_parser.add_argument("--output-dir", required=True)
    _add_printer_arguments(calibrate_parser)
    calibrate_parser.set_defaults(handler=_calibrate_case)
    return parser


def _add_printer_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--printer", required=True)
    parser.add_argument("--config", default="local/printers.toml")
