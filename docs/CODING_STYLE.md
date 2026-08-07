# Coding style

ESCPost should be easy to change without losing rendering fidelity. Code
and tests are read more often than they are written, so clarity matters more
than cleverness.

## Write for the next reader

- Prefer simple names and direct control flow.
- Include units in names when confusion is possible: `width_bytes`,
  `height_dots`, and `units_per_inch` are better than `width`, `height`, and
  `scale`.
- Put the public or high-level operation first. Place the smaller functions it
  calls below it so a file can be read from coarse behavior to details.
- Keep constants and shared data types near the top of a file.
- Extract a helper when it gives a concept a useful name or removes distracting
  detail. Do not split a short operation merely to create more functions.
- Return a clear error for unsupported or unsafe input. Never hide uncertainty
  behind a silent best guess.

These rules apply to both the Rust code and the Python render binding.

## Comments explain why

Add a comment when the reason for code is not clear from the code itself.
Important examples include:

- ESC/POS state or buffering behavior;
- byte layout, bit order, coordinate systems, and unit conversions;
- rounding, clipping, scaling, and cursor-advance rules;
- values that come from a printer profile rather than the ESC/POS standard;
- deliberate documented divergences from physical hardware behavior; and
- code that prevents a subtle regression or unsafe parse.

Use plain language. A short explanation near the relevant code is usually
better than a long theoretical description.

```rust
// ESC * is column-major: each group describes one x coordinate and
// contains either 8 or 24 vertical source dots.
```

Do not add comments that only repeat the syntax:

```rust
// Add one to x.
x += 1;
```

If an explanation becomes long, put the durable design reasoning in
`ARCHITECTURE.md` or `DESIGN_DECISIONS.md` and leave a short pointer in the
code. Keep comments updated when behavior changes; an outdated comment is a
bug.

## Keep protocol knowledge visible

- Use ESC/POS command names such as `ESC t` or `GS v 0` in parser errors,
  function names, tests, and comments.
- Give raw control bytes readable names such as `ESC`, `GS`, and `LF` in tests.
- Keep printer-independent command behavior separate from profile-specific
  values and quirks.
- State which layout representation a command uses when it is easy to confuse,
  for example column-major `ESC *` versus row-major `GS v 0`.
- Use the offline Epson ESC/POS reference as the normative source for standard
  command behavior. Record model-specific facts in the printer profile or its
  evidence, not as unexplained constants in renderer code.

## Tests are executable explanations

Each test should make one useful behavior easy to understand.

- Name the command and behavior in the test name.
- Build the input near the assertion unless a shared conformance fixture is
  the behavior under test.
- Comment on byte sequences or expected dot coordinates when their meaning is
  not obvious.
- Explain why an assertion detects the regression. For example, a visible
  marker after a glyph can prove exact cursor advancement.
- Prefer exact dot positions and dimensions over vague image comparisons when
  testing layout.
- Keep test helpers below the tests so the behaviors are visible first.
- Include the relevant boundary, invalid input, or state-reset case when it
  protects meaningful parser behavior.

Comments in tests should describe printer behavior and the purpose of the
assertion. They should not restate the test name line by line.

## Formatting and checks

Rust changes should pass:

```bash
docker compose run --rm test cargo fmt --check
docker compose run --rm test cargo clippy --workspace --all-targets -- -D warnings
docker compose run --rm test cargo test --workspace
```

Changes to the Python render binding should remain formatted consistently with
the surrounding code and pass the containerized binding test suite documented
in the [project README](../README.md).
