# Command-line interface

`escpost` is the command-line toolbox for rendering, previewing, capturing,
and printing ESC/POS jobs. This document describes the commands available in
the current release. Planned commands and options are tracked in
[`TODO.md`](TODO.md).

## Installation

Install the CLI from crates.io with Rust 1.87 or newer:

```bash
cargo install escpost
```

Run `escpost --help` or `escpost <COMMAND> --help` for the concise built-in
reference.

## Commands

```text
escpost render       Render a known ESC/POS byte stream
escpost print        Send a known byte stream to a configured printer
escpost serve        Capture RAW TCP print jobs and preview them in a browser
escpost printers     List and register printers
escpost profiles     Browse the embedded printer-profile catalog
```

## Global option

`--non-interactive` prevents ESCPost from prompting for missing values. It may
appear before or after a subcommand:

```bash
escpost --non-interactive render receipt.bin --profile REFERENCE -o receipt.png
escpost render receipt.bin --profile REFERENCE -o receipt.png --non-interactive
```

When a required value cannot be resolved without prompting, the command exits
with an error. ESCPost also avoids prompting when standard input is not a
terminal or is being used as receipt input.

## Input sources

The `render` and `print` commands accept a positional `SOURCE`:

- a raw ESC/POS file;
- a readable hexadecimal file;
- `-` for standard input; or
- an ESCPost conformance-case directory containing `case.toml` and
  `input.hex`.

Use `--format auto|binary|hex` to select the representation. In `auto` mode,
files with a `.hex` extension and recognized case directories are hexadecimal;
other files and standard input are binary.

## `escpost render`

Render one ESC/POS source into one or more PNG sheets:

```text
escpost render [OPTIONS] <SOURCE>

Options:
    --format auto|binary|hex
    --profile <PROFILE>
    -o, --output <OUTPUT>
    --output-dir <DIRECTORY>
    --sheet <NUMBER>
    --web
    --browser
    --web-listen <ADDRESS>
    --watch
    --scale <N>
    --antialias[=true|false]
```

At least one output is required. In an interactive terminal, ESCPost can
prompt for one; with `--non-interactive`, it reports an error when none is
given.

### One PNG

`-o receipt.png` writes one PNG file. `-o -` writes only PNG bytes to standard
output:

```bash
escpost render receipt.bin \
  --profile REFERENCE \
  --output receipt.png \
  --non-interactive

generate-receipt | \
  escpost render - --format binary --profile REFERENCE -o - >receipt.png
```

If a job produces several sheets, use `--sheet <NUMBER>` to select a one-based
sheet. Without a selection, single-file output fails rather than discarding
later sheets. `--sheet` requires `--output` and cannot be combined with
`--output-dir`.

### All sheets

`--output-dir <DIRECTORY>` writes every sheet and a `manifest.json` file:

```bash
escpost render receipt.hex \
  --profile REFERENCE \
  --output-dir renderings \
  --non-interactive
```

Sheets use ordered names such as `sheet-001.png` and `sheet-002.png`. The
manifest is the authoritative list for the current render. Unrelated files in
the directory are preserved.

### Browser preview and watching

`--web` starts the local viewer and prints its URL. `--browser` also opens that
URL in the default browser. `--watch` rerenders a filesystem source after it
changes and implies web mode.

```bash
escpost render receipt.hex --profile REFERENCE --web --watch
```

Use `--web-listen <IP:PORT>` to request an exact address. Omitting it selects
the first available loopback port from 9000 through 9099. Port `0` asks the
operating system to choose a free port. Binding to a non-loopback address
exposes the receipt preview to the corresponding network.

The Docker wrapper cannot open a browser on the host. Use `--web` through the
wrapper and open the printed URL manually.

### Preview quality

`--scale <N>` renders each printer dot at `N × N` preview pixels. The default
is `1` for `render`. `--antialias` enables grayscale glyph edges for display;
it does not represent additional dots produced by a physical printer.

## `escpost print`

Send the source bytes unchanged to a configured printer:

```text
escpost print [OPTIONS] <SOURCE>

Options:
    --format auto|binary|hex
    --printer <NAME>
    --config <FILE>
```

Example:

```bash
escpost print receipt.hex --printer kitchen --non-interactive
```

`--printer` refers to a name registered in `printers.toml`. If it is omitted
at an interactive terminal, ESCPost offers the available configured printers.
In non-interactive operation, an unresolved printer is an error.

For a hexadecimal source, ESCPost decodes the text and sends the resulting
bytes. It does not insert initialization, feed, cut, or other ESC/POS commands.
USB and RAW TCP connection details come from the selected printer entry.

`--config <FILE>` selects an exact printer configuration file for this
invocation.

## `escpost printers`

The implemented printer-management commands are `list` and `add`.

Printer configuration is resolved in this order:

```text
--config <FILE>
→ $ESCPOST_CONFIG_DIR/printers.toml
→ the platform user-configuration directory
```

On Linux, the platform path is
`$XDG_CONFIG_HOME/escpost/printers.toml`, falling back to
`~/.config/escpost/printers.toml`.

### `printers list`

List attached USB printer interfaces and configured RAW TCP network printers:

```text
escpost printers [--config <FILE>] list [--transport usb|network]
```

Examples:

```bash
escpost printers list
escpost printers list --transport usb
escpost printers --config ./printers.toml list --transport network
```

Listing is read-only: a missing implicit configuration file is treated as an
empty configuration and is not created. The optional transport filter accepts
`usb` or `network`.

### `printers add`

Register a USB or RAW TCP network printer:

```text
escpost printers [--config <FILE>] add [<NAME>]
    [--transport usb|network]
    [--host <HOST>]
    [--port <PORT>]
    [--vendor-id <ID>]
    [--product-id <ID>]
    [--serial <SERIAL>]
    [--profile <PROFILE>]
```

Register a network printer:

```bash
escpost --non-interactive printers add kitchen \
  --transport network \
  --host printer.local \
  --port 9100 \
  --profile REFERENCE
```

The network port defaults to `9100` when omitted.

At an interactive terminal, USB registration lets the user select an attached
printer interface. A script can instead identify a device explicitly:

```bash
escpost --non-interactive printers add counter \
  --transport usb \
  --vendor-id 0x0416 \
  --product-id 0x5011 \
  --serial B120300001 \
  --profile NT-5890K
```

`--vendor-id` and `--product-id` accept decimal or `0x`-prefixed hexadecimal
values and must be supplied together. `--serial` optionally narrows otherwise
identical devices. Scripted registration fails rather than guessing when the
selector does not identify exactly one usable USB route.

Adding a printer records connection information; it does not send print data.
The profile is optional because raw jobs can be sent without rendering them.

## `escpost profiles`

Browse the embedded catalog of printer profiles. These commands do not access
physical printers or modify `printers.toml`.

### `profiles list`

```text
escpost profiles list
    [--vendor <NAME>]
    [--source calibrated|synthesized|virtual]
    [--search <TEXT>]
    [--json]
```

Filters compose with AND:

- `--vendor` matches a case-insensitive vendor substring;
- `--source` selects calibration provenance; and
- `--search` matches a case-insensitive substring of the profile id, vendor,
  or model.

Without `--json`, the command prints a compact table. `--json` prints the full
filtered catalog as a JSON array.

### `profiles show`

Show the complete details of one profile:

```bash
escpost profiles show NT-5890K
escpost profiles show REFERENCE --json
```

An unknown profile id is an error. `--json` prints one JSON object instead of
the human-readable detail view.

### `profiles find`

Interactively search the catalog and print the selected profile id:

```bash
escpost profiles find
```

The command requires an interactive terminal and is unavailable with
`--non-interactive`. For scripts, use `profiles list --search <TEXT>`.

## `escpost serve`

Run a virtual RAW TCP printer and preview captured jobs in the web viewer:

```text
escpost serve [OPTIONS]

Options:
    --profile <PROFILE>
    --listen <ADDRESS>
    --web-listen <ADDRESS>
    --idle-timeout <SECONDS>
    --scale <N>
    --antialias[=true|false]
    --no-open
```

Example:

```bash
escpost serve \
  --listen 127.0.0.1:9100 \
  --profile REFERENCE \
  --web-listen 127.0.0.1:9000
```

The profile defaults to `REFERENCE`. Without explicit addresses, the RAW TCP
listener selects the first free loopback port from 9100 through 9109 and the
web viewer selects one from 9000 through 9099.

A job completes when its client connection closes or after the configured
idle period. `--idle-timeout` defaults to 20 seconds; `0` disables idle
completion. The current viewer displays the most recently completed job.

The viewer opens automatically when the environment permits it. `--no-open`
(also accepted as `--no-browser`) disables that behavior. Auto-opening is also
skipped with `--non-interactive`, without a terminal, under CI, or when
`BROWSER=none`.

`--scale` defaults to `3` for the browser preview. Antialiasing is enabled by
default; pass `--antialias=false` for faithful one-bit printer dots.

RAW TCP port 9100 has no authentication or encryption. Binding either listener
to a non-loopback address can expose receipt data and should be deliberate.

## Errors and output

Invalid invocations, missing required values, decoding failures, rendering
failures, connection errors, and transfer errors return a nonzero exit status.
Human diagnostics go to standard error when standard output carries PNG or
JSON data.

Cancellation with `Ctrl+C` shuts down long-running web and virtual-printer
processes.
