"""Loading the shared receipt used to calibrate printer profiles."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from .cases import CaseError, load_hex_input


@dataclass(frozen=True)
class Calibration:
    profile_directory: Path
    input_path: Path
    profile: str
    input_bytes: bytes

    @classmethod
    def load(cls, repository: str | Path, profile: str) -> Calibration:
        repository = Path(repository).resolve()
        profile_directory = repository / "profiles" / profile
        profile_path = profile_directory / "profile.toml"
        if not profile_path.is_file():
            raise CaseError(f"printer profile does not exist: {profile_path}")

        input_path = repository / "calibration" / "input.hex"
        return cls(
            profile_directory=profile_directory,
            input_path=input_path,
            profile=profile,
            input_bytes=load_hex_input(input_path),
        )
