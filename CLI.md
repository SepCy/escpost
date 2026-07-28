# Command-line interface

## Purpose

The `escpost` command is the developer-facing ESC/POS workbench. It should be
useful both at a terminal and in unattended scripts without exposing different
Python and Rust command families.

This document defines the intended public behavior of the Rust CLI. It is both
a user reference and a contract against which the implementation and tests can
be reviewed. `render`, direct-USB `print`, and USB-backed `printers list` are
implemented; the other top-level commands remain planned or temporarily
available through the Python hardware workflow.
`README.md` describes what works today, while `TODO.md` is the single
implementation checklist.

Stable requirement identifiers appear in the final section. Keep an identifier
when wording is clarified so tests, issues, and roadmap items can continue to
refer to the same behavior.

## Command model

```text
escpost render       Render a known byte stream
escpost print        Send a known byte stream unchanged to a physical printer
escpost inspect      Decode and explain a byte stream
escpost serve        Run a virtual printer and its web interface
escpost proxy        Capture while forwarding to a physical printer
escpost replay       Resend a captured job
escpost diff         Compare jobs or renderings
escpost lint         Find portability and profile problems
escpost printers     List available printers and manage discovery or pairing
escpost calibrate    Compare rendering with physical hardware
escpost doctor       Diagnose the local ESCPost environment
```

`render` and `serve` share rendering and web-interface code, but they represent
different lifecycles:

```text
render → process input already known to the developer
serve  → wait for future jobs sent to a virtual printer
```

There is no separate `preview` command. Browser presentation is an output mode
of `render`.

## Global behavior

### Non-interactive operation

`--non-interactive` is a global option and may appear before or after a
subcommand:

```bash
escpost --non-interactive render receipt.bin --profile REFERENCE -o receipt.png
escpost render receipt.bin --profile REFERENCE -o receipt.png --non-interactive
```

Non-interactive mode:

- never displays a prompt;
- never treats a missing confirmation as approval;
- continues to use explicit arguments, configuration, source metadata, and
  documented defaults; and
- returns a clear nonzero error when a required value remains unresolved.

ESCPost also behaves non-interactively when stdin is not a terminal, when stdin
contains receipt data, or when a machine-readable output mode is active. This
prevents a CI job or pipeline from waiting indefinitely for input.

`--non-interactive` is not an alias for `--yes` or `--force`. Operations that
need explicit consent use an operation-specific option such as
`--allow-printing-probe`.

### Resolving omitted values

A value which supports interactive selection is resolved in this order:

```text
explicit command-line value
→ source metadata or local configuration
→ documented command default
→ interactive prompt
→ missing-value error
```

Prompts are appropriate when they reduce genuine ambiguity, for example
selecting one discovered printer or choosing a printer profile. Commands
should not prompt merely to repeat an obvious default.

Promptable values remain optional during command-line parsing and become
required during resolution. This lets the same command provide a guided
terminal workflow and a strict automation workflow.

### Standard streams

A single hyphen (`-`) represents stdin when used as an input source and stdout
when used as an output destination.

Two hyphens (`--`) end option parsing. They do not select stdout:

```bash
# Read ESC/POS from stdin and write one PNG to stdout.
generate-receipt |
  escpost render - --format binary --profile REFERENCE -o - > receipt.png

# Read a file whose name starts with a hyphen.
escpost render --profile REFERENCE -o receipt.png -- -receipt.bin
```

When binary data is written to stdout:

- stdout contains only those bytes;
- progress, diagnostics, and the selected web URL use stderr;
- prompting is disabled; and
- ESCPost refuses to write binary data directly to an interactive terminal.

Machine-readable output must never be mixed with human status messages.

## Input sources

Commands which consume ESC/POS use a positional `SOURCE`.

### Files

A regular file may contain:

- raw ESC/POS bytes; or
- readable hexadecimal text.

`--format auto|binary|hex` controls decoding. Automatic mode may use a
recognized filename extension such as `.hex`; it must not inspect arbitrary
file contents and guess that text-looking binary data is hexadecimal.

### Standard input

`SOURCE` equal to `-` reads the complete byte stream from stdin. The format
defaults to binary unless explicitly selected.

### Structured directories

A directory is accepted only when it is a recognized ESCPost bundle, such as:

- a conformance case containing `case.toml` and `input.hex`; or
- a captured job containing the documented capture metadata and immutable
  input bytes.

An arbitrary directory of PNGs is not an ESC/POS source. The temporary
`local/preview/manifest.json` workflow is not part of the Rust CLI contract.

### Captures

Commands may eventually accept a capture identifier together with its local
capture store. A capture always supplies immutable original bytes; changing
profiles rerenders those bytes rather than rewriting the capture.

## Profile selection

Rendering behavior depends on a printer profile. The profile is resolved from:

1. an explicit `--profile`;
2. recognized case or capture metadata; or
3. interactive selection.

If none is available in non-interactive operation, the command fails and
suggests `--profile REFERENCE` for a generic virtual printer. ESCPost does not
silently claim hardware fidelity by choosing a physical profile.

Commands must show the selected profile in human and structured results.
File and directory rendering report it on stderr. Binary stdout remains
byte-clean, and the web API includes it in its JSON result.

## `escpost render`

`render` turns one known ESC/POS source into one or more one-bit PNG sheets.

```text
escpost render <SOURCE>
    [--format auto|binary|hex]
    [--profile <PROFILE>]
    [-o <PNG> | --output-dir <DIRECTORY>]
    [--sheet <NUMBER>]
    [--web]
    [--browser]
    [--web-listen <ADDRESS>]
    [--watch]
```

At least one destination is required:

- `-o` or `--output` writes one PNG;
- `--output-dir` writes every sheet and a manifest;
- `--web` hosts the result in the local web interface; or
- `--browser` hosts the result and opens the default browser.

File output and web output may be requested together. In an interactive
terminal, ESCPost may ask the developer to choose a destination when none was
provided. In non-interactive operation, omitting every destination is an
error.

### Single-file and stdout output

`-o receipt.png` and `-o -` each represent exactly one PNG. They succeed when:

- the job renders exactly one sheet; or
- `--sheet <NUMBER>` selects one sheet from a multi-sheet result.

If several sheets exist and no sheet was selected, ESCPost fails and recommends
`--output-dir`. It never discards later sheets or concatenates PNG files.
`--sheet` is valid only together with `--output`.

An explicit file destination is overwritten without prompting when rendering
succeeds. This is normal transformation-command behavior and keeps automation
predictable; `--force` is not required. ESCPost must finish rendering before
it replaces an existing output, so a render failure leaves the previous file
untouched.

`-o -` cannot be combined with `--web`, `--browser`, or `--watch`. A web
process remains alive, so a downstream pipeline would not receive a timely
end-of-file.

### Directory output

`--output-dir` writes every sheet with deterministic ordered names and writes
the manifest only after all PNG files are complete. Consumers can therefore
treat a visible manifest as the ordered list of completed output.

Sheet names are `sheet-001.png`, `sheet-002.png`, and so on. `manifest.json`
contains those names in print order.

The command creates the directory when necessary and overwrites generated
files with the same names. It does not delete unrelated files or stale sheets
left by an earlier render. The manifest is the authoritative list for the
current result, so consumers ignore any unlisted PNGs.

### Web output

`--web` renders the job in memory, starts the local web viewer, prints its URL,
and remains active until interrupted.

`--browser` implies `--web` and additionally opens the selected URL with the
operating system's default browser. Browser launch is never implied by
`--web`, which keeps Docker, SSH, and headless use predictable.

The development Docker wrapper cannot launch the host browser. Use `--web`
with the wrapper and open its printed URL; `--browser` is intended for the
host-native executable.

If `--web-listen` is omitted, ESCPost:

1. binds only to `127.0.0.1`;
2. attempts ports `9000` through `9099` in order;
3. retains the first successfully bound socket; and
4. reports an error when the range is exhausted.

The implementation must bind each candidate directly instead of probing and
releasing it, which would introduce a race with another process.

An explicit nonzero `--web-listen` address is strict: ESCPost either binds that
exact address or fails. Port zero asks the operating system to select an
available port:

```bash
escpost render receipt.bin \
  --profile REFERENCE \
  --web \
  --web-listen 127.0.0.1:0
```

Selecting a non-loopback address is an explicit request to expose potentially
sensitive receipt data. ESCPost must display that fact clearly and must never
choose a non-loopback address by default.

`--web-listen` implies `--web`. `--watch` also implies `--web` and rerenders a
filesystem source after it changes. Watch mode is unavailable for stdin and
immutable captures.

When file output and watch mode are combined, every successful rerender also
updates the selected file destination. Parse or render failures keep the
previous complete files and web result available and expose the error in the
web page.

The initial web interface must:

- show sheets in print order;
- label each sheet with its name, order, and printer-dot dimensions;
- use one printer dot per screen pixel at the default zoom;
- offer integer zoom without smoothing; and
- update after a watched input is rendered successfully.

A render or parse error must remain visible without replacing the last
successful result with a partial speculative rendering.

### Cases and later comparisons

When `SOURCE` is a conformance-case directory, `render --web` can additionally
show its expected PNGs, current actual PNGs, notes, and eventually a pixel
difference.

A later version may accept repeated profiles and show the same immutable input
rendered for each profile. This must remain one input job with several
interpretations, not several rewritten inputs.

## `escpost print`

`print` sends one known ESC/POS source unchanged to an explicitly identified
physical USB interface:

```text
escpost print <SOURCE>
    [--format auto|binary|hex]
    --usb-vendor-id <ID>
    --usb-product-id <ID>
    --usb-interface <NUMBER>
    --usb-out-endpoint <ADDRESS>
```

The source rules are the same as for `render`. A conformance-case directory
supplies its immutable `input.hex`, but its profile metadata does not select or
alter the physical target. `print` does not require a renderer profile.

All four USB values are required. IDs and endpoint addresses accept decimal or
`0x`-prefixed notation. The first implementation deliberately provides:

- no printer-name or local-configuration lookup;
- no USB interface or endpoint discovery;
- no profile inference;
- no default USB values; and
- no automatic choice when several devices share the requested vendor and
  product IDs.

When no matching device exists, or when several devices match, the command
fails without sending data. A later explicit disambiguator or discovery
workflow may make several identical devices addressable.

The command invocation itself authorizes the physical write; it does not ask
for another confirmation. It claims exactly the supplied interface and opens
the supplied bulk OUT endpoint. On Linux, claiming may temporarily detach the
kernel printer driver for that interface. The interface is released when the
command ends.

The transport sends exactly the bytes loaded from `SOURCE`. It must not prepend
initialization, append feeds or cuts, render content, normalize line endings,
or call high-level printer helpers. Success reports the USB target and byte
count on stderr. Failures return nonzero and distinguish enumeration, missing
or ambiguous devices, open/claim/endpoint failures, and incomplete transfers.

Automated tests substitute only at the physical USB boundary. Ordinary test
commands must never send bytes to connected printers; hardware output happens
only through an explicit `escpost print` invocation.

## `escpost printers`

`printers` separates passive inventory from active discovery and connection
setup:

```text
escpost printers [--config <FILE>] list [--transport <TRANSPORT>] [--json]
escpost printers [--config <FILE>] scan [--transport <TRANSPORT>]
escpost printers [--config <FILE>] pair <CANDIDATE>
```

Commands in the `printers` family resolve `printers.toml` in this order:

```text
--config <FILE>
→ $ESCPOST_CONFIG_DIR/printers.toml
→ platform user-configuration directory
```

The platform default comes from the operating system through Rust's
`directories` crate. Linux uses
`$XDG_CONFIG_HOME/escpost/printers.toml`, falling back to
`~/.config/escpost/printers.toml`. A missing implicit file means no configured
printers. Read-only commands do not create the directory or file.

### `printers list`

`list` is the normal read-only command. It returns printers that are currently
usable or already known to the operating system, across every implemented
transport:

- attached USB printers;
- paired Bluetooth Classic printers exposed through a supported serial or
  RFCOMM path;
- supported, paired Bluetooth Low Energy printers;
- configured operating-system printer queues; and
- configured network printers.

The default includes every supported transport. `--transport
usb|bluetooth|network|spooler` narrows the result without changing its shape.
The human output identifies the transport and shows the connection fields
needed by the corresponding print command. When a connected USB interface
matches a saved entry, it also shows the developer-assigned name and profile.
`--json` exposes the same snapshot for scripts using a versioned schema.

Listing is passive: it does not pair devices, change configuration, send
ESC/POS data, or start a broad Bluetooth or network search. Reading USB
descriptors and querying printers already registered with the operating system
are part of listing.

### `printers scan`

`scan` is reserved for an active search for new or unconfigured devices. It may
take longer, request operating-system permissions, and find nearby Bluetooth
or network candidates which are not yet usable. Results are candidates rather
than silently saved printers. Scanning never pairs a device or sends printable
ESC/POS probes.

Transport-specific flags and timeouts must be documented when each scanning
backend is implemented. A broad network scan must remain opt-in.

### `printers pair`

`pair` turns one explicit scan candidate into a connection the operating
system or ESCPost can use. It is a state-changing operation and may delegate
to the platform's Bluetooth UI or permission flow. It never infers a candidate
from a printer name or silently selects one of several matches.

Non-interactive pairing requires every value and authorization needed by the
platform; otherwise it fails instead of waiting for a prompt. Some BLE
printers do not use operating-system pairing, so support is defined by the
transport backend rather than assumed for every Bluetooth device.

The current Rust implementation lists attached USB printer-class interfaces.
Bluetooth, network, spooler, `scan`, and `pair` support can be added without
renaming the inventory command or changing its read-only meaning.

## `escpost serve`

`serve` listens for future RAW print jobs and displays captured jobs in the
same web interface used by `render --web`.

```bash
escpost serve \
  --listen 127.0.0.1:9100 \
  --profile REFERENCE \
  --web-listen 127.0.0.1:9000
```

Unlike `render --web`, `serve`:

- accepts multiple jobs over time;
- treats network and ESC/POS boundaries according to the virtual-printer
  contract;
- keeps an ordered job history subject to retention limits; and
- may later emulate bidirectional printer status.

Running `serve` already provides previews of its captured jobs. A second
`render --web` process is not needed for those jobs.

RAW TCP and HTTP listeners bind to loopback by default. Exposing either
listener beyond the host must be explicit because RAW port 9100 has no
authentication or encryption and receipt contents may be sensitive.

## Other commands

The remaining commands should follow the same source, profile, interaction,
output, and error conventions where they apply.

- `inspect` explains parsed commands, byte offsets, state changes, output
  bounds, device events, and diagnostics.
- `proxy` forwards exact bytes while capturing and rendering them.
- `replay` sends an immutable capture to a selected transport.
- `diff` compares surfaces, PNGs, traces, or result metadata.
- `lint` reports portability and profile-specific problems without conflating
  them with invalid input.
- `printers` lists usable printers and owns explicit scanning and pairing.
- `calibrate` renders and physically prints the same version-controlled input.
- `doctor` reports platform, configuration, transport, and permission
  problems.

Their detailed syntax should be added here when implementation work begins,
before it is treated as stable.

## Errors and process behavior

- Invalid invocation, missing values, parse failures, rendering failures,
  connection errors, and unsafe requests return nonzero exit statuses.
- A failure message identifies the operation and actionable cause without
  dumping full receipt contents by default.
- Cancellation with `Ctrl+C` shuts down listeners and releases resources
  cleanly.
- Exact numeric exit-code categories must be documented before the first
  stable CLI release.
- Human output remains concise. Structured output is versioned before external
  automation is encouraged to depend on it.

## Requirement catalogue

Implementation status belongs only in `TODO.md`. This catalogue defines what
the completed implementation must satisfy.

### Global requirements

| ID | Requirement |
|---|---|
| CLI-G01 | Use one `escpost` executable with coherent top-level commands. |
| CLI-G02 | Accept global `--non-interactive` before or after subcommands. |
| CLI-G03 | Never prompt or assume confirmation in effective non-interactive mode. |
| CLI-G04 | Resolve promptable values using the documented precedence. |
| CLI-G05 | Keep binary, structured, and human output from corrupting one another. |
| CLI-G06 | Return nonzero status for every failed operation. |
| CLI-G07 | Shut down long-running commands cleanly on interruption. |

### Input and profile requirements

| ID | Requirement |
|---|---|
| CLI-I01 | Accept raw binary files, hexadecimal files, and stdin. |
| CLI-I02 | Treat `-` as stdin/stdout and `--` only as the end of options. |
| CLI-I03 | Never infer hexadecimal format by inspecting arbitrary input contents. |
| CLI-I04 | Accept only recognized structured directories as input sources. |
| CLI-I05 | Preserve immutable source bytes when loading captures. |
| CLI-I06 | Resolve and report the selected printer profile explicitly. |

### Render requirements

| ID | Requirement |
|---|---|
| CLI-R01 | Render a known source to one or more PNG sheets. |
| CLI-R02 | Support a single PNG file, stdout, all-sheet directory, and web destinations. |
| CLI-R03 | Never silently discard or concatenate sheets for a single-PNG destination. |
| CLI-R04 | Write only PNG bytes to stdout and refuse binary output to a terminal. |
| CLI-R05 | Write the all-sheet manifest only after every referenced PNG is complete. |
| CLI-R06 | Allow persisted PNG and web destinations in the same invocation. |
| CLI-R07 | Reject stdout PNG output combined with a long-running web mode. |
| CLI-R08 | Overwrite explicit and conflicting generated outputs without prompting while preserving unrelated files. |

### Physical-print requirements

| ID | Requirement |
|---|---|
| CLI-P01 | Accept the same file, hexadecimal, stdin, and recognized-directory sources as `render`. |
| CLI-P02 | Require explicit USB vendor ID, product ID, interface, and bulk OUT endpoint. |
| CLI-P03 | Never infer a printer alias, profile, interface, endpoint, or other USB default. |
| CLI-P04 | Send the loaded bytes unchanged without adding ESC/POS commands. |
| CLI-P05 | Fail without printing when zero or several devices match the requested IDs. |
| CLI-P06 | Report the selected USB target and transferred byte count without logging receipt contents. |
| CLI-P07 | Return typed, actionable errors and nonzero status for every failed physical operation. |
| CLI-P08 | Keep automated tests physically inert by substituting only at the USB boundary. |

### Printer-management requirements

| ID | Requirement |
|---|---|
| CLI-M01 | Make `printers list` a passive, transport-neutral inventory of currently usable or known printers. |
| CLI-M02 | List all supported transports by default and permit an explicit transport filter. |
| CLI-M03 | Identify each result's transport and expose the connection fields required for printing. |
| CLI-M04 | Keep machine-readable listing output separate from human output and version its schema. |
| CLI-M05 | Reserve `printers scan` for active discovery that never pairs, saves, or prints implicitly. |
| CLI-M06 | Reserve `printers pair` for an explicit state-changing connection workflow which may delegate to the operating system. |
| CLI-M07 | Never infer a scan or pairing target by display name or choose silently among several candidates. |
| CLI-M08 | Resolve printer configuration from an explicit file, `ESCPOST_CONFIG_DIR`, then the platform user-configuration directory. |
| CLI-M09 | Keep passive listing free of configuration writes while showing names that match connected printers. |

### Web requirements

| ID | Requirement |
|---|---|
| CLI-W01 | Make `--web` start the shared local web interface without opening a browser. |
| CLI-W02 | Make `--browser` imply `--web` and open the selected URL. |
| CLI-W03 | Search loopback ports 9000–9099 when no HTTP address was specified. |
| CLI-W04 | Bind and retain candidate sockets atomically rather than probing first. |
| CLI-W05 | Treat an explicit nonzero address as strict and support explicit port zero. |
| CLI-W06 | Never select a non-loopback listener implicitly. |
| CLI-W07 | Show ordered sheets at one printer dot per screen pixel by default. |
| CLI-W08 | Support filesystem watch mode without speculative partial results. |
| CLI-W09 | Keep receipt data in memory unless persistence was explicitly requested. |
| CLI-W10 | Reuse one web implementation for `render --web` and `serve`. |

### Automation and verification requirements

| ID | Requirement |
|---|---|
| CLI-T01 | Test command parsing in interactive and non-interactive policies. |
| CLI-T02 | Test binary stdout byte-for-byte without human-output contamination. |
| CLI-T03 | Test zero, one, and several rendered sheets for every destination type. |
| CLI-T04 | Test occupied automatic ports, exhausted ranges, strict ports, and port zero. |
| CLI-T05 | Test loopback defaults and rejection or warning of unintended exposure. |
| CLI-T06 | Test HTTP routes, ordered sheets, missing files, and path traversal. |
| CLI-T07 | Verify the Rust web workflow before removing the Python preview service. |
