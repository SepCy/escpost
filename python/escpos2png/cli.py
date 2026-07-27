"""Command-line workflows for rendering and physically calibrating cases."""

from __future__ import annotations

from pathlib import Path
from typing import Sequence

import click

from ._native import render_result
from .cases import Case, CaseError
from .printers import (
    DiscoveredUsbPrinter,
    UsbPrinter,
    discover_usb_printers,
    load_usb_printer,
    save_usb_printer,
)
from .qualification import Qualification


LOCAL_CONFIG = Path("local/printers.toml")
CASE_PATH = click.Path(path_type=Path, file_okay=False)
DIRECTORY_PATH = click.Path(path_type=Path, file_okay=False)
CONFIG_PATH = click.Path(path_type=Path, dir_okay=False)


def main(argv: Sequence[str] | None = None) -> int:
    try:
        result = _cli.main(
            args=list(argv) if argv is not None else None,
            prog_name="escpos2png",
            standalone_mode=False,
        )
    except click.ClickException as error:
        error.show()
        return error.exit_code
    except (CaseError, OSError, RuntimeError, ValueError) as error:
        click.echo(f"error: {error}", err=True)
        return 2

    return result if isinstance(result, int) else 0


def _parse_usb_id(
    _context: click.Context,
    _parameter: click.Parameter,
    value: str | None,
) -> int | None:
    if value is None:
        return None

    try:
        parsed = int(value, 0)
    except ValueError as error:
        raise click.BadParameter(
            "must be an integer or 0x-prefixed hex"
        ) from error

    if not 0 <= parsed <= 0xFFFF:
        raise click.BadParameter("must be between 0x0000 and 0xffff")
    return parsed


def _printer_options(function):
    function = click.option(
        "--config",
        type=CONFIG_PATH,
        default=LOCAL_CONFIG,
        show_default=True,
        help="local printer configuration",
    )(function)
    return click.option(
        "--printer",
        required=True,
        help="local printer name",
    )(function)


@click.group()
def _cli() -> None:
    """Render ESC/POS receipts and calibrate them against physical printers."""


@_cli.group("printers")
def _printer_commands() -> None:
    """Discover and configure physical printers."""


@_printer_commands.command("discover")
@click.option(
    "--name",
    help="local printer name to write",
)
@click.option(
    "--profile",
    help="renderer profile assigned to the printer",
)
@click.option(
    "--vendor-id",
    callback=_parse_usb_id,
    help="filter by decimal or 0x-prefixed USB vendor ID",
)
@click.option(
    "--product-id",
    callback=_parse_usb_id,
    help="filter by decimal or 0x-prefixed USB product ID",
)
@click.option(
    "--serial",
    help="filter by exact USB serial number",
)
@click.option(
    "--config",
    type=CONFIG_PATH,
    default=LOCAL_CONFIG,
    show_default=True,
    help="local configuration to update",
)
def _discover_printers(
    name: str | None,
    profile: str | None,
    vendor_id: int | None,
    product_id: int | None,
    serial: str | None,
    config: Path,
) -> None:
    """List USB printers, optionally saving the single matching device."""
    printers = [
        printer
        for printer in discover_usb_printers()
        if _matches_discovery_filters(
            printer,
            vendor_id=vendor_id,
            product_id=product_id,
            serial=serial,
        )
    ]
    if not printers:
        raise RuntimeError("no matching USB printer-class devices found")

    for number, printer in enumerate(printers, start=1):
        _announce_discovered_printer(number, printer)

    if name is None and profile is None:
        return
    if name is None or profile is None:
        raise ValueError("--name and --profile must be provided together")
    if len(printers) != 1:
        raise ValueError(
            "multiple USB printers match; select one with "
            "--vendor-id, --product-id, or --serial"
        )

    save_usb_printer(config, name, profile, printers[0])
    click.echo(f"saved: {config} [{name}]")


@_cli.group("case")
def _case_commands() -> None:
    """Render and physically test conformance cases."""


@_case_commands.command("render")
@click.argument("case_directory", type=CASE_PATH)
@click.option(
    "--output-dir",
    type=DIRECTORY_PATH,
    required=True,
)
def _render_case(case_directory: Path, output_dir: Path) -> None:
    """Render a conformance case to PNG."""
    case = Case.load(case_directory)
    _announce_case(case)
    _write_rendered_sheets(case.input_bytes, case.profile, output_dir)


@_case_commands.command("print")
@click.argument("case_directory", type=CASE_PATH)
@_printer_options
def _print_case(case_directory: Path, printer: str, config: Path) -> None:
    """Send a conformance case's bytes to a printer."""
    case = Case.load(case_directory)
    _announce_case(case)
    _send_to_printer(case.input_bytes, case.profile, config, printer)


@_case_commands.command("calibrate")
@click.argument("case_directory", type=CASE_PATH)
@click.option(
    "--output-dir",
    type=DIRECTORY_PATH,
    required=True,
)
@_printer_options
def _calibrate_case(
    case_directory: Path,
    output_dir: Path,
    printer: str,
    config: Path,
) -> None:
    """Render and print the same conformance-case bytes."""
    case = Case.load(case_directory)
    _announce_case(case)
    _write_rendered_sheets(case.input_bytes, case.profile, output_dir)
    _send_to_printer(case.input_bytes, case.profile, config, printer)


@_cli.group("qualification")
def _qualification_commands() -> None:
    """Render and physically test the shared printer qualification receipt."""


@_qualification_commands.command("render")
@click.argument("profile")
@click.option(
    "--output-dir",
    type=DIRECTORY_PATH,
    required=True,
)
def _render_qualification(profile: str, output_dir: Path) -> None:
    """Render the shared receipt with one printer profile."""
    qualification = Qualification.load(Path.cwd(), profile)
    _announce_qualification(qualification)
    _write_rendered_sheets(
        qualification.input_bytes,
        qualification.profile,
        output_dir,
    )


@_qualification_commands.command("print")
@_printer_options
def _print_qualification(printer: str, config: Path) -> None:
    """Send the shared receipt using the configured printer profile."""
    qualification = _load_printer_qualification(config, printer)
    _announce_qualification(qualification)
    _send_to_printer(
        qualification.input_bytes,
        qualification.profile,
        config,
        printer,
    )


@_qualification_commands.command("calibrate")
@click.option(
    "--output-dir",
    type=DIRECTORY_PATH,
    required=True,
)
@_printer_options
def _calibrate_qualification(
    output_dir: Path,
    printer: str,
    config: Path,
) -> None:
    """Render and print the shared receipt for one configured printer."""
    qualification = _load_printer_qualification(config, printer)
    _announce_qualification(qualification)
    _write_rendered_sheets(
        qualification.input_bytes,
        qualification.profile,
        output_dir,
    )
    _send_to_printer(
        qualification.input_bytes,
        qualification.profile,
        config,
        printer,
    )


def _matches_discovery_filters(
    printer: DiscoveredUsbPrinter,
    *,
    vendor_id: int | None,
    product_id: int | None,
    serial: str | None,
) -> bool:
    return (
        (vendor_id is None or printer.vendor_id == vendor_id)
        and (product_id is None or printer.product_id == product_id)
        and (serial is None or printer.serial_number == serial)
    )


def _announce_discovered_printer(
    number: int,
    printer: DiscoveredUsbPrinter,
) -> None:
    product = printer.product or "USB printer"
    manufacturer = (
        f" ({printer.manufacturer})" if printer.manufacturer is not None else ""
    )
    location = (
        f"bus {printer.bus} address {printer.address}"
        if printer.bus is not None and printer.address is not None
        else "location unavailable"
    )

    click.echo(f"[{number}] {product}{manufacturer}")
    click.echo(
        f"    usb: {printer.vendor_id:04x}:{printer.product_id:04x}; "
        f"{location}; interface {printer.interface_number}"
    )
    endpoints = f"    endpoints: out {printer.out_endpoint:#04x}"
    if printer.in_endpoint is not None:
        endpoints += f"; in {printer.in_endpoint:#04x}"
    click.echo(endpoints)
    if printer.serial_number is not None:
        click.echo(f"    serial: {printer.serial_number}")


def _announce_case(case: Case) -> None:
    click.echo(f"case: {case.directory}")
    click.echo(f"profile: {case.profile}")
    click.echo(f"bytes: {len(case.input_bytes)}")


def _announce_qualification(qualification: Qualification) -> None:
    click.echo(f"qualification input: {qualification.input_path}")
    click.echo(f"profile: {qualification.profile}")
    click.echo(f"bytes: {len(qualification.input_bytes)}")


def _load_printer_qualification(config: Path, printer: str) -> Qualification:
    printer_config = load_usb_printer(config, printer)
    return Qualification.load(Path.cwd(), printer_config.profile)


def _write_rendered_sheets(
    input_bytes: bytes,
    profile: str,
    output_directory: Path,
) -> None:
    rendered = render_result(input_bytes, profile=profile)
    output_directory.mkdir(parents=True, exist_ok=True)

    for sheet_number, png in enumerate(rendered["sheets"], start=1):
        output = output_directory / f"actual-{sheet_number:03}.png"
        output.write_bytes(png)
        click.echo(f"wrote: {output}")
    click.echo(
        "canonical profile sha256: "
        f"{rendered['metadata']['canonical_profile_sha256']}"
    )


def _send_to_printer(
    input_bytes: bytes,
    profile: str,
    config: Path,
    printer: str,
) -> None:
    printer_config = load_usb_printer(config, printer)
    if printer_config.profile != profile:
        raise ValueError(
            f"input profile {profile!r} does not match printer profile "
            f"{printer_config.profile!r}"
        )

    click.echo(f"printer: {printer_config.name}")
    click.echo(
        f"usb: {printer_config.vendor_id:#06x}:{printer_config.product_id:#06x} "
        f"out {printer_config.out_endpoint:#04x}"
    )
    if printer_config.in_endpoint is not None:
        click.echo(f"usb in: {printer_config.in_endpoint:#04x}")

    physical_printer = _open_usb_printer(printer_config)
    try:
        physical_printer._raw(input_bytes)
    finally:
        physical_printer.close()


def _open_usb_printer(config: UsbPrinter):
    try:
        from escpos.printer import Usb
    except ImportError as error:
        raise RuntimeError(
            "physical printing requires the escpos2png[printer] extra"
        ) from error

    usb_args = {}
    if config.serial_number is not None:
        usb_args["custom_match"] = _usb_serial_matcher(config.serial_number)
    if config.interface_number != 0:
        raise RuntimeError(
            "the python-escpos USB adapter currently supports interface 0 only"
        )

    options = {
        "usb_args": usb_args,
        "out_ep": config.out_endpoint,
        "profile": config.profile,
    }
    if config.in_endpoint is not None:
        options["in_ep"] = config.in_endpoint

    return Usb(config.vendor_id, config.product_id, **options)


def _usb_serial_matcher(serial_number: str):
    def matches(device) -> bool:
        import usb.util

        try:
            return (
                usb.util.get_string(device, device.iSerialNumber) == serial_number
            )
        except (OSError, ValueError):
            return False

    return matches
