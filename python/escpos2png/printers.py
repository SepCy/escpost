"""Machine-local physical-printer configuration."""

from __future__ import annotations

import tomllib
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class UsbPrinter:
    name: str
    profile: str
    vendor_id: int
    product_id: int
    out_endpoint: int


class PrinterConfigError(ValueError):
    """The local physical-printer configuration is invalid."""


def load_usb_printer(path: str | Path, name: str) -> UsbPrinter:
    config = _load_config(Path(path))
    printer = config.get(name)
    if not isinstance(printer, dict):
        raise PrinterConfigError(f"printer {name!r} does not exist in {path}")
    if printer.get("transport") != "usb":
        raise PrinterConfigError(f"printer {name!r} does not use the USB transport")

    return UsbPrinter(
        name=name,
        profile=_require_string(printer, "profile", name),
        vendor_id=_require_usb_integer(printer, "vendor_id", name, maximum=0xFFFF),
        product_id=_require_usb_integer(printer, "product_id", name, maximum=0xFFFF),
        out_endpoint=_require_usb_integer(
            printer, "out_endpoint", name, maximum=0xFF
        ),
    )


def _load_config(path: Path) -> dict:
    try:
        with path.open("rb") as config_file:
            return tomllib.load(config_file)
    except FileNotFoundError as error:
        raise PrinterConfigError(f"printer configuration does not exist: {path}") from error
    except tomllib.TOMLDecodeError as error:
        raise PrinterConfigError(f"invalid printer configuration {path}: {error}") from error


def _require_string(config: dict, field: str, printer: str) -> str:
    value = config.get(field)
    if not isinstance(value, str) or not value:
        raise PrinterConfigError(
            f"printer {printer!r} field {field!r} must be a non-empty string"
        )
    return value


def _require_usb_integer(
    config: dict, field: str, printer: str, *, maximum: int
) -> int:
    value = config.get(field)
    try:
        parsed = int(value, 0) if isinstance(value, str) else value
    except ValueError as error:
        raise PrinterConfigError(
            f"printer {printer!r} field {field!r} is not an integer"
        ) from error

    if isinstance(parsed, bool) or not isinstance(parsed, int) or not 0 <= parsed <= maximum:
        raise PrinterConfigError(
            f"printer {printer!r} field {field!r} must be between 0 and {maximum:#x}"
        )
    return parsed
