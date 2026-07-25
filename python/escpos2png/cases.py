"""Loading and validation for shared renderer/printer conformance cases."""

from __future__ import annotations

import hashlib
import tomllib
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Case:
    directory: Path
    profile: str
    input_bytes: bytes
    input_sha256: str

    @classmethod
    def load(cls, directory: str | Path) -> Case:
        directory = Path(directory).resolve()
        manifest = _load_manifest(directory / "case.toml")
        _require_schema_version(manifest)

        input_path = _resolve_case_file(directory, _require_string(manifest, "input"))
        encoding = _require_string(manifest, "input_encoding")
        if encoding != "hex":
            raise CaseError(f"unsupported input encoding {encoding!r}")

        input_bytes = _decode_hex(input_path)
        expected_hash = _require_string(manifest, "input_sha256")
        actual_hash = hashlib.sha256(input_bytes).hexdigest()
        if actual_hash != expected_hash:
            raise CaseError(
                f"input SHA-256 mismatch: expected {expected_hash}, got {actual_hash}"
            )

        return cls(
            directory=directory,
            profile=_require_string(manifest, "profile"),
            input_bytes=input_bytes,
            input_sha256=actual_hash,
        )


class CaseError(ValueError):
    """A conformance case is missing, malformed, or internally inconsistent."""


def _load_manifest(path: Path) -> dict:
    try:
        with path.open("rb") as manifest_file:
            return tomllib.load(manifest_file)
    except FileNotFoundError as error:
        raise CaseError(f"case manifest does not exist: {path}") from error
    except tomllib.TOMLDecodeError as error:
        raise CaseError(f"invalid case manifest {path}: {error}") from error


def _require_schema_version(manifest: dict) -> None:
    version = manifest.get("schema_version")
    if version != 1:
        raise CaseError(f"unsupported case schema version {version!r}")


def _require_string(manifest: dict, field: str) -> str:
    value = manifest.get(field)
    if not isinstance(value, str) or not value:
        raise CaseError(f"case field {field!r} must be a non-empty string")
    return value


def _resolve_case_file(directory: Path, relative_path: str) -> Path:
    path = (directory / relative_path).resolve()
    if not path.is_relative_to(directory):
        raise CaseError(f"case file escapes its directory: {relative_path}")
    return path


def _decode_hex(path: Path) -> bytes:
    try:
        tokens = path.read_text(encoding="ascii").split()
    except (FileNotFoundError, UnicodeDecodeError) as error:
        raise CaseError(f"could not read hexadecimal input {path}: {error}") from error

    decoded = bytearray()
    for index, token in enumerate(tokens, start=1):
        invalid_character = any(
            character not in "0123456789abcdefABCDEF" for character in token
        )
        if len(token) != 2 or invalid_character:
            raise CaseError(f"invalid hexadecimal byte {token!r} at token {index}")
        decoded.append(int(token, 16))
    return bytes(decoded)
