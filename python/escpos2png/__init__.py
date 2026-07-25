"""Python interface for the escpos2png rendering engine."""

from ._native import render, render_result

__all__ = ["render", "render_result"]
