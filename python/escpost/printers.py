"""USB printer discovery and machine-local configuration."""

from __future__ import annotations

import os
import stat
import sys
import tempfile
import tomllib
from dataclasses import dataclass
from pathlib import Path


USB_CLASS_PRINTER = 0x07


@dataclass(frozen=True)
class UsbPrinter:
    name: str
    profile: str
    vendor_id: int
    product_id: int
    serial_number: str | None
    interface_number: int
    out_endpoint: int
    in_endpoint: int | None


@dataclass(frozen=True)
class DiscoveredUsbPrinter:
    vendor_id: int
    product_id: int
    bus: int | None
    address: int | None
    interface_number: int
    out_endpoint: int
    in_endpoint: int | None
    manufacturer: str | None
    product: str | None
    serial_number: str | None


class PrinterConfigError(ValueError):
    """The local physical-printer configuration is invalid."""


def default_printers_config_path() -> Path:
    """Return the legacy CLI's copy of the native configuration location.

    The Python hardware commands are temporary, but they must read the same
    file as the Rust CLI while calibration is migrated. Docker always supplies
    the override, avoiding duplicated platform policy in normal development.
    """
    override = os.environ.get("ESCPOST_CONFIG_DIR")
    if override:
        return Path(override) / "printers.toml"

    if sys.platform == "darwin":
        directory = (
            Path.home()
            / "Library"
            / "Application Support"
            / "io.receiptful.escpost"
        )
    elif os.name == "nt":
        roaming = os.environ.get("APPDATA")
        directory = (
            Path(roaming)
            if roaming
            else Path.home() / "AppData" / "Roaming"
        ) / "receiptful" / "escpost" / "config"
    else:
        xdg_directory = os.environ.get("XDG_CONFIG_HOME")
        directory = (
            Path(xdg_directory)
            if xdg_directory
            else Path.home() / ".config"
        ) / "escpost"

    return directory / "printers.toml"


def discover_usb_printers() -> list[DiscoveredUsbPrinter]:
    usb_core, usb_util = _load_usb_modules()
    printers = []

    for device in usb_core.find(find_all=True):
        printers.extend(_discover_device_printers(device, usb_core, usb_util))

    return sorted(
        printers,
        key=lambda printer: (
            printer.bus is None,
            printer.bus or 0,
            printer.address is None,
            printer.address or 0,
            printer.interface_number,
        ),
    )


def save_usb_printer(
    path: str | Path,
    name: str,
    profile: str,
    printer: DiscoveredUsbPrinter,
) -> None:
    tomlkit = _load_tomlkit()
    config_path = Path(path)
    document = _load_editable_config(config_path, tomlkit)
    table = tomlkit.table()
    table.add("transport", "usb")
    table.add("profile", profile)
    table.add("vendor_id", f"0x{printer.vendor_id:04x}")
    table.add("product_id", f"0x{printer.product_id:04x}")
    if printer.serial_number is not None:
        table.add("serial_number", printer.serial_number)
    table.add("interface_number", printer.interface_number)
    table.add("out_endpoint", f"0x{printer.out_endpoint:02x}")
    if printer.in_endpoint is not None:
        table.add("in_endpoint", f"0x{printer.in_endpoint:02x}")
    document[name] = table

    _write_config_atomically(config_path, tomlkit.dumps(document))


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
        serial_number=_optional_string(printer, "serial_number", name),
        interface_number=_require_usb_integer(
            printer, "interface_number", name, maximum=0xFF
        ),
        out_endpoint=_require_usb_integer(
            printer, "out_endpoint", name, maximum=0xFF
        ),
        in_endpoint=_optional_usb_integer(
            printer, "in_endpoint", name, maximum=0xFF
        ),
    )


def _discover_device_printers(device, usb_core, usb_util):
    interfaces = []
    for configuration in device:
        for interface in configuration:
            if interface.bInterfaceClass != USB_CLASS_PRINTER:
                continue

            out_endpoints = _bulk_endpoints(
                interface, usb_util.ENDPOINT_OUT, usb_util
            )
            in_endpoints = _bulk_endpoints(
                interface, usb_util.ENDPOINT_IN, usb_util
            )
            if out_endpoints:
                interfaces.append(
                    (
                        interface.bInterfaceNumber,
                        out_endpoints[0],
                        in_endpoints[0] if in_endpoints else None,
                    )
                )

    if not interfaces:
        return []

    identity = {
        "vendor_id": device.idVendor,
        "product_id": device.idProduct,
        "bus": device.bus,
        "address": device.address,
        "manufacturer": _read_usb_string(
            device, device.iManufacturer, usb_core, usb_util
        ),
        "product": _read_usb_string(device, device.iProduct, usb_core, usb_util),
        "serial_number": _read_usb_string(
            device, device.iSerialNumber, usb_core, usb_util
        ),
    }
    printers = [
        DiscoveredUsbPrinter(
            **identity,
            interface_number=interface_number,
            out_endpoint=out_endpoint,
            in_endpoint=in_endpoint,
        )
        for interface_number, out_endpoint, in_endpoint in interfaces
    ]

    usb_util.dispose_resources(device)
    return printers


def _bulk_endpoints(interface, direction, usb_util):
    return sorted(
        endpoint.bEndpointAddress
        for endpoint in interface
        if usb_util.endpoint_type(endpoint.bmAttributes)
        == usb_util.ENDPOINT_TYPE_BULK
        and usb_util.endpoint_direction(endpoint.bEndpointAddress) == direction
    )


def _read_usb_string(device, index, usb_core, usb_util):
    if not index:
        return None

    try:
        return usb_util.get_string(device, index)
    except (usb_core.USBError, ValueError):
        return None


def _load_usb_modules():
    try:
        import usb.core
        import usb.util
    except ImportError as error:
        raise RuntimeError(
            "USB discovery requires the escpost[printer] extra"
        ) from error

    return usb.core, usb.util


def _load_tomlkit():
    try:
        import tomlkit
    except ImportError as error:
        raise RuntimeError(
            "saving printer configuration requires the escpost[printer] extra"
        ) from error

    return tomlkit


def _load_editable_config(path: Path, tomlkit):
    if not path.exists():
        return tomlkit.document()

    try:
        return tomlkit.parse(path.read_text())
    except (OSError, tomlkit.exceptions.ParseError) as error:
        raise PrinterConfigError(
            f"invalid printer configuration {path}: {error}"
        ) from error


def _write_config_atomically(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    mode = stat.S_IMODE(path.stat().st_mode) if path.exists() else 0o600
    descriptor, temporary_name = tempfile.mkstemp(
        dir=path.parent,
        prefix=f".{path.name}.",
        text=True,
    )
    temporary_path = Path(temporary_name)

    try:
        with os.fdopen(descriptor, "w") as config_file:
            config_file.write(content)
        temporary_path.chmod(mode)
        temporary_path.replace(path)
    except BaseException:
        temporary_path.unlink(missing_ok=True)
        raise


def _load_config(path: Path) -> dict:
    try:
        with path.open("rb") as config_file:
            return tomllib.load(config_file)
    except FileNotFoundError as error:
        raise PrinterConfigError(
            f"printer configuration does not exist: {path}"
        ) from error
    except tomllib.TOMLDecodeError as error:
        raise PrinterConfigError(
            f"invalid printer configuration {path}: {error}"
        ) from error


def _require_string(config: dict, field: str, printer: str) -> str:
    value = config.get(field)
    if not isinstance(value, str) or not value:
        raise PrinterConfigError(
            f"printer {printer!r} field {field!r} must be a non-empty string"
        )
    return value


def _optional_string(config: dict, field: str, printer: str) -> str | None:
    value = config.get(field)
    if value is None:
        return None
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

    if (
        isinstance(parsed, bool)
        or not isinstance(parsed, int)
        or not 0 <= parsed <= maximum
    ):
        raise PrinterConfigError(
            f"printer {printer!r} field {field!r} must be between 0 and {maximum:#x}"
        )
    return parsed


def _optional_usb_integer(
    config: dict, field: str, printer: str, *, maximum: int
) -> int | None:
    if field not in config:
        return None
    return _require_usb_integer(config, field, printer, maximum=maximum)
