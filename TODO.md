# Developer-tool roadmap

## Product direction

ESCPost should become an ESC/POS developer workbench, not only a PNG
renderer.

The intended workflow is:

```text
Capture → inspect → preview → diagnose → replay → compare with hardware
```

The deterministic renderer remains the core library. Network transports, the
web interface, physical-printer access, and developer commands build around
that core without changing the submitted ESC/POS bytes.

This document describes planned work. Features listed here are not implemented
unless the current README and command coverage say otherwise. Once a feature
exists and its shape is durable, its architecture belongs in `ARCHITECTURE.md`
and the completed item can be removed from this roadmap.

`CLI.md` is the durable command-line contract. This file is the only
implementation checklist: roadmap items refer to stable CLI requirement IDs
without duplicating completion state in the contract.

## Rust CLI location

`crates/escpost-cli/` is the Rust binary crate. Its executable remains named
`escpost`.

The crate should initially contain command parsing and the modules used only by
the developer executable:

```text
crates/escpost-cli/
└── src/
    ├── main.rs
    ├── commands/
    │   ├── render.rs
    │   ├── inspect.rs
    │   ├── serve.rs
    │   ├── proxy.rs
    │   ├── replay.rs
    │   ├── diff.rs
    │   ├── lint.rs
    │   ├── printers.rs
    │   ├── calibrate.rs
    │   └── doctor.rs
    └── server/
        ├── raw_tcp.rs
        ├── web.rs
        ├── jobs.rs
        └── status.rs
```

This layout is illustrative rather than a requirement to create every module
up front. Add each module with the feature that needs it.

Do not create a separate server or protocol crate merely in anticipation of
reuse. Extract one when another executable or embedding API genuinely needs
the same behavior. The `escpost` rendering crate must remain independent of
CLI, networking, storage, and web concerns.

The existing Python package remains the application binding. Migrate the
current Click commands incrementally and remove them only after the Rust CLI
has equivalent behavior. The root `./escpost` container wrapper should
eventually invoke the Rust executable while keeping the developer-facing
command name stable.

## Rust CLI foundation and render migration

These tasks implement the first vertical slice of `CLI.md`. They come before
the virtual printer because `render --web` proves the executable, renderer
integration, web server, embedded assets, and packaging model with a known
input.

### Command foundation

- [x] Create `crates/escpost-cli` with an `escpost` binary and `clap` derive
      command model. (`CLI-G01`)
- [x] Add the global `--non-interactive` option and effective terminal-policy
      detection. (`CLI-G02`, `CLI-G03`)
- [x] Add one value-resolution layer for explicit values, metadata,
      configuration, defaults, and later interactive prompts. (`CLI-G04`)
- [x] Keep binary stdout, structured output, human status, and diagnostics on
      their documented channels. (`CLI-G05`)
- [x] Use typed command errors and nonzero failure statuses without adding a
      catch-all error dependency. (`CLI-G06`)
- [x] Handle `Ctrl+C` cleanly for long-running commands. (`CLI-G07`)
- [x] Keep the current Python hardware commands reachable during migration
      without publishing two executables named `escpost`.

### Inputs and profiles

- [x] Load raw binary files, `.hex` files, and stdin with explicit format
      overrides. (`CLI-I01`, `CLI-I02`, `CLI-I03`)
- [x] Recognize conformance-case directories and reject arbitrary directories
      as ESC/POS sources. (`CLI-I04`)
- [x] Resolve profiles from `--profile`, case metadata, or an interactive
      choice; fail clearly when unresolved in non-interactive mode.
      (`CLI-I06`)
- [ ] Add capture inputs only with the capture-store implementation.
      (`CLI-I05`)

### PNG destinations

- [x] Add `escpost render <SOURCE>`. (`CLI-R01`)
- [x] Support `-o <PNG>`, `-o -`, and `--output-dir <DIRECTORY>`.
      (`CLI-R02`)
- [x] Require exactly one sheet for a single-PNG destination unless
      `--sheet <NUMBER>` selects one. (`CLI-R03`)
- [x] Protect terminal stdout and keep piped PNG bytes exact. (`CLI-R04`)
- [x] Write deterministic sheet names and publish the manifest only after all
      PNGs succeed. (`CLI-R05`)
- [x] Overwrite explicit and conflicting generated outputs without prompting,
      render before replacing an existing file, and preserve unrelated or
      stale files. (`CLI-R08`)

### Rust web output

- [x] Add `--web`, `--browser`, `--web-listen`, and filesystem `--watch`.
      (`CLI-W01`, `CLI-W02`, `CLI-W08`)
- [x] Host the current HTML interface from the Rust executable and keep the
      initial asset embedded in the binary. (`CLI-W07`, `CLI-W10`)
- [x] Search and retain the first bindable loopback port from 9000 through
      9099 when no address is specified. (`CLI-W03`, `CLI-W04`)
- [x] Bind explicit nonzero addresses strictly, support explicit port zero,
      and make non-loopback exposure visible. (`CLI-W05`, `CLI-W06`)
- [x] Keep jobs and PNGs in memory unless a file destination was explicitly
      selected. (`CLI-W09`)
- [x] Permit file output together with web output, but reject stdout PNG output
      with a long-running web mode. (`CLI-R06`, `CLI-R07`)
- [x] Add HTTP and CLI integration coverage for `CLI-T01` through `CLI-T06`.
- [x] Verify feature parity in Docker and a real browser, then remove the
      Python `http.server` preview service and its manifest-polling workflow.
      (`CLI-T07`)
- [x] Update the root wrapper, Compose configuration, README examples, and
      architecture after the Rust path becomes authoritative.

## Virtual network printer

- [ ] Add an `escpost serve` command.
- [ ] Listen for RAW TCP print data on port 9100 by default.
- [ ] Bind to `127.0.0.1` by default.
- [ ] Require an explicit option to listen on LAN or public interfaces.
- [ ] Select one printer profile for each listener.
- [ ] Accept commands split across arbitrary TCP packet boundaries.
- [ ] Render every completed job without modifying its input bytes.
- [ ] Expose the captured job and its ordered PNG sheets in the web interface.
- [ ] Allow separate configuration of the RAW printer port and HTTP port.
- [ ] Apply input, rendered-dot, sheet-count, connection, and retention limits.
- [ ] Provide a health endpoint suitable for containers and automated tests.

Port 9100 is the common RAW/AppSocket transport used by network printers. It is
not an ESC/POS-defined job protocol and provides no authentication or
encryption. See the
[OpenPrinting network-printer documentation](https://openprinting.github.io/cups/doc/network.html).

An initial invocation could look like:

```bash
escpost serve \
  --listen 127.0.0.1:9100 \
  --profile REFERENCE \
  --web-listen 127.0.0.1:8765
```

### Job and sheet boundaries

Network connection boundaries and receipt cuts describe different things.
They must not be conflated.

- [ ] Treat a TCP connection close as the default end of the active job.
- [ ] Treat Standard-mode `FF` as an explicit ESC/POS job boundary.
- [ ] Treat full and partial cuts as sheet boundaries within a job.
- [ ] Support multiple explicitly completed jobs on one persistent connection.
- [ ] Offer an optional idle timeout for clients that keep a connection open
      without sending an explicit job terminator.
- [ ] Make timeout-completed jobs visibly distinguishable from explicitly
      completed jobs.
- [ ] Test one-byte TCP chunks, commands split across chunks, several commands
      in one chunk, persistent connections, disconnects, and truncated jobs.

Epson describes Standard-mode `FF` as completing one series of printing
actions. See the
[Epson `FF` command reference](https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/ff_in_standard.html).

## Rust web interface

- [x] Replace the Python preview server with a server hosted by the Rust CLI.
- [x] Keep ordered sheet names and responsive wrapping.
- [x] Keep original one-printer-dot-to-one-screen-pixel display as the default.
- [ ] Show live jobs newest first.
- [ ] Update the page when a job arrives without requiring manual refresh.
- [ ] Show the selected profile, completion reason, connection, timestamp, and
      rendering status for each job.
- [ ] Allow rerendering a captured job with another profile.
- [ ] Download the original binary, readable hexadecimal input, PNG sheets,
      command trace, events, and diagnostics.
- [ ] Show multiple profile renderings side by side.
- [ ] Offer exact-pixel overlay or difference views where that helps compare
      profiles or renderer versions.
- [ ] Replay a selected job to a configured physical printer.
- [ ] Export a captured job as a reproducible conformance case.
- [ ] Control simulated paper, cover, error, drawer, and online status after
      bidirectional emulation is available.
- [ ] Make persistence optional and provide an explicit retention limit.

Captured receipts can contain personal, order, and payment information. The UI
must clearly show whether jobs are held only in memory or written to disk.
Persistent capture should be opt-in unless the developer selects an explicit
local storage directory.

## Command inspector

The inspector should explain how the byte stream changes printer state and
produces output. It should be useful even when strict rendering fails.

- [ ] Add `escpost inspect <input>`.
- [ ] Show the byte offset and raw bytes for every parsed command.
- [ ] Show the command's ESC/POS name and decoded parameters.
- [ ] Show relevant printer state before and after the command.
- [ ] Record the dot bounds painted by each printable command.
- [ ] Record paper feeds, cuts, drawer pulses, and other device events.
- [ ] Link profile-dependent behavior to the selected profile field.
- [ ] Link standard behavior to the relevant ESC/POS reference page.
- [ ] Let the web UI highlight output bounds when a command is selected.
- [ ] Let the web UI jump from a rendered element to its originating command.
- [ ] Preserve and expose the exact raw bytes after every failure.

Diagnostics must keep these cases separate:

- malformed or truncated ESC/POS;
- a valid command not yet implemented by `escpost`;
- a valid command unavailable on the selected printer profile;
- a command ignored because of its parameters or the current printer state;
- clipped or out-of-area output;
- a documented profile approximation; and
- input after the last safely framed command which cannot be parsed reliably.

Strict rendering must continue to stop rather than guess after unsafe framing.
The capture and inspector layers may show the remaining opaque bytes, but must
not present speculative parsing as fact.

## Transparent physical-printer proxy

- [ ] Add an `escpost proxy` command.
- [ ] Accept the same RAW TCP input as the virtual printer.
- [ ] Forward the exact bytes to a configured USB or network printer.
- [ ] Capture and preview those bytes without delaying them unnecessarily.
- [ ] Forward physical-printer responses to the originating client.
- [ ] Never normalize, repair, or rewrite bytes in proxy mode.
- [ ] Make physical device actions explicit in the command invocation.
- [ ] Surface upstream and downstream disconnects clearly.
- [ ] Save enough metadata to replay the exact input later.
- [ ] Test backpressure, partial writes, printer disconnects, and responses
      interleaved with continued host input.

An example invocation could be:

```bash
escpost proxy \
  --listen 127.0.0.1:9100 \
  --to printer:netum-usb
```

Proxy mode provides the shortest comparison loop: an ERP sends one job, the
developer sees the PNG, and the real printer receives the identical bytes.

## Bidirectional printer emulation

Some POS applications wait for printer status or identity before continuing.
A listener which only consumes data is therefore a capture server, not yet a
complete virtual printer.

- [ ] Handle `DLE EOT` real-time status requests.
- [ ] Handle `GS a` Automatic Status Back subscriptions.
- [ ] Handle the commonly queried `GS I` printer identity forms.
- [ ] Source supported identity and status behavior from the selected profile.
- [ ] Model online, offline, cover, paper, error, feed-button, drawer, and
      cutter state as capabilities require.
- [ ] Send automatic status when an enabled state changes.
- [ ] Allow simulated state to be changed through the web UI and a local API.
- [ ] Support deterministic delayed replies for client resilience tests.
- [ ] Support deliberate disconnects and missing replies for failure tests.
- [ ] Keep real-time command recognition safe inside length-framed binary
      payloads.
- [ ] Add model-specific response forms only when a profile and evidence
      justify them.

Relevant initial references are Epson's
[`DLE EOT`](https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/dle_eot.html),
[`GS a`](https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_la.html),
and
[`GS I`](https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_ci.html)
documentation.

## Developer CLI

The eventual command set and its shared interaction, input, output, and error
rules are specified in `CLI.md`.

- [ ] Accept binary files, hexadecimal text files, standard input, and captured
      job identifiers where applicable.
- [ ] Keep machine-readable JSON output separate from concise human output.
- [ ] Use nonzero exit statuses for rendering, linting, comparison, connection,
      and configuration failures.
- [ ] Ensure automation never needs to scrape the web interface.
- [x] Keep the Docker wrapper as the documented development entry point.
- [ ] Add shell completion only after command names and arguments stabilize.

### Printer discovery and diagnostics

- [ ] Preserve existing USB discovery and `local/printers.toml` configuration.
- [ ] Add configured RAW network-printer targets.
- [ ] Add a safe direct host-and-port reachability check.
- [ ] Add profile-controlled status and identity probes that do not print.
- [ ] Explain USB device permissions and container group access in `doctor`.
- [ ] Avoid broad network scanning or vendor discovery protocols until a
      concrete integration needs them.
- [ ] Never send a printable probe to an unknown device without confirmation.

## Capture, replay, and regression tooling

- [ ] Give each captured job a stable local identifier.
- [ ] Preserve its immutable raw byte stream.
- [ ] Store connection and profile metadata separately from the bytes.
- [ ] Replay a capture to the virtual printer, USB, or RAW network printer.
- [ ] Replay using chosen TCP chunk sizes.
- [ ] Offer slow writes and configurable pauses.
- [ ] Disconnect at a selected byte offset.
- [ ] Delay, suppress, or alter simulated status responses.
- [ ] Export a capture into the existing conformance-case format.
- [ ] Generate expected, actual, and visual-difference PNGs for failed golden
      comparisons.
- [ ] Compare command traces, device events, diagnostics, sheet count, sheet
      dimensions, dot surfaces, and PNGs where each is relevant.
- [ ] Investigate a stream minimizer which removes bytes while preserving a
      selected parse error, rendering difference, or physical-printer symptom.

The export format should use ordinary files that are readable and reviewable
in Git. Do not introduce a database solely to store local captures. Begin with
an optional directory containing the raw bytes, small metadata, and derived
artifacts; revisit storage only when real usage proves that inadequate.

## Portability analysis

- [ ] Add `escpost lint`.
- [ ] Run one stream against one or several selected profiles.
- [ ] Report unsupported commands, code pages, symbols, and mechanisms.
- [ ] Report content outside the printable area.
- [ ] Report cutter commands for profiles without a cutter.
- [ ] Report model-dependent behavior which can materially change layout.
- [ ] Compare sheet count, dimensions, events, diagnostics, and approximations.
- [ ] Render the same capture side by side for selected profiles.
- [ ] Keep portability warnings separate from invalid-input errors.

An example invocation could be:

```bash
escpost lint receipt.bin \
  --profiles REFERENCE,NT-5890K
```

## Profile calibration workflow

- [ ] Preserve the shared physical calibration receipt.
- [ ] Render and print exactly the same version-controlled input.
- [ ] Guide developers from printer discovery to local configuration.
- [ ] Show expected output, generated output, physical evidence, and remaining
      profile TODOs together.
- [ ] Allow a captured physical-printer job to be rerendered immediately after
      a profile edit.
- [ ] Validate and explain profile fields before compiling the profile pack.
- [ ] Keep model-specific facts and evidence in the printer's profile
      directory.
- [ ] Do not infer permanent capabilities merely because one undocumented
      probe happened to print.

## Security and resource safety

- [ ] Default both RAW TCP and HTTP listeners to loopback.
- [ ] Warn clearly before binding RAW port 9100 to a non-loopback interface.
- [ ] Recommend a trusted LAN, VPN, or SSH tunnel for remote use.
- [ ] Do not imply that RAW port 9100 supports authentication.
- [ ] Require an explicit physical target before forwarding bytes.
- [ ] Never forward captured jobs automatically after a restart.
- [ ] Apply existing renderer resource limits to network submissions.
- [ ] Limit open connections, input rate, job duration, retained jobs, and disk
      usage.
- [ ] Treat receipt contents as potentially sensitive.
- [ ] Avoid logging entire receipt payloads unless the developer requests it.
- [ ] Make destructive device commands and simulated power commands visible in
      traces even when no physical action is taken.

## Implementation order

### Phase 1: Rust render command and web output

- [x] Complete the Rust CLI foundation and render-migration checklist above.
- [x] Verify file, directory, stdout, and web destinations end to end.
- [x] Remove the Python preview service only after the Rust web path passes its
      automated and browser checks.

### Phase 2: virtual printer

- [ ] Add the RAW TCP listener and job framing.
- [ ] Feed completed network jobs into the web job store shared with
      `render --web`.
- [ ] Show live ordered sheets and downloadable raw input.
- [ ] Add container health and transport-fragmentation tests.

### Phase 3: inspection

- [ ] Add a public command trace and structured diagnostics to the renderer.
- [ ] Add byte offsets, state changes, painted bounds, and device events.
- [ ] Expose the trace through CLI JSON and the web interface.
- [ ] Add profile switching and rerendering of captured jobs.

### Phase 4: hardware loop

- [ ] Port printer configuration and discovery to the Rust CLI.
- [ ] Add replay to USB and RAW network printers.
- [ ] Add transparent proxy mode with response forwarding.
- [ ] Port calibration commands and then retire the Click CLI.

### Phase 5: realistic emulation and integration testing

- [ ] Add status and printer-identity responses.
- [ ] Add controllable printer faults and timing behavior.
- [ ] Add multi-profile linting and visual comparisons.
- [ ] Add conformance-case export and stream minimization.

The first useful developer release should include the virtual RAW printer,
live PNG preview, command inspector, and transparent proxy. Those features
solve the normal integration loop without waiting for every post-v1 ESC/POS
command family.
