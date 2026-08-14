use std::fmt;
use std::io::{self, IsTerminal, Write};
use std::time::Duration;

use crate::cli::{
    AddPrinterArgs, DiscoverPrintersArgs, InventoryTransport, PrinterTransport, PrintersArgs,
    PrintersCommand,
};
use crate::configuration::{
    self, ConfiguredNetworkPrinter, ConfiguredUsbPrinter, PrinterConfiguration,
    UsbPrinterRegistration,
};
use crate::discovery::{self, DiscoveredHost, ScanTarget, Subnet};
use crate::error::CliError;
use inquire::validator::Validation;
use inquire::{CustomType, Select, Text};
use nusb::MaybeFuture;
use nusb::descriptors::{ConfigurationDescriptor, TransferType};
use nusb::transfer::Direction;
use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::timeout;

const USB_CLASS_PRINTER: u8 = 0x07;
const NETWORK_PROBE_TIMEOUT: Duration = Duration::from_secs(1);
const UNASSIGNED_PROFILE: &str = "unassigned";

#[derive(Clone, Debug, PartialEq, Eq)]
struct UsbPrinter {
    vendor_id: u16,
    product_id: u16,
    bus: String,
    address: u8,
    manufacturer: Option<String>,
    product: Option<String>,
    serial_number: Option<String>,
    interface_number: u8,
    out_endpoints: Vec<u8>,
    in_endpoints: Vec<u8>,
}

/// OS-reported identity of a printer-class USB device, gathered without ever
/// opening it (see `UsbInventory::identities`). `printers list` uses this
/// alone to decide whether a saved USB printer is connected: interface and
/// endpoint routing are not available at this level, so a matched entry's
/// display block sources those from the saved configuration instead (see
/// `connected_usb_printer`).
#[derive(Clone, Debug, PartialEq, Eq)]
struct UsbDeviceIdentity {
    vendor_id: u16,
    product_id: u16,
    bus: String,
    address: u8,
    manufacturer: Option<String>,
    product: Option<String>,
    serial_number: Option<String>,
}

/// Best-effort USB enumeration for `printers discover`: printers found so
/// far, plus a warning line for each device that could not be opened or
/// whose active configuration could not be inspected. A device-level
/// failure never aborts the rest of the sweep, the same way the network
/// sweep silently skips unreachable hosts.
struct UsbEnumeration {
    printers: Vec<UsbPrinter>,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UsbAddTarget {
    vendor_id: u16,
    product_id: u16,
    bus: String,
    address: u8,
    manufacturer: Option<String>,
    product: Option<String>,
    serial_number: Option<String>,
    interface_number: u8,
    out_endpoint: u8,
    in_endpoint: Option<u8>,
    ambiguous_without_serial: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct UsbPrinterInterface {
    interface_number: u8,
    out_endpoints: Vec<u8>,
    in_endpoints: Vec<u8>,
}

/// A non-interactive request to register one connected USB printer by its
/// stable descriptor identity rather than by choosing it from a menu.
struct UsbSelector {
    vendor_id: u16,
    product_id: u16,
    serial: Option<String>,
}

struct ConnectedUsbPrinter {
    printer: UsbPrinter,
    configuration_index: Option<usize>,
}

/// The result of matching `printers list`'s metadata-only USB identities
/// against the saved configuration (see `merge_usb_identities`). Unlike
/// `printers discover`'s `ConnectedUsbPrinter`, an identity matching no
/// saved printer is simply dropped rather than kept with `configuration_index:
/// None`: `list` never shows a connected-but-unconfigured USB device, so
/// every entry here is already known to belong to one configuration index.
struct MergedUsbIdentities {
    connected: Vec<ConnectedUsbEntry>,
    unavailable_configuration_indexes: Vec<usize>,
}

struct ConnectedUsbEntry {
    printer: UsbPrinter,
    configuration_index: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct ResolvedAddPrinter {
    name: String,
    connection: ResolvedAddConnection,
    profile: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum ResolvedAddConnection {
    Usb(UsbAddTarget),
    Network { host: String, port: u16 },
}

impl ResolvedAddPrinter {
    fn transport(&self) -> PrinterTransport {
        match self.connection {
            ResolvedAddConnection::Usb(_) => PrinterTransport::Usb,
            ResolvedAddConnection::Network { .. } => PrinterTransport::Network,
        }
    }
}

struct ListedPrinter<'a> {
    display_name: String,
    kind: ListedPrinterKind<'a>,
}

enum ListedPrinterKind<'a> {
    ConnectedUsb(&'a ConnectedUsbEntry),
    UnavailableUsb(&'a ConfiguredUsbPrinter),
    Network {
        printer: &'a ConfiguredNetworkPrinter,
        connected: bool,
    },
}

trait UsbInventory {
    fn list(&mut self) -> Result<Vec<UsbPrinter>, CliError>;

    /// Metadata-only USB presence check for `printers list`: the OS-reported
    /// identity (vendor, product, serial, live bus/address, and
    /// manufacturer/product strings) of every printer-class device, without
    /// ever opening one. There is no per-device failure mode here — nothing
    /// about an individual device is opened or inspected — so, unlike
    /// `list_tolerant`, total enumeration failure is the only error this can
    /// return.
    fn identities(&mut self) -> Result<Vec<UsbDeviceIdentity>, CliError>;

    /// Best-effort enumeration for `printers discover`: a device that fails
    /// to open or whose active configuration cannot be inspected is skipped
    /// with a warning instead of aborting the whole enumeration, mirroring
    /// the network sweep's own tolerance of unreachable hosts. Total
    /// enumeration failure (the initial USB device listing itself) still
    /// propagates as an error. The default forwards to the strict `list()`
    /// with no warnings, since only the real USB backend can fail on
    /// individual devices; a test double that needs to exercise a partial
    /// failure overrides this directly.
    fn list_tolerant(&mut self) -> Result<UsbEnumeration, CliError> {
        Ok(UsbEnumeration {
            printers: self.list()?,
            warnings: Vec::new(),
        })
    }
}

trait AddPrompter {
    fn name(&mut self) -> Result<String, CliError>;
    fn reject_name(&mut self, error: &CliError) {
        eprintln!("Error: {error}. Choose another printer name.");
    }
    fn transport(&mut self) -> Result<PrinterTransport, CliError>;
    fn usb_printer(&mut self, printers: Vec<UsbAddTarget>) -> Result<UsbAddTarget, CliError>;
    fn host(&mut self) -> Result<String, CliError>;
    fn port(&mut self) -> Result<u16, CliError>;
    fn profile(&mut self) -> Result<Option<String>, CliError>;
}

pub(crate) async fn run(arguments: PrintersArgs, non_interactive: bool) -> Result<(), CliError> {
    match arguments.command {
        PrintersCommand::List(list) => {
            let path = configuration::resolved_path(arguments.config.as_deref())?;
            let configuration = configuration::load(arguments.config.as_deref())?;
            eprintln!(
                "Reading configuration from {}",
                configuration::display_path(&path)
            );
            let network_statuses = if list.transport == Some(InventoryTransport::Usb) {
                Vec::new()
            } else {
                probe_network_printers(configuration.network_printers()).await
            };
            let mut inventory = NusbInventory;
            execute(
                &mut inventory,
                &configuration,
                &network_statuses,
                list.transport,
                &mut io::stdout().lock(),
            )?;
            // Count-independent, unlike discover's hints: there is always
            // exactly one next step worth pointing at, whether the registry
            // was empty or full.
            eprintln!("Discover connected printers with: escpost printers discover");
            Ok(())
        }
        PrintersCommand::Add(mut add) => {
            if add.discover {
                if add.transport == Some(PrinterTransport::Usb) {
                    return Err(CliError::DiscoverForUsbPrinter);
                }
                // Discovery implies the network transport, so the wizard must
                // not ask for one.
                add.transport = Some(PrinterTransport::Network);
                let host =
                    discover_host_for_add(arguments.config.as_deref(), &add, non_interactive)
                        .await?;
                add.host = Some(host.address.to_string());
                add.port = Some(host.port);
            }
            add_printer(arguments.config.as_deref(), add, non_interactive)
        }
        PrintersCommand::Discover(discover) => {
            let path = configuration::resolved_path(arguments.config.as_deref())?;
            // Unlike `list`, a scan does not require a saved configuration to
            // already exist: an explicit --config naming a not-yet-created
            // file (the common case on a machine's first discovery run) is
            // not an error, only invalid TOML in an existing file is.
            let configuration = configuration::load_for_update(arguments.config.as_deref())?;
            eprintln!(
                "Reading configuration from {}",
                configuration::display_path(&path)
            );
            run_discover(discover, &configuration).await
        }
    }
}

fn add_printer(
    config_path: Option<&std::path::Path>,
    arguments: AddPrinterArgs,
    non_interactive: bool,
) -> Result<(), CliError> {
    let can_prompt = !non_interactive && io::stdin().is_terminal() && io::stderr().is_terminal();
    execute_add(
        config_path,
        arguments,
        can_prompt,
        &mut InquireAddPrompter,
        &mut NusbInventory,
    )?;
    Ok(())
}

pub(crate) fn add_interactively(config_path: Option<&std::path::Path>) -> Result<String, CliError> {
    execute_add(
        config_path,
        AddPrinterArgs {
            name: None,
            transport: None,
            host: None,
            port: None,
            vendor_id: None,
            product_id: None,
            serial: None,
            profile: None,
            discover: false,
            subnet: Vec::new(),
            timeout: None,
        },
        true,
        &mut InquireAddPrompter,
        &mut NusbInventory,
    )
}

fn execute_add(
    config_path: Option<&std::path::Path>,
    arguments: AddPrinterArgs,
    can_prompt: bool,
    prompter: &mut impl AddPrompter,
    inventory: &mut impl UsbInventory,
) -> Result<String, CliError> {
    let configuration = configuration::load_for_update(config_path)?;
    let resolved = resolve_add(arguments, can_prompt, prompter, inventory, &configuration)?;
    save_and_report_printer(config_path, &resolved)?;
    Ok(resolved.name)
}

fn save_and_report_printer(
    config_path: Option<&std::path::Path>,
    printer: &ResolvedAddPrinter,
) -> Result<(), CliError> {
    let path = match &printer.connection {
        ResolvedAddConnection::Network { host, port } => configuration::add_network_printer(
            config_path,
            &printer.name,
            host,
            *port,
            printer.profile.as_deref(),
        ),
        ResolvedAddConnection::Usb(target) => configuration::add_usb_printer(
            config_path,
            &printer.name,
            &UsbPrinterRegistration {
                vendor_id: target.vendor_id,
                product_id: target.product_id,
                serial_number: target.serial_number.as_deref(),
                interface_number: target.interface_number,
                out_endpoint: target.out_endpoint,
                in_endpoint: target.in_endpoint,
                profile: printer.profile.as_deref(),
            },
        ),
    }?;
    eprintln!("Printer: {}", printer.name);
    eprintln!("Transport: {}", printer.transport());
    eprintln!(
        "Updated configuration at {}",
        configuration::display_path(&path)
    );
    if let ResolvedAddConnection::Usb(target) = &printer.connection
        && target.ambiguous_without_serial
    {
        eprintln!(
            "Warning: this USB printer has no serial number; printing will be ambiguous while another device with the same USB identity is connected."
        );
    }
    Ok(())
}

fn resolve_add(
    arguments: AddPrinterArgs,
    can_prompt: bool,
    prompter: &mut impl AddPrompter,
    inventory: &mut impl UsbInventory,
    configuration: &PrinterConfiguration,
) -> Result<ResolvedAddPrinter, CliError> {
    let AddPrinterArgs {
        name,
        transport,
        host,
        port,
        vendor_id,
        product_id,
        serial,
        profile,
        // Already resolved to --host/--port by the Discover arm of `run`
        // before this function is reached.
        discover: _,
        subnet: _,
        timeout: _,
    } = arguments;
    if !can_prompt && name.is_none() {
        return Err(CliError::MissingPrinterName);
    }
    let interactive_wizard =
        can_prompt && (name.is_none() || transport.is_none() || host.is_none() || port.is_none());
    let transport = match transport {
        Some(transport) => transport,
        None if can_prompt => prompter.transport()?,
        None => return Err(CliError::MissingPrinterTransport),
    };
    let connection = match transport {
        PrinterTransport::Usb => {
            if host.is_some() {
                return Err(CliError::NetworkHostForUsbPrinter);
            }
            if port.is_some() {
                return Err(CliError::NetworkPortForUsbPrinter);
            }
            let selector = usb_selector(vendor_id, product_id, serial)?;
            // Without selectors, choosing a device and endpoint is a deliberate
            // act that only a terminal can perform.
            if !can_prompt && selector.is_none() {
                return Err(CliError::UsbRegistrationRequiresInteractive);
            }
            let candidates = usb_add_targets(inventory.list()?, configuration);
            ResolvedAddConnection::Usb(select_usb_target(
                candidates,
                selector.as_ref(),
                can_prompt,
                prompter,
            )?)
        }
        PrinterTransport::Network => {
            if vendor_id.is_some() || product_id.is_some() || serial.is_some() {
                return Err(CliError::UsbSelectorForNetworkPrinter);
            }
            let host = match host {
                Some(host) => host,
                None if can_prompt => prompter.host()?,
                None => return Err(CliError::MissingPrinterHost),
            };
            if host.trim().is_empty() {
                return Err(CliError::BlankPrinterHost);
            }
            let port = match port {
                Some(port) => port,
                None if can_prompt => prompter.port()?,
                None => 9100,
            };
            if port == 0 {
                return Err(CliError::InvalidPrinterPort);
            }
            ResolvedAddConnection::Network { host, port }
        }
    };
    let name = resolve_name(name, can_prompt, prompter, configuration)?;
    // `interactive_wizard` is already true for every interactive USB add, so it
    // covers the profile prompt without letting a non-interactive USB add try to
    // read from a terminal that is not there.
    let profile = match profile {
        Some(profile) => Some(profile),
        None if interactive_wizard => prompter.profile()?,
        None => None,
    };
    if profile
        .as_deref()
        .is_some_and(|profile| profile.trim().is_empty())
    {
        return Err(CliError::BlankPrinterProfile);
    }

    Ok(ResolvedAddPrinter {
        name,
        connection,
        profile,
    })
}

/// Build a USB selector from the descriptor options. Vendor and product IDs
/// identify a model together, so neither is meaningful alone; a serial number
/// only further narrows that identity.
fn usb_selector(
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    serial: Option<String>,
) -> Result<Option<UsbSelector>, CliError> {
    match (vendor_id, product_id) {
        (Some(vendor_id), Some(product_id)) => Ok(Some(UsbSelector {
            vendor_id,
            product_id,
            serial,
        })),
        (None, None) if serial.is_none() => Ok(None),
        _ => Err(CliError::IncompleteUsbSelector),
    }
}

/// Resolve the connected USB route to register. Without a selector this is an
/// interactive menu; with one the descriptor must identify exactly one route,
/// and a still-ambiguous choice of endpoint is deferred to the terminal rather
/// than guessed.
fn select_usb_target(
    candidates: Vec<UsbAddTarget>,
    selector: Option<&UsbSelector>,
    can_prompt: bool,
    prompter: &mut impl AddPrompter,
) -> Result<UsbAddTarget, CliError> {
    let Some(selector) = selector else {
        if candidates.is_empty() {
            return Err(CliError::NoUnconfiguredUsbPrinters);
        }
        return prompter.usb_printer(candidates);
    };

    let mut matched = filter_usb_targets(candidates, selector);
    match matched.len() {
        0 => Err(CliError::NoMatchingUsbPrinter),
        1 => Ok(matched.remove(0)),
        _ if can_prompt => prompter.usb_printer(matched),
        _ => Err(CliError::AmbiguousUsbPrinter),
    }
}

/// Keep only the unconfigured routes whose stable descriptor matches the
/// selector. An omitted serial matches any device of the requested model.
fn filter_usb_targets(targets: Vec<UsbAddTarget>, selector: &UsbSelector) -> Vec<UsbAddTarget> {
    targets
        .into_iter()
        .filter(|target| {
            target.vendor_id == selector.vendor_id
                && target.product_id == selector.product_id
                && selector
                    .serial
                    .as_deref()
                    .is_none_or(|serial| target.serial_number.as_deref() == Some(serial))
        })
        .collect()
}

fn resolve_name(
    explicit_name: Option<String>,
    can_prompt: bool,
    prompter: &mut impl AddPrompter,
    configuration: &PrinterConfiguration,
) -> Result<String, CliError> {
    if !can_prompt {
        let name = explicit_name.ok_or(CliError::MissingPrinterName)?;
        validate_name(&name, configuration)?;
        return Ok(name);
    }

    let mut candidate = explicit_name;
    loop {
        let name = match candidate.take() {
            Some(name) => name,
            None => prompter.name()?,
        };
        match validate_name(&name, configuration) {
            Ok(()) => return Ok(name),
            Err(error) => prompter.reject_name(&error),
        }
    }
}

fn validate_name(name: &str, configuration: &PrinterConfiguration) -> Result<(), CliError> {
    if name.trim().is_empty() {
        return Err(CliError::BlankPrinterName);
    }
    if configuration.printer(name).is_some() {
        return Err(CliError::PrinterAlreadyConfigured(name.to_owned()));
    }
    Ok(())
}

struct InquireAddPrompter;

impl AddPrompter for InquireAddPrompter {
    fn name(&mut self) -> Result<String, CliError> {
        Text::new("Printer name")
            .prompt()
            .map_err(|error| CliError::PrinterPrompt(error.to_string()))
    }

    fn transport(&mut self) -> Result<PrinterTransport, CliError> {
        Select::new(
            "Transport",
            vec![PrinterTransport::Usb, PrinterTransport::Network],
        )
        .prompt()
        .map_err(|error| CliError::PrinterPrompt(error.to_string()))
    }

    fn usb_printer(&mut self, printers: Vec<UsbAddTarget>) -> Result<UsbAddTarget, CliError> {
        Select::new("USB printer", printers)
            .prompt()
            .map_err(|error| CliError::PrinterPrompt(error.to_string()))
    }

    fn host(&mut self) -> Result<String, CliError> {
        Text::new("Network host")
            .prompt()
            .map_err(|error| CliError::PrinterPrompt(error.to_string()))
    }

    fn port(&mut self) -> Result<u16, CliError> {
        CustomType::<u16>::new("Network port")
            .with_default(9100)
            .with_error_message("Enter a port between 1 and 65535")
            .with_validator(|port: &u16| {
                Ok(if *port == 0 {
                    Validation::Invalid("Port must be between 1 and 65535".into())
                } else {
                    Validation::Valid
                })
            })
            .prompt()
            .map_err(|error| CliError::PrinterPrompt(error.to_string()))
    }

    fn profile(&mut self) -> Result<Option<String>, CliError> {
        let profile = Text::new("Printer profile (optional)")
            .with_help_message("Leave empty when the printer has not been calibrated yet")
            .prompt()
            .map_err(|error| CliError::PrinterPrompt(error.to_string()))?;
        Ok((!profile.trim().is_empty()).then(|| profile.trim().to_owned()))
    }
}

impl fmt::Display for PrinterTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usb => formatter.write_str("usb"),
            Self::Network => formatter.write_str("network"),
        }
    }
}

impl fmt::Display for UsbAddTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let model = usb_printer_label_parts(self.product.as_deref(), self.manufacturer.as_deref());
        write!(
            formatter,
            "{model} ({:04x}:{:04x};",
            self.vendor_id, self.product_id
        )?;
        if let Some(serial_number) = &self.serial_number {
            write!(formatter, " serial {serial_number};")?;
        } else {
            formatter.write_str(" no serial;")?;
        }
        write!(
            formatter,
            " bus {} address {}; interface {}; OUT {:#04x})",
            self.bus, self.address, self.interface_number, self.out_endpoint
        )
    }
}

/// The pure core of `printers list`: a registry-only inventory of configured
/// USB and network printers, each cross-checked against what is actually
/// reachable right now (`merge_usb_identities`'s metadata-only presence
/// check for USB, `network_statuses`'s TCP probe for network). A connected
/// USB device that matches no saved identity is never shown here — that is
/// `printers discover`'s job — so the merge is used only to resolve status
/// for entries that are already in `printers.toml`. USB presence never opens
/// a device: when no USB printers are configured at all, `inventory.
/// identities()` is not even called, so `list` is structurally incapable of
/// hitting a device-open permission error the way `discover` or `add` can.
fn execute(
    inventory: &mut impl UsbInventory,
    configuration: &PrinterConfiguration,
    network_statuses: &[bool],
    transport: Option<InventoryTransport>,
    output: &mut impl Write,
) -> Result<(), CliError> {
    let identities = if transport == Some(InventoryTransport::Network)
        || configuration.usb_printers().is_empty()
    {
        Vec::new()
    } else {
        inventory.identities()?
    };
    let listing = merge_usb_identities(identities, configuration);
    let mut printers = listed_printers(
        &listing,
        configuration,
        network_statuses,
        transport != Some(InventoryTransport::Usb),
    );
    if printers.is_empty() {
        writeln!(output, "No printers configured.").map_err(CliError::WriteHumanOutput)?;
        return Ok(());
    }

    printers.sort_by(|left, right| {
        left.status_rank()
            .cmp(&right.status_rank())
            .then_with(|| {
                left.display_name
                    .to_lowercase()
                    .cmp(&right.display_name.to_lowercase())
            })
            .then_with(|| left.display_name.cmp(&right.display_name))
            .then_with(|| left.transport_rank().cmp(&right.transport_rank()))
    });
    for (offset, printer) in printers.into_iter().enumerate() {
        match printer.kind {
            ListedPrinterKind::ConnectedUsb(connected) => {
                let configured = &configuration.usb_printers()[connected.configuration_index];
                write_printer(output, offset + 1, &connected.printer, configured)?;
            }
            ListedPrinterKind::UnavailableUsb(printer) => {
                write_unavailable_printer(output, offset + 1, printer)?;
            }
            ListedPrinterKind::Network { printer, connected } => {
                write_network_printer(output, offset + 1, printer, connected)?;
            }
        }
    }
    Ok(())
}

async fn probe_network_printers(printers: &[ConfiguredNetworkPrinter]) -> Vec<bool> {
    let mut probes = JoinSet::new();
    for (index, printer) in printers.iter().enumerate() {
        let host = printer.host.clone();
        let port = printer.port;
        probes.spawn(async move {
            // Opening and immediately dropping a TCP stream proves that the
            // configured RAW endpoint accepts connections without sending a
            // single byte that the printer could interpret as ESC/POS data.
            let connected = timeout(
                NETWORK_PROBE_TIMEOUT,
                TcpStream::connect((host.as_str(), port)),
            )
            .await
            .is_ok_and(|result| result.is_ok());
            (index, connected)
        });
    }

    let mut statuses = vec![false; printers.len()];
    while let Some(result) = probes.join_next().await {
        if let Ok((index, connected)) = result {
            statuses[index] = connected;
        }
    }
    statuses
}

/// A discovered endpoint offered for registration, labeled with any saved
/// printers already pointing at it.
#[derive(Debug)]
struct DiscoverChoice {
    host: DiscoveredHost,
    configured_as: Vec<String>,
}

impl fmt::Display for DiscoverChoice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}",
            format_network_endpoint(&self.host.address.to_string(), self.host.port)
        )?;
        let mut notes = Vec::new();
        if let Some(interface) = &self.host.interface {
            notes.push(format!("via {interface}"));
        }
        if !self.configured_as.is_empty() {
            notes.push(format!("configured as {}", self.configured_as.join(", ")));
        }
        if !notes.is_empty() {
            write!(formatter, " ({})", notes.join("; "))?;
        }
        Ok(())
    }
}

trait DiscoverPicker {
    fn discovered_host(&mut self, choices: Vec<DiscoverChoice>)
    -> Result<DiscoverChoice, CliError>;
}

struct InquireDiscoverPicker;

impl DiscoverPicker for InquireDiscoverPicker {
    fn discovered_host(
        &mut self,
        choices: Vec<DiscoverChoice>,
    ) -> Result<DiscoverChoice, CliError> {
        Select::new("Network printer", choices)
            .prompt()
            .map_err(|error| CliError::PrinterPrompt(error.to_string()))
    }
}

async fn discover_host_for_add(
    config_path: Option<&std::path::Path>,
    arguments: &AddPrinterArgs,
    non_interactive: bool,
) -> Result<DiscoveredHost, CliError> {
    let port = arguments.port.unwrap_or(9100);
    if port == 0 {
        return Err(CliError::InvalidPrinterPort);
    }
    let configuration = configuration::load_for_update(config_path)?;
    let targets = discovery_targets(&arguments.subnet)?;
    let hosts = discovery::scan(
        &targets,
        port,
        Duration::from_millis(arguments.timeout.unwrap_or(1000)),
    )
    .await;
    let choices = hosts
        .into_iter()
        .map(|host| {
            let configured_as = configured_names(&configuration, &host)
                .into_iter()
                .map(str::to_owned)
                .collect();
            DiscoverChoice {
                host,
                configured_as,
            }
        })
        .collect();
    let can_prompt = !non_interactive && io::stdin().is_terminal() && io::stderr().is_terminal();
    choose_discovered_host(choices, port, can_prompt, &mut InquireDiscoverPicker)
}

/// Resolve the sweep result to one endpoint. Exactly one candidate needs no
/// prompt; several candidates need a terminal, because choosing a printer
/// implicitly could register a stranger's device.
fn choose_discovered_host(
    mut choices: Vec<DiscoverChoice>,
    port: u16,
    can_prompt: bool,
    picker: &mut impl DiscoverPicker,
) -> Result<DiscoveredHost, CliError> {
    match choices.len() {
        0 => Err(CliError::NoDiscoveredPrinters(port)),
        1 => Ok(choices.remove(0).host),
        _ if can_prompt => Ok(picker.discovered_host(choices)?.host),
        _ => Err(CliError::AmbiguousDiscoveredPrinters(
            choices.iter().map(ToString::to_string).collect(),
        )),
    }
}

async fn run_discover(
    arguments: DiscoverPrintersArgs,
    configuration: &PrinterConfiguration,
) -> Result<(), CliError> {
    if arguments.transport == Some(InventoryTransport::Usb)
        && (!arguments.subnet.is_empty() || arguments.port.is_some() || arguments.timeout.is_some())
    {
        return Err(CliError::NetworkScanOptionForUsbDiscovery);
    }
    let port = arguments.port.unwrap_or(9100);
    if port == 0 {
        return Err(CliError::InvalidPrinterPort);
    }
    let hosts = if arguments.transport == Some(InventoryTransport::Usb) {
        Vec::new()
    } else {
        let targets = discovery_targets(&arguments.subnet)?;
        discovery::scan(
            &targets,
            port,
            Duration::from_millis(arguments.timeout.unwrap_or(1000)),
        )
        .await
    };
    let mut inventory = NusbInventory;
    let connected = execute_discover(
        &mut inventory,
        configuration,
        &hosts,
        arguments.transport,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    )?;
    if let Some(hint) = usb_registration_hint(&connected) {
        eprintln!("{hint}");
    }
    if let Some(hint) = registration_hint(&hosts, configuration, port) {
        eprintln!("{hint}");
    }
    Ok(())
}

/// The pure core of `printers discover`: enumerate USB (unless
/// `--transport network`) and format the sweep hosts (unless `--transport
/// usb`), printing USB blocks before network blocks with continuous
/// numbering. USB enumeration is best-effort (see `UsbInventory::
/// list_tolerant`): a device that could not be opened or inspected is
/// reported as a warning on `warnings_output` before anything is written to
/// `output`, and the rest of the sweep still runs. Returns the connected USB
/// printers so the caller can also build the USB registration hint without
/// enumerating USB devices twice.
fn execute_discover(
    inventory: &mut impl UsbInventory,
    configuration: &PrinterConfiguration,
    hosts: &[DiscoveredHost],
    transport: Option<InventoryTransport>,
    output: &mut impl Write,
    warnings_output: &mut impl Write,
) -> Result<Vec<ConnectedUsbPrinter>, CliError> {
    let connected = if transport == Some(InventoryTransport::Network) {
        Vec::new()
    } else {
        let enumeration = inventory.list_tolerant()?;
        for warning in &enumeration.warnings {
            writeln!(warnings_output, "Warning: {warning}").map_err(CliError::WriteHumanOutput)?;
        }
        discovered_usb_printers(enumeration.printers, configuration)
    };
    let hosts: &[DiscoveredHost] = if transport == Some(InventoryTransport::Usb) {
        &[]
    } else {
        hosts
    };

    if connected.is_empty() && hosts.is_empty() {
        writeln!(output, "No printers discovered.").map_err(CliError::WriteHumanOutput)?;
        return Ok(connected);
    }

    write_discovered_usb_printers(output, &connected, configuration)?;
    write_discovered_network_printers(output, hosts, configuration, connected.len() + 1)?;
    Ok(connected)
}

/// Explicit --subnet values are scanned exactly as given; without them the
/// sweep covers every small directly connected network.
fn discovery_targets(subnets: &[Subnet]) -> Result<Vec<ScanTarget>, CliError> {
    if subnets.is_empty() {
        let targets = discovery::local_scan_targets()?;
        if targets.is_empty() {
            return Err(CliError::NoDiscoverableSubnets);
        }
        return Ok(targets);
    }
    Ok(subnets
        .iter()
        .map(|subnet| ScanTarget {
            subnet: *subnet,
            interface: None,
            excluded: None,
        })
        .collect())
}

/// Write each connected USB printer's block, numbered from 1. New printers
/// (no matching configuration) head their block with the descriptor-derived
/// label, `status: new`, and no `model:`/`profile:` lines; configured
/// printers head it with the saved name, `status: configured`, and both
/// lines, falling back to `unassigned` like `printers list`.
fn write_discovered_usb_printers(
    output: &mut impl Write,
    connected: &[ConnectedUsbPrinter],
    configuration: &PrinterConfiguration,
) -> Result<(), CliError> {
    for (offset, connected) in connected.iter().enumerate() {
        let configured = connected
            .configuration_index
            .map(|index| &configuration.usb_printers()[index]);
        let model = usb_printer_label(&connected.printer);
        let listing = match configured {
            Some(configured) => UsbListing {
                heading: &configured.name,
                status: "configured",
                model: Some(model.as_str()),
                profile: Some(configured.profile.as_deref()),
                printer: &connected.printer,
            },
            None => UsbListing {
                heading: &model,
                status: "new",
                model: None,
                profile: None,
                printer: &connected.printer,
            },
        };
        write_usb_listing(output, offset + 1, &listing)?;
    }
    Ok(())
}

/// Write each discovered network host's block, numbered starting at `start`
/// so USB blocks (numbered 1..) can precede it in the combined listing.
fn write_discovered_network_printers(
    output: &mut impl Write,
    hosts: &[DiscoveredHost],
    configuration: &PrinterConfiguration,
    start: usize,
) -> Result<(), CliError> {
    for (offset, host) in hosts.iter().enumerate() {
        let address = host.address.to_string();
        let endpoint = format_network_endpoint(&address, host.port);
        let matches = configured_network_printers(configuration, host);
        let also_configured: Vec<&str> = matches
            .iter()
            .skip(1)
            .map(|printer| printer.name.as_str())
            .collect();
        let listing = if let Some(first) = matches.first() {
            NetworkListing {
                heading: &first.name,
                status: "configured",
                profile: Some(first.profile.as_deref()),
                host: &address,
                port: host.port,
                interface: host.interface.as_deref(),
                also_configured: &also_configured,
            }
        } else {
            NetworkListing {
                heading: &endpoint,
                status: "new",
                profile: None,
                host: &address,
                port: host.port,
                interface: host.interface.as_deref(),
                also_configured: &[],
            }
        };
        write_network_listing(output, start + offset, &listing)?;
    }
    Ok(())
}

/// Saved network printers matching a discovered endpoint, in configuration
/// order. Matching is textual on host and exact on port; saved hostnames
/// never match.
fn configured_network_printers<'a>(
    configuration: &'a PrinterConfiguration,
    host: &DiscoveredHost,
) -> Vec<&'a ConfiguredNetworkPrinter> {
    configuration
        .network_printers()
        .iter()
        .filter(|printer| printer.port == host.port && printer.host == host.address.to_string())
        .collect()
}

/// Names of saved network printers matching a discovered endpoint.
fn configured_names<'a>(
    configuration: &'a PrinterConfiguration,
    host: &DiscoveredHost,
) -> Vec<&'a str> {
    configured_network_printers(configuration, host)
        .into_iter()
        .map(|printer| printer.name.as_str())
        .collect()
}

/// A one-line nudge toward registering a freshly discovered printer, printed
/// to stderr after the listing. `None` when every discovered host is already
/// configured, including an empty sweep. The hint always points at `--discover`
/// rather than a concrete `--host`, regardless of how many new hosts were
/// found: `add --discover` already auto-selects a single discovered host and
/// opens the picker for several, so one command covers both cases.
fn registration_hint(
    hosts: &[DiscoveredHost],
    configuration: &PrinterConfiguration,
    port: u16,
) -> Option<String> {
    let any_new_host = hosts
        .iter()
        .any(|host| configured_names(configuration, host).is_empty());
    if !any_new_host {
        return None;
    }
    let port_suffix = if port == 9100 {
        String::new()
    } else {
        format!(" --port {port}")
    };
    Some(format!(
        "Register a new printer with: escpost printers add <NAME> --transport network --discover{port_suffix}"
    ))
}

/// A one-line nudge toward registering a freshly discovered USB printer,
/// mirroring `registration_hint`'s network counterpart: printed before it,
/// on stderr, and `None` unless at least one connected USB printer does not
/// yet match a saved identity. Count-independent for the same reason —
/// there is exactly one non-interactive way to register a USB printer
/// regardless of how many new ones were found.
fn usb_registration_hint(connected: &[ConnectedUsbPrinter]) -> Option<&'static str> {
    connected
        .iter()
        .any(|printer| printer.configuration_index.is_none())
        .then_some("Register a new printer with: escpost printers add <NAME> --transport usb")
}

fn listed_printers<'a>(
    usb: &'a MergedUsbIdentities,
    configuration: &'a PrinterConfiguration,
    network_statuses: &[bool],
    include_network: bool,
) -> Vec<ListedPrinter<'a>> {
    let mut printers = Vec::new();
    // `printers list` is the registry, not a discovery tool: a connected USB
    // device that matches no saved identity is `printers discover`'s
    // business now, so `merge_usb_identities` has already dropped it before
    // this function ever sees it.
    for connected in &usb.connected {
        let configured = &configuration.usb_printers()[connected.configuration_index];
        printers.push(ListedPrinter {
            display_name: configured.name.clone(),
            kind: ListedPrinterKind::ConnectedUsb(connected),
        });
    }
    for index in &usb.unavailable_configuration_indexes {
        let printer = &configuration.usb_printers()[*index];
        printers.push(ListedPrinter {
            display_name: printer.name.clone(),
            kind: ListedPrinterKind::UnavailableUsb(printer),
        });
    }
    if include_network {
        for (index, printer) in configuration.network_printers().iter().enumerate() {
            printers.push(ListedPrinter {
                display_name: printer.name.clone(),
                kind: ListedPrinterKind::Network {
                    printer,
                    connected: network_statuses.get(index).copied().unwrap_or(false),
                },
            });
        }
    }
    printers
}

impl ListedPrinter<'_> {
    fn status_rank(&self) -> u8 {
        match self.kind {
            ListedPrinterKind::ConnectedUsb(_)
            | ListedPrinterKind::Network {
                connected: true, ..
            } => 0,
            ListedPrinterKind::UnavailableUsb(_)
            | ListedPrinterKind::Network {
                connected: false, ..
            } => 1,
        }
    }

    fn transport_rank(&self) -> u8 {
        match self.kind {
            ListedPrinterKind::ConnectedUsb(_) | ListedPrinterKind::UnavailableUsb(_) => 0,
            ListedPrinterKind::Network { .. } => 1,
        }
    }
}

/// Sort connected USB printers by stable location. Both `printers list`'s
/// merge and `printers discover`'s classification rely on this order to make
/// first-match-wins configuration assignment deterministic across runs,
/// regardless of the order the operating system enumerates devices.
fn sort_by_usb_location(printers: &mut [UsbPrinter]) {
    printers.sort_by(|left, right| {
        (
            &left.bus,
            left.address,
            left.interface_number,
            left.vendor_id,
            left.product_id,
        )
            .cmp(&(
                &right.bus,
                right.address,
                right.interface_number,
                right.vendor_id,
                right.product_id,
            ))
    });
}

/// Match each connected USB printer against at most one configured identity,
/// first-match-wins in `printers` order. This keeps one saved alias from
/// naming several identical connected interfaces when the configuration has
/// no serial number. `printers` must already be sorted by stable USB
/// location (`sort_by_usb_location`) so the match is deterministic across
/// runs. Returns the connected printers alongside which configuration
/// indexes were claimed, so callers can also report unclaimed ones.
fn classify_usb_printers(
    printers: Vec<UsbPrinter>,
    configuration: &PrinterConfiguration,
) -> (Vec<ConnectedUsbPrinter>, Vec<bool>) {
    let mut matched_configurations = vec![false; configuration.usb_printers().len()];
    let mut connected = Vec::with_capacity(printers.len());
    for printer in printers {
        let matching_configurations = configuration
            .usb_printers()
            .iter()
            .enumerate()
            .filter(|(_, configured)| configuration_matches(&printer, configured))
            .collect::<Vec<_>>();
        let primary_configuration = matching_configurations
            .iter()
            .filter(|(index, _)| !matched_configurations[*index])
            .min_by(|(_, left), (_, right)| compare_display_names(&left.name, &right.name))
            .map(|(index, _)| *index);
        if primary_configuration.is_some() {
            for (configuration_index, _) in matching_configurations {
                matched_configurations[configuration_index] = true;
            }
        }
        connected.push(ConnectedUsbPrinter {
            printer,
            configuration_index: primary_configuration,
        });
    }
    (connected, matched_configurations)
}

/// The list-specific analogue of `configuration_matches`: whether an
/// OS-reported device identity (no interface or endpoint data available
/// without opening the device) satisfies a saved USB printer. Serial
/// semantics mirror `configuration_matches` exactly: an unset saved serial
/// matches any identity of that vendor/product, a set one requires an exact
/// match.
fn identity_matches_configuration(
    identity: &UsbDeviceIdentity,
    configured: &ConfiguredUsbPrinter,
) -> bool {
    configured.vendor_id == identity.vendor_id
        && configured.product_id == identity.product_id
        && configured
            .serial_number
            .as_ref()
            .is_none_or(|serial| identity.serial_number.as_ref() == Some(serial))
}

/// Sort device identities by stable location, the identity-level analogue of
/// `sort_by_usb_location`. `merge_usb_identities` relies on this order for
/// the same reason `classify_usb_printers` relies on `sort_by_usb_location`:
/// deterministic first-match-wins configuration assignment regardless of the
/// order the operating system enumerates devices.
fn sort_by_usb_identity_location(identities: &mut [UsbDeviceIdentity]) {
    identities.sort_by(|left, right| {
        (&left.bus, left.address, left.vendor_id, left.product_id).cmp(&(
            &right.bus,
            right.address,
            right.vendor_id,
            right.product_id,
        ))
    });
}

/// Compose the printer shown for one matched `list` entry: live location and
/// descriptor strings come from the OS-reported identity (never opened),
/// while interface and endpoint routing come from the saved configuration —
/// `list` never reads endpoints from the device itself, only from
/// `printers.toml`. The serial line prefers the identity's serial (today's
/// live value) and falls back to the configured one so an entry matched by
/// an unset configured serial still shows the connected device's own serial
/// when it has one.
fn connected_usb_printer(
    identity: UsbDeviceIdentity,
    configured: &ConfiguredUsbPrinter,
) -> UsbPrinter {
    UsbPrinter {
        vendor_id: identity.vendor_id,
        product_id: identity.product_id,
        bus: identity.bus,
        address: identity.address,
        manufacturer: identity.manufacturer,
        product: identity.product,
        serial_number: identity
            .serial_number
            .or_else(|| configured.serial_number.clone()),
        interface_number: configured.interface_number,
        out_endpoints: vec![configured.out_endpoint],
        in_endpoints: configured.in_endpoint.into_iter().collect(),
    }
}

/// The list-specific analogue of `classify_usb_printers`: match each
/// metadata-only device identity against the saved USB configuration,
/// first-match-wins by stable location exactly like that function, then
/// sort the connected results by display name. An identity matching no
/// saved configuration is dropped outright — unlike `printers discover`,
/// `list` never shows a connected-but-unconfigured USB device — and a saved
/// printer claimed by an identity that lost the first-match-wins tiebreak
/// to a sibling configuration is neither connected nor unavailable,
/// mirroring `classify_usb_printers`' own ambiguity handling.
fn merge_usb_identities(
    mut identities: Vec<UsbDeviceIdentity>,
    configuration: &PrinterConfiguration,
) -> MergedUsbIdentities {
    sort_by_usb_identity_location(&mut identities);
    let mut matched_configurations = vec![false; configuration.usb_printers().len()];
    let mut connected = Vec::new();
    for identity in identities {
        let matching_configurations = configuration
            .usb_printers()
            .iter()
            .enumerate()
            .filter(|(_, configured)| identity_matches_configuration(&identity, configured))
            .collect::<Vec<_>>();
        let primary_configuration = matching_configurations
            .iter()
            .filter(|(index, _)| !matched_configurations[*index])
            .min_by(|(_, left), (_, right)| compare_display_names(&left.name, &right.name))
            .map(|(index, _)| *index);
        let Some(configuration_index) = primary_configuration else {
            continue;
        };
        for (index, _) in matching_configurations {
            matched_configurations[index] = true;
        }
        let printer =
            connected_usb_printer(identity, &configuration.usb_printers()[configuration_index]);
        connected.push(ConnectedUsbEntry {
            printer,
            configuration_index,
        });
    }
    connected.sort_by_cached_key(|connected| {
        let name = &configuration.usb_printers()[connected.configuration_index].name;
        (name.to_lowercase(), name.clone())
    });
    let mut unavailable_configuration_indexes = configuration
        .usb_printers()
        .iter()
        .enumerate()
        .filter(|(index, _)| !matched_configurations[*index])
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    unavailable_configuration_indexes.sort_by(|left, right| {
        compare_display_names(
            &configuration.usb_printers()[*left].name,
            &configuration.usb_printers()[*right].name,
        )
    });

    MergedUsbIdentities {
        connected,
        unavailable_configuration_indexes,
    }
}

/// Classify connected USB printers for `printers discover`. Unlike
/// `merge_usb_identities`, this keeps the stable-location order instead of
/// re-sorting by display name afterward: `list` groups by name, but
/// `discover`'s USB block simply reports every connected printer as it is
/// found, the same way its network sweep already reports hosts in scan
/// order. Printers not claimed by any configuration are left `None`, ready
/// to be reported as newly discovered.
fn discovered_usb_printers(
    mut printers: Vec<UsbPrinter>,
    configuration: &PrinterConfiguration,
) -> Vec<ConnectedUsbPrinter> {
    sort_by_usb_location(&mut printers);
    classify_usb_printers(printers, configuration).0
}

fn usb_add_targets(
    printers: Vec<UsbPrinter>,
    configuration: &PrinterConfiguration,
) -> Vec<UsbAddTarget> {
    let unconfigured = printers
        .into_iter()
        .filter(|printer| {
            !configuration
                .usb_printers()
                .iter()
                .any(|configured| configuration_matches(printer, configured))
        })
        .collect::<Vec<_>>();
    let mut targets = Vec::new();

    for printer in &unconfigured {
        // Bus and address are useful for distinguishing devices in this
        // one-time menu, but the operating system may assign new values after
        // reconnecting. The saved identity therefore uses stable descriptors.
        let ambiguous_without_serial = printer.serial_number.is_none()
            && unconfigured.iter().any(|other| {
                other.vendor_id == printer.vendor_id
                    && other.product_id == printer.product_id
                    && (other.bus != printer.bus || other.address != printer.address)
            });
        let in_endpoint = (printer.in_endpoints.len() == 1).then(|| printer.in_endpoints[0]);

        // Most printers expose one bulk OUT endpoint. If firmware exposes
        // several, present each as a separate explicit choice rather than
        // silently choosing a route that may not carry print data.
        for out_endpoint in &printer.out_endpoints {
            targets.push(UsbAddTarget {
                vendor_id: printer.vendor_id,
                product_id: printer.product_id,
                bus: printer.bus.clone(),
                address: printer.address,
                manufacturer: printer.manufacturer.clone(),
                product: printer.product.clone(),
                serial_number: printer.serial_number.clone(),
                interface_number: printer.interface_number,
                out_endpoint: *out_endpoint,
                in_endpoint,
                ambiguous_without_serial,
            });
        }
    }

    targets.sort_by_cached_key(|target| {
        let label =
            usb_printer_label_parts(target.product.as_deref(), target.manufacturer.as_deref());
        (
            label.to_lowercase(),
            label,
            target.bus.clone(),
            target.address,
            target.interface_number,
            target.out_endpoint,
        )
    });
    targets
}

struct NusbInventory;

impl UsbInventory for NusbInventory {
    fn list(&mut self) -> Result<Vec<UsbPrinter>, CliError> {
        let devices = nusb::list_devices()
            .wait()
            .map_err(CliError::EnumerateUsb)?;
        let mut printers = Vec::new();

        // Filter with operating-system metadata first. Listing should never
        // open unrelated USB devices merely to find their interface classes.
        for device_info in devices.filter(is_printer_device) {
            printers.extend(usb_printers_for_device(&device_info)?);
        }

        Ok(printers)
    }

    fn list_tolerant(&mut self) -> Result<UsbEnumeration, CliError> {
        let devices = nusb::list_devices()
            .wait()
            .map_err(CliError::EnumerateUsb)?;
        let mut printers = Vec::new();
        let mut warnings = Vec::new();

        for device_info in devices.filter(is_printer_device) {
            match usb_printers_for_device(&device_info) {
                Ok(device_printers) => printers.extend(device_printers),
                Err(error) => warnings.push(describe_usb_enumeration_failure(&error)),
            }
        }

        Ok(UsbEnumeration { printers, warnings })
    }

    fn identities(&mut self) -> Result<Vec<UsbDeviceIdentity>, CliError> {
        let devices = nusb::list_devices()
            .wait()
            .map_err(CliError::EnumerateUsb)?;

        // Operating-system device metadata only: no `.open()` anywhere in
        // this path, so `printers list` cannot fail with a permission error
        // the way opening a device for `discover` or `add` can.
        Ok(devices
            .filter(is_printer_device)
            .map(|device_info| UsbDeviceIdentity {
                vendor_id: device_info.vendor_id(),
                product_id: device_info.product_id(),
                bus: device_info.bus_id().to_owned(),
                address: device_info.device_address(),
                manufacturer: device_info.manufacturer_string().map(str::to_owned),
                product: device_info.product_string().map(str::to_owned),
                serial_number: device_info.serial_number().map(str::to_owned),
            })
            .collect())
    }
}

/// Open one USB device and collect the printer-class interfaces it exposes.
/// Shared by `list()`'s strict enumeration (used by `printers list` and
/// `printers add`, where a device failure aborts the whole command) and
/// `list_tolerant()`'s best-effort enumeration (used by `printers discover`,
/// where a device failure becomes a warning and enumeration continues).
fn usb_printers_for_device(device_info: &nusb::DeviceInfo) -> Result<Vec<UsbPrinter>, CliError> {
    let device = device_info
        .open()
        .wait()
        .map_err(|source| CliError::OpenUsbDevice {
            vendor_id: device_info.vendor_id(),
            product_id: device_info.product_id(),
            source,
        })?;
    let configuration =
        device
            .active_configuration()
            .map_err(|source| CliError::InspectUsbConfiguration {
                vendor_id: device_info.vendor_id(),
                product_id: device_info.product_id(),
                source,
            })?;

    Ok(printer_interfaces(configuration)
        .into_iter()
        .map(|interface| UsbPrinter {
            vendor_id: device_info.vendor_id(),
            product_id: device_info.product_id(),
            bus: device_info.bus_id().to_owned(),
            address: device_info.device_address(),
            manufacturer: device_info.manufacturer_string().map(str::to_owned),
            product: device_info.product_string().map(str::to_owned),
            serial_number: device_info.serial_number().map(str::to_owned),
            interface_number: interface.interface_number,
            out_endpoints: interface.out_endpoints,
            in_endpoints: interface.in_endpoints,
        })
        .collect())
}

/// Render a USB enumeration failure gathered by `list_tolerant` as a warning
/// line for `printers discover`, using the same bare `vendor:product` hex
/// notation as discover's own `usb:` coordinate line. `CliError`'s Display
/// uses a `0x`-prefixed form instead, which stays unchanged for the fatal
/// case where `list()` itself propagates the error (`printers list`,
/// `printers add`).
fn describe_usb_enumeration_failure(error: &CliError) -> String {
    match error {
        CliError::OpenUsbDevice {
            vendor_id,
            product_id,
            source,
        } => format!("could not open USB device {vendor_id:04x}:{product_id:04x}: {source}"),
        CliError::InspectUsbConfiguration {
            vendor_id,
            product_id,
            source,
        } => format!(
            "could not inspect the active configuration of USB device {vendor_id:04x}:{product_id:04x}: {source}"
        ),
        other => other.to_string(),
    }
}

fn is_printer_device(device: &nusb::DeviceInfo) -> bool {
    device.class() == USB_CLASS_PRINTER
        || device
            .interfaces()
            .any(|interface| interface.class() == USB_CLASS_PRINTER)
}

/// A USB printer entry as shown by both `printers list` and `printers
/// discover`, mirroring `NetworkListing` below so the two commands cannot
/// drift apart. `model` distinguishes "no model line" (an unconfigured
/// connected printer) from "print the model line" (a configured printer).
/// `profile` distinguishes "no profile line at all" (a freshly discovered,
/// unconfigured printer on `discover`) from "print the line, falling back to
/// `unassigned`" (a configured printer on either command, or an unconfigured
/// but connected printer on `list`).
struct UsbListing<'a> {
    heading: &'a str,
    status: &'a str,
    model: Option<&'a str>,
    profile: Option<Option<&'a str>>,
    printer: &'a UsbPrinter,
}

fn write_usb_listing(
    output: &mut impl Write,
    number: usize,
    listing: &UsbListing<'_>,
) -> Result<(), CliError> {
    writeln!(output, "[{number}] {}", listing.heading).map_err(CliError::WriteHumanOutput)?;
    writeln!(output, "    status: {}", listing.status).map_err(CliError::WriteHumanOutput)?;
    if let Some(model) = listing.model {
        writeln!(output, "    model: {model}").map_err(CliError::WriteHumanOutput)?;
    }
    if let Some(profile) = listing.profile {
        writeln!(
            output,
            "    profile: {}",
            profile.unwrap_or(UNASSIGNED_PROFILE)
        )
        .map_err(CliError::WriteHumanOutput)?;
    }
    writeln!(output, "    transport: usb").map_err(CliError::WriteHumanOutput)?;
    writeln!(
        output,
        "    usb: {:04x}:{:04x}; bus {} address {}; interface {}",
        listing.printer.vendor_id,
        listing.printer.product_id,
        listing.printer.bus,
        listing.printer.address,
        listing.printer.interface_number
    )
    .map_err(CliError::WriteHumanOutput)?;
    write!(
        output,
        "    endpoints: out {}",
        format_endpoints(&listing.printer.out_endpoints)
    )
    .map_err(CliError::WriteHumanOutput)?;
    if !listing.printer.in_endpoints.is_empty() {
        write!(
            output,
            "; in {}",
            format_endpoints(&listing.printer.in_endpoints)
        )
        .map_err(CliError::WriteHumanOutput)?;
    }
    writeln!(output).map_err(CliError::WriteHumanOutput)?;
    if let Some(serial_number) = &listing.printer.serial_number {
        writeln!(output, "    serial: {serial_number}").map_err(CliError::WriteHumanOutput)?;
    }
    Ok(())
}

/// Write one `list` connected-USB block. Every caller already has a matched
/// configuration (`merge_usb_identities` drops anything unmatched), so
/// `configured` is not optional here, unlike the discover-side listing. The
/// `model:` line is omitted rather than falling back to a generic label when
/// the device identity itself carries no product string, matching
/// `write_usb_listing`'s own `model: None` handling.
fn write_printer(
    output: &mut impl Write,
    number: usize,
    printer: &UsbPrinter,
    configured: &ConfiguredUsbPrinter,
) -> Result<(), CliError> {
    let model = printer
        .product
        .as_deref()
        .map(|_| usb_printer_label(printer));
    write_usb_listing(
        output,
        number,
        &UsbListing {
            heading: &configured.name,
            status: "connected",
            model: model.as_deref(),
            profile: Some(configured.profile.as_deref()),
            printer,
        },
    )
}

fn write_unavailable_printer(
    output: &mut impl Write,
    number: usize,
    printer: &ConfiguredUsbPrinter,
) -> Result<(), CliError> {
    writeln!(output, "[{number}] {}", printer.name).map_err(CliError::WriteHumanOutput)?;
    writeln!(output, "    status: unavailable").map_err(CliError::WriteHumanOutput)?;
    writeln!(
        output,
        "    profile: {}",
        printer.profile.as_deref().unwrap_or(UNASSIGNED_PROFILE)
    )
    .map_err(CliError::WriteHumanOutput)?;
    writeln!(output, "    transport: usb").map_err(CliError::WriteHumanOutput)?;
    writeln!(
        output,
        "    usb: {:04x}:{:04x}; interface {}",
        printer.vendor_id, printer.product_id, printer.interface_number
    )
    .map_err(CliError::WriteHumanOutput)?;
    write!(output, "    endpoints: out {:#04x}", printer.out_endpoint)
        .map_err(CliError::WriteHumanOutput)?;
    if let Some(in_endpoint) = printer.in_endpoint {
        write!(output, "; in {in_endpoint:#04x}").map_err(CliError::WriteHumanOutput)?;
    }
    writeln!(output).map_err(CliError::WriteHumanOutput)?;
    if let Some(serial_number) = &printer.serial_number {
        writeln!(output, "    serial: {serial_number}").map_err(CliError::WriteHumanOutput)?;
    }
    Ok(())
}

/// A network printer entry as shown by both `printers list` and `printers
/// discover`, so the two commands cannot drift apart. `profile` distinguishes
/// "no profile line at all" (a freshly discovered, unconfigured host) from
/// "print the line, falling back to `unassigned`" (a configured printer).
struct NetworkListing<'a> {
    heading: &'a str,
    status: &'a str,
    profile: Option<Option<&'a str>>,
    host: &'a str,
    port: u16,
    interface: Option<&'a str>,
    also_configured: &'a [&'a str],
}

fn write_network_listing(
    output: &mut impl Write,
    number: usize,
    listing: &NetworkListing<'_>,
) -> Result<(), CliError> {
    writeln!(output, "[{number}] {}", listing.heading).map_err(CliError::WriteHumanOutput)?;
    writeln!(output, "    status: {}", listing.status).map_err(CliError::WriteHumanOutput)?;
    if let Some(profile) = listing.profile {
        writeln!(
            output,
            "    profile: {}",
            profile.unwrap_or(UNASSIGNED_PROFILE)
        )
        .map_err(CliError::WriteHumanOutput)?;
    }
    writeln!(output, "    transport: network").map_err(CliError::WriteHumanOutput)?;
    writeln!(
        output,
        "    network: {}",
        format_network_endpoint(listing.host, listing.port)
    )
    .map_err(CliError::WriteHumanOutput)?;
    if let Some(interface) = listing.interface {
        writeln!(output, "    interface: {interface}").map_err(CliError::WriteHumanOutput)?;
    }
    for name in listing.also_configured {
        writeln!(output, "    also configured as: {name}").map_err(CliError::WriteHumanOutput)?;
    }
    Ok(())
}

fn write_network_printer(
    output: &mut impl Write,
    number: usize,
    printer: &ConfiguredNetworkPrinter,
    connected: bool,
) -> Result<(), CliError> {
    write_network_listing(
        output,
        number,
        &NetworkListing {
            heading: &printer.name,
            status: if connected {
                "connected"
            } else {
                "unavailable"
            },
            profile: Some(printer.profile.as_deref()),
            host: &printer.host,
            port: printer.port,
            interface: None,
            also_configured: &[],
        },
    )
}

fn format_network_endpoint(host: &str, port: u16) -> String {
    if host.contains(':') && !(host.starts_with('[') && host.ends_with(']')) {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn configuration_matches(printer: &UsbPrinter, configured: &ConfiguredUsbPrinter) -> bool {
    configured.vendor_id == printer.vendor_id
        && configured.product_id == printer.product_id
        && configured.interface_number == printer.interface_number
        && printer.out_endpoints.contains(&configured.out_endpoint)
        && configured
            .serial_number
            .as_ref()
            .is_none_or(|serial| printer.serial_number.as_ref() == Some(serial))
}

fn compare_display_names(left: &str, right: &str) -> std::cmp::Ordering {
    left.to_lowercase()
        .cmp(&right.to_lowercase())
        .then_with(|| left.cmp(right))
}

fn usb_printer_label(printer: &UsbPrinter) -> String {
    usb_printer_label_parts(printer.product.as_deref(), printer.manufacturer.as_deref())
}

fn usb_printer_label_parts(product: Option<&str>, manufacturer: Option<&str>) -> String {
    let product = product.unwrap_or("USB printer");
    manufacturer.map_or_else(
        || product.to_owned(),
        |value| format!("{product} ({value})"),
    )
}

fn format_endpoints(endpoints: &[u8]) -> String {
    endpoints
        .iter()
        .map(|endpoint| format!("{endpoint:#04x}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn printer_interfaces(configuration: ConfigurationDescriptor<'_>) -> Vec<UsbPrinterInterface> {
    configuration
        .interface_alt_settings()
        .filter(|interface| {
            // The print command does not change alternate settings. Only show
            // endpoints that will exist immediately after claiming an
            // interface in its standard alternate setting.
            interface.class() == USB_CLASS_PRINTER && interface.alternate_setting() == 0
        })
        .filter_map(|interface| {
            let mut out_endpoints = Vec::new();
            let mut in_endpoints = Vec::new();
            for endpoint in interface
                .endpoints()
                .filter(|endpoint| endpoint.transfer_type() == TransferType::Bulk)
            {
                match endpoint.direction() {
                    Direction::Out => out_endpoints.push(endpoint.address()),
                    Direction::In => in_endpoints.push(endpoint.address()),
                }
            }
            out_endpoints.sort_unstable();
            in_endpoints.sort_unstable();

            (!out_endpoints.is_empty()).then_some(UsbPrinterInterface {
                interface_number: interface.interface_number(),
                out_endpoints,
                in_endpoints,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        AddPrompter, ConnectedUsbPrinter, DiscoverChoice, DiscoverPicker, ResolvedAddConnection,
        ResolvedAddPrinter, UsbAddTarget, UsbDeviceIdentity, UsbEnumeration, UsbInventory,
        UsbPrinter, UsbPrinterInterface, UsbSelector, choose_discovered_host, execute, execute_add,
        execute_discover, filter_usb_targets, printer_interfaces, registration_hint, resolve_add,
        select_usb_target, usb_add_targets, usb_registration_hint, usb_selector,
        write_discovered_network_printers,
    };
    use crate::cli::{AddPrinterArgs, InventoryTransport, PrinterTransport};
    use crate::configuration::PrinterConfiguration;
    use crate::discovery::DiscoveredHost;
    use crate::error::CliError;
    use std::net::Ipv4Addr;

    #[test]
    fn interactive_network_add_prompts_for_the_port() {
        let arguments = AddPrinterArgs {
            name: None,
            transport: None,
            host: None,
            port: None,
            vendor_id: None,
            product_id: None,
            serial: None,
            profile: None,
            discover: false,
            subnet: Vec::new(),
            timeout: None,
        };
        let mut prompter = FixedAddPrompter::with_names(["kitchen"]);

        let resolved = resolve_add(
            arguments,
            true,
            &mut prompter,
            &mut FixedInventory {
                printers: Vec::new(),
            },
            &PrinterConfiguration::default(),
        )
        .expect("interactive values should resolve");

        assert_eq!(
            resolved,
            ResolvedAddPrinter {
                name: "kitchen".to_owned(),
                connection: ResolvedAddConnection::Network {
                    host: "10.42.0.71".to_owned(),
                    port: 9200,
                },
                profile: Some("REFERENCE".to_owned()),
            }
        );
        assert_eq!(prompter.port_prompts, 1);
    }

    #[test]
    fn explicit_network_port_skips_port_and_profile_prompts() {
        let arguments = AddPrinterArgs {
            name: Some("kitchen".to_owned()),
            transport: Some(PrinterTransport::Network),
            host: Some("10.42.0.71".to_owned()),
            port: Some(9100),
            vendor_id: None,
            product_id: None,
            serial: None,
            profile: None,
            discover: false,
            subnet: Vec::new(),
            timeout: None,
        };

        let resolved = resolve_add(
            arguments,
            true,
            &mut UnexpectedAddPrompter,
            &mut FixedInventory {
                printers: Vec::new(),
            },
            &PrinterConfiguration::default(),
        )
        .expect("complete explicit values should resolve");

        assert_eq!(resolved.profile, None);
    }

    #[test]
    fn usb_add_rejects_an_explicit_network_port() {
        let arguments = AddPrinterArgs {
            name: Some("counter".to_owned()),
            transport: Some(PrinterTransport::Usb),
            host: None,
            port: Some(9100),
            vendor_id: None,
            product_id: None,
            serial: None,
            profile: None,
            discover: false,
            subnet: Vec::new(),
            timeout: None,
        };

        let error = resolve_add(
            arguments,
            true,
            &mut UnexpectedAddPrompter,
            &mut FixedInventory {
                printers: Vec::new(),
            },
            &PrinterConfiguration::default(),
        )
        .expect_err("a USB configuration must not accept network coordinates");

        assert!(matches!(error, CliError::NetworkPortForUsbPrinter));
    }

    #[test]
    fn interactive_add_reprompts_when_its_explicit_name_already_exists() {
        let configuration = PrinterConfiguration::parse(
            r#"
[kitchen]
transport = "network"
host = "10.42.0.20"
port = 9100
"#,
        )
        .expect("the existing printer should parse");
        let arguments = AddPrinterArgs {
            name: Some("kitchen".to_owned()),
            transport: Some(PrinterTransport::Network),
            host: Some("10.42.0.71".to_owned()),
            port: Some(9100),
            vendor_id: None,
            product_id: None,
            serial: None,
            profile: Some("REFERENCE".to_owned()),
            discover: false,
            subnet: Vec::new(),
            timeout: None,
        };
        let mut prompter = FixedAddPrompter::with_names(["counter"]);

        let resolved = resolve_add(
            arguments,
            true,
            &mut prompter,
            &mut FixedInventory {
                printers: Vec::new(),
            },
            &configuration,
        )
        .expect("a second unique name should continue registration");

        assert_eq!(resolved.name, "counter");
        assert_eq!(
            prompter.rejected_names,
            vec!["printer \"kitchen\" is already configured"]
        );
        assert_eq!(prompter.port_prompts, 0);
    }

    #[test]
    fn interactive_usb_add_saves_the_selected_descriptor_coordinates() {
        let directory = temporary_directory("add-usb");
        let configuration = directory.join("printers.toml");
        let arguments = AddPrinterArgs {
            name: None,
            transport: None,
            host: None,
            port: None,
            vendor_id: None,
            product_id: None,
            serial: None,
            profile: None,
            discover: false,
            subnet: Vec::new(),
            timeout: None,
        };
        let mut inventory = FixedInventory {
            printers: vec![UsbPrinter {
                vendor_id: 0x0416,
                product_id: 0x5011,
                bus: "003".to_owned(),
                address: 60,
                manufacturer: Some("YICHIP3121".to_owned()),
                product: Some("USB Portable Printer".to_owned()),
                serial_number: Some("B120300001".to_owned()),
                interface_number: 0,
                out_endpoints: vec![0x01],
                in_endpoints: vec![0x81],
            }],
        };

        let name = execute_add(
            Some(&configuration),
            arguments,
            true,
            &mut UsbAddPrompter,
            &mut inventory,
        )
        .expect("the selected USB printer should be saved");

        assert_eq!(name, "counter-usb");
        let document = fs::read_to_string(&configuration)
            .expect("the printer configuration should be readable");
        let table =
            toml::from_str::<toml::Table>(&document).expect("the configuration should be TOML");
        let printer = table["counter-usb"]
            .as_table()
            .expect("the configured printer should be a table");
        assert_eq!(printer["transport"].as_str(), Some("usb"));
        assert_eq!(printer["profile"].as_str(), Some("REFERENCE"));
        assert_eq!(printer["vendor_id"].as_str(), Some("0x0416"));
        assert_eq!(printer["product_id"].as_str(), Some("0x5011"));
        assert_eq!(printer["serial_number"].as_str(), Some("B120300001"));
        assert_eq!(printer["interface_number"].as_integer(), Some(0));
        assert_eq!(printer["out_endpoint"].as_str(), Some("0x01"));
        assert_eq!(printer["in_endpoint"].as_str(), Some("0x81"));
        fs::remove_dir_all(directory).expect("the test directory should be removable");
    }

    #[test]
    fn configured_usb_printers_are_not_offered_for_addition() {
        let configuration = PrinterConfiguration::parse(
            r#"
[counter]
transport = "usb"
vendor_id = "0x0416"
product_id = "0x5011"
serial_number = "B120300001"
interface_number = 0
out_endpoint = "0x01"
"#,
        )
        .expect("the saved printer should parse");

        let targets = usb_add_targets(
            vec![netum_usb_printer(vec![0x01], vec![0x81])],
            &configuration,
        );

        assert!(
            targets.is_empty(),
            "a connected printer already represented by the configuration must not be offered again"
        );
    }

    #[test]
    fn every_bulk_out_endpoint_is_an_explicit_usb_add_choice() {
        let targets = usb_add_targets(
            vec![netum_usb_printer(vec![0x01, 0x02], vec![0x81, 0x82])],
            &PrinterConfiguration::default(),
        );

        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].out_endpoint, 0x01);
        assert_eq!(targets[1].out_endpoint, 0x02);
        assert_eq!(
            targets[0].in_endpoint, None,
            "several IN endpoints must not be reduced to an arbitrary guess"
        );
        assert_eq!(targets[1].in_endpoint, None);
    }

    #[test]
    fn identical_usb_devices_without_serials_are_marked_ambiguous() {
        let mut first = netum_usb_printer(vec![0x01], vec![0x81]);
        first.serial_number = None;
        let mut second = first.clone();
        second.address = 61;

        let targets = usb_add_targets(vec![first, second], &PrinterConfiguration::default());

        assert_eq!(targets.len(), 2);
        assert!(
            targets.iter().all(|target| target.ambiguous_without_serial),
            "both saved identities would match both connected physical devices"
        );
    }

    #[test]
    fn usb_add_choice_explains_the_descriptor_and_route_being_saved() {
        let target = usb_add_targets(
            vec![netum_usb_printer(vec![0x01], vec![0x81])],
            &PrinterConfiguration::default(),
        )
        .remove(0);

        assert_eq!(
            target.to_string(),
            "USB Portable Printer (YICHIP3121) (0416:5011; serial B120300001; bus 003 address 60; interface 0; OUT 0x01)"
        );
    }

    #[test]
    fn list_shows_the_usb_coordinates_needed_by_print() {
        let mut inventory = FixedInventory {
            printers: vec![UsbPrinter {
                vendor_id: 0x0416,
                product_id: 0x5011,
                bus: "3".to_owned(),
                address: 57,
                manufacturer: Some("YICHIP3121".to_owned()),
                product: Some("USB Portable Printer".to_owned()),
                serial_number: Some("B120300001".to_owned()),
                interface_number: 0,
                out_endpoints: vec![0x01],
                in_endpoints: vec![0x81],
            }],
        };
        // `list` only shows configured printers now, so the coordinates a
        // connected USB interface needs for `print` must come from a
        // CONFIGURED entry's merged block, not a bare unconfigured device.
        let configuration = PrinterConfiguration::parse(
            "\
[netum-usb]
transport = \"usb\"
vendor_id = \"0x0416\"
product_id = \"0x5011\"
serial_number = \"B120300001\"
interface_number = 0
out_endpoint = \"0x01\"
in_endpoint = \"0x81\"
",
        )
        .expect("the printer configuration should be valid");
        let mut output = Vec::new();

        execute(&mut inventory, &configuration, &[], None, &mut output)
            .expect("listing should succeed");

        assert_eq!(
            String::from_utf8(output).expect("the listing should be UTF-8"),
            "\
[1] netum-usb
    status: connected
    model: USB Portable Printer (YICHIP3121)
    profile: unassigned
    transport: usb
    usb: 0416:5011; bus 3 address 57; interface 0
    endpoints: out 0x01; in 0x81
    serial: B120300001
"
        );
    }

    #[test]
    fn a_connected_but_unconfigured_usb_printer_is_not_listed() {
        // Discovery duty moved entirely to `printers discover`: a connected
        // USB interface that matches no saved identity must not produce a
        // block in `list`, even though it would previously have appeared
        // under its descriptor-derived label. The configuration here has no
        // USB printers at all, so this also exercises requirement 1's
        // enumeration skip (`identities()` is never called); see
        // `a_connected_but_unconfigured_usb_identity_is_not_listed_alongside_a_configured_entry`
        // below for the case where USB *is* configured but this particular
        // device still does not match any of it.
        let mut inventory = FixedInventory {
            printers: vec![UsbPrinter {
                vendor_id: 0x0416,
                product_id: 0x5011,
                bus: "3".to_owned(),
                address: 57,
                manufacturer: Some("YICHIP3121".to_owned()),
                product: Some("USB Portable Printer".to_owned()),
                serial_number: Some("B120300001".to_owned()),
                interface_number: 0,
                out_endpoints: vec![0x01],
                in_endpoints: vec![0x81],
            }],
        };
        let mut output = Vec::new();

        execute(
            &mut inventory,
            &PrinterConfiguration::default(),
            &[],
            None,
            &mut output,
        )
        .expect("listing should succeed");

        assert_eq!(
            String::from_utf8(output).expect("the listing should be UTF-8"),
            "No printers configured.\n"
        );
    }

    #[test]
    fn empty_list_is_a_successful_snapshot() {
        let mut inventory = FixedInventory {
            printers: Vec::new(),
        };
        let mut output = Vec::new();

        execute(
            &mut inventory,
            &PrinterConfiguration::default(),
            &[],
            None,
            &mut output,
        )
        .expect("an empty listing should succeed");

        assert_eq!(
            String::from_utf8(output).expect("the listing should be UTF-8"),
            "No printers configured.\n"
        );
    }

    #[test]
    fn configured_printer_is_listed_when_it_is_unavailable() {
        let mut inventory = FixedInventory {
            printers: Vec::new(),
        };
        let configuration = PrinterConfiguration::parse(
            "\
[netum-usb]
transport = \"usb\"
profile = \"NT-5890K\"
vendor_id = \"0x0416\"
product_id = \"0x5011\"
serial_number = \"B120300001\"
interface_number = 0
out_endpoint = \"0x01\"
in_endpoint = \"0x81\"
",
        )
        .expect("the printer configuration should be valid");
        let mut output = Vec::new();

        execute(&mut inventory, &configuration, &[], None, &mut output)
            .expect("listing should succeed");

        assert_eq!(
            String::from_utf8(output).expect("the listing should be UTF-8"),
            "\
[1] netum-usb
    status: unavailable
    profile: NT-5890K
    transport: usb
    usb: 0416:5011; interface 0
    endpoints: out 0x01; in 0x81
    serial: B120300001
"
        );
    }

    #[test]
    fn configured_usb_printer_can_remain_unprofiled() {
        let mut inventory = FixedInventory {
            printers: Vec::new(),
        };
        let configuration = PrinterConfiguration::parse(
            "\
[uncalibrated-usb]
transport = \"usb\"
vendor_id = \"0x0416\"
product_id = \"0x5011\"
interface_number = 0
out_endpoint = \"0x01\"
",
        )
        .expect("an unprofiled USB printer should be valid");
        let mut output = Vec::new();

        execute(&mut inventory, &configuration, &[], None, &mut output)
            .expect("listing should succeed");

        assert!(
            String::from_utf8(output)
                .expect("the listing should be UTF-8")
                .contains("profile: unassigned")
        );
    }

    #[test]
    fn connected_configured_printer_is_merged_into_one_named_entry() {
        let mut inventory = FixedInventory {
            printers: vec![UsbPrinter {
                vendor_id: 0x0416,
                product_id: 0x5011,
                bus: "3".to_owned(),
                address: 57,
                manufacturer: None,
                product: Some("USB Portable Printer".to_owned()),
                serial_number: Some("B120300001".to_owned()),
                interface_number: 0,
                out_endpoints: vec![0x01],
                in_endpoints: vec![0x81],
            }],
        };
        let configuration = PrinterConfiguration::parse(
            "\
[netum-usb]
transport = \"usb\"
profile = \"NT-5890K\"
vendor_id = \"0x0416\"
product_id = \"0x5011\"
serial_number = \"B120300001\"
interface_number = 0
out_endpoint = \"0x01\"
in_endpoint = \"0x81\"
",
        )
        .expect("the printer configuration should be valid");
        let mut output = Vec::new();

        execute(&mut inventory, &configuration, &[], None, &mut output)
            .expect("listing should succeed");

        assert_eq!(
            String::from_utf8(output).expect("the listing should be UTF-8"),
            "\
[1] netum-usb
    status: connected
    model: USB Portable Printer
    profile: NT-5890K
    transport: usb
    usb: 0416:5011; bus 3 address 57; interface 0
    endpoints: out 0x01; in 0x81
    serial: B120300001
"
        );
    }

    #[test]
    fn connected_printers_sort_first_then_each_status_sorts_by_display_name() {
        let mut inventory = FixedInventory {
            printers: vec![
                UsbPrinter {
                    vendor_id: 0x1000,
                    product_id: 0x0001,
                    bus: "1".to_owned(),
                    address: 1,
                    manufacturer: None,
                    product: Some("Zed Model".to_owned()),
                    serial_number: Some("CONNECTED".to_owned()),
                    interface_number: 0,
                    out_endpoints: vec![0x01],
                    in_endpoints: Vec::new(),
                },
                UsbPrinter {
                    vendor_id: 0x2000,
                    product_id: 0x0002,
                    bus: "2".to_owned(),
                    address: 2,
                    manufacturer: None,
                    product: Some("Alpha Model".to_owned()),
                    serial_number: None,
                    interface_number: 0,
                    out_endpoints: vec![0x01],
                    in_endpoints: Vec::new(),
                },
            ],
        };
        let configuration = PrinterConfiguration::parse(
            "\
[Zulu]
transport = \"usb\"
profile = \"CONNECTED\"
vendor_id = \"0x1000\"
product_id = \"0x0001\"
serial_number = \"CONNECTED\"
interface_number = 0
out_endpoint = \"0x01\"

[Alpha]
transport = \"usb\"
profile = \"CONNECTED-ALPHA\"
vendor_id = \"0x2000\"
product_id = \"0x0002\"
interface_number = 0
out_endpoint = \"0x01\"

[charlie]
transport = \"usb\"
profile = \"OFFLINE-C\"
vendor_id = \"0x3000\"
product_id = \"0x0003\"
interface_number = 0
out_endpoint = \"0x01\"

[Bravo]
transport = \"usb\"
profile = \"OFFLINE-B\"
vendor_id = \"0x4000\"
product_id = \"0x0004\"
interface_number = 0
out_endpoint = \"0x01\"
",
        )
        .expect("the printer configuration should be valid");
        let mut output = Vec::new();

        execute(&mut inventory, &configuration, &[], None, &mut output)
            .expect("listing should succeed");

        let headings = String::from_utf8(output)
            .expect("the listing should be UTF-8")
            .lines()
            .filter(|line| line.starts_with('['))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(
            headings,
            vec!["[1] Alpha", "[2] Zulu", "[3] Bravo", "[4] charlie",]
        );
    }

    #[test]
    fn one_saved_identity_names_at_most_one_connected_interface() {
        let mut inventory = FixedInventory {
            printers: vec![
                UsbPrinter {
                    vendor_id: 0x1000,
                    product_id: 0x0001,
                    bus: "2".to_owned(),
                    address: 2,
                    manufacturer: None,
                    product: Some("Second Model".to_owned()),
                    serial_number: None,
                    interface_number: 0,
                    out_endpoints: vec![0x01],
                    in_endpoints: Vec::new(),
                },
                UsbPrinter {
                    vendor_id: 0x1000,
                    product_id: 0x0001,
                    bus: "1".to_owned(),
                    address: 1,
                    manufacturer: None,
                    product: Some("First Model".to_owned()),
                    serial_number: None,
                    interface_number: 0,
                    out_endpoints: vec![0x01],
                    in_endpoints: Vec::new(),
                },
            ],
        };
        let configuration = PrinterConfiguration::parse(
            "\
[shared-identity]
transport = \"usb\"
profile = \"GENERIC\"
vendor_id = \"0x1000\"
product_id = \"0x0001\"
interface_number = 0
out_endpoint = \"0x01\"
",
        )
        .expect("the printer configuration should be valid");
        let mut output = Vec::new();

        execute(&mut inventory, &configuration, &[], None, &mut output)
            .expect("listing should succeed");

        // Both connected devices share one USB identity, but only one
        // configured entry claims it (`merge_usb_identities`, first-match by
        // stable location). The other device is left unconfigured, and
        // `list` no longer shows connected-but-unconfigured devices at all,
        // so it must produce no second block ("Second Model" never appears).
        let output = String::from_utf8(output).expect("the listing should be UTF-8");
        assert_eq!(output.matches("] shared-identity\n").count(), 1);
        assert_eq!(output.matches("status: connected").count(), 1);
        assert!(!output.contains("status: unavailable"));
        assert!(!output.contains("Second Model"));
    }

    #[test]
    fn a_connected_but_unconfigured_usb_identity_is_not_listed_alongside_a_configured_entry() {
        // Unlike `a_connected_but_unconfigured_usb_printer_is_not_listed`,
        // the configuration here is not empty, so `identities()` genuinely
        // runs; this proves the non-matching identity is dropped by
        // `merge_usb_identities` itself rather than by requirement 1's
        // enumeration skip.
        let mut inventory = FixedInventory {
            printers: vec![
                netum_usb_printer(vec![0x01], vec![0x81]),
                UsbPrinter {
                    vendor_id: 0x9999,
                    product_id: 0x0001,
                    bus: "9".to_owned(),
                    address: 9,
                    manufacturer: None,
                    product: Some("Stranger Printer".to_owned()),
                    serial_number: None,
                    interface_number: 0,
                    out_endpoints: vec![0x01],
                    in_endpoints: Vec::new(),
                },
            ],
        };
        let configuration = PrinterConfiguration::parse(
            "\
[netum-usb]
transport = \"usb\"
vendor_id = \"0x0416\"
product_id = \"0x5011\"
serial_number = \"B120300001\"
interface_number = 0
out_endpoint = \"0x01\"
",
        )
        .expect("the printer configuration should be valid");
        let mut output = Vec::new();

        execute(&mut inventory, &configuration, &[], None, &mut output)
            .expect("listing should succeed");

        let output = String::from_utf8(output).expect("the listing should be UTF-8");
        assert!(output.contains("] netum-usb\n"));
        assert!(output.contains("status: connected"));
        assert!(
            !output.contains("Stranger Printer"),
            "an identity matching no saved USB printer must never appear in `list`:\n{output}"
        );
    }

    #[test]
    fn list_first_match_wins_between_two_configured_entries_sharing_one_identity() {
        // The "vice versa" half of first-match-wins: two configured entries
        // both matching the *same* ambiguous pair of connected devices (no
        // serial on either side) must still produce exactly one connected
        // block. The losing configured entry is claimed by the ambiguity
        // resolution too, so it is neither connected nor unavailable —
        // mirroring `classify_usb_printers`' own handling of this case for
        // `printers discover`.
        let mut first_device = netum_usb_printer(vec![0x01], vec![0x81]);
        first_device.serial_number = None;
        first_device.bus = "1".to_owned();
        let mut second_device = first_device.clone();
        second_device.bus = "2".to_owned();
        let mut inventory = FixedInventory {
            printers: vec![first_device, second_device],
        };
        let configuration = PrinterConfiguration::parse(
            "\
[first-usb]
transport = \"usb\"
vendor_id = \"0x0416\"
product_id = \"0x5011\"
interface_number = 0
out_endpoint = \"0x01\"

[second-usb]
transport = \"usb\"
vendor_id = \"0x0416\"
product_id = \"0x5011\"
interface_number = 0
out_endpoint = \"0x01\"
",
        )
        .expect("the printer configuration should be valid");
        let mut output = Vec::new();

        execute(&mut inventory, &configuration, &[], None, &mut output)
            .expect("listing should succeed");

        let output = String::from_utf8(output).expect("the listing should be UTF-8");
        assert_eq!(output.matches("] first-usb\n").count(), 1);
        assert_eq!(output.matches("status: connected").count(), 1);
        assert!(
            !output.contains("second-usb"),
            "the losing configured entry must not appear as connected or unavailable:\n{output}"
        );
    }

    #[test]
    fn list_configured_without_serial_matches_a_connected_serial_and_prefers_it() {
        let mut inventory = FixedInventory {
            printers: vec![netum_usb_printer(vec![0x01], vec![0x81])],
        };
        let configuration = PrinterConfiguration::parse(
            "\
[unserialized-usb]
transport = \"usb\"
vendor_id = \"0x0416\"
product_id = \"0x5011\"
interface_number = 0
out_endpoint = \"0x01\"
",
        )
        .expect("an unserialized configuration should be valid");
        let mut output = Vec::new();

        execute(&mut inventory, &configuration, &[], None, &mut output)
            .expect("listing should succeed");

        let output = String::from_utf8(output).expect("the listing should be UTF-8");
        assert!(output.contains("status: connected"));
        assert!(
            output.contains("serial: B120300001"),
            "the connected device's own serial should be shown even though the saved entry has none:\n{output}"
        );
    }

    #[test]
    fn list_configured_serial_must_equal_the_connected_serial() {
        let mut inventory = FixedInventory {
            printers: vec![netum_usb_printer(vec![0x01], vec![0x81])],
        };
        let configuration = PrinterConfiguration::parse(
            "\
[mismatched-serial-usb]
transport = \"usb\"
vendor_id = \"0x0416\"
product_id = \"0x5011\"
serial_number = \"SOME-OTHER-SERIAL\"
interface_number = 0
out_endpoint = \"0x01\"
",
        )
        .expect("the printer configuration should be valid");
        let mut output = Vec::new();

        execute(&mut inventory, &configuration, &[], None, &mut output)
            .expect("listing should succeed");

        let output = String::from_utf8(output).expect("the listing should be UTF-8");
        assert!(
            output.contains("status: unavailable"),
            "a differing saved serial must not match the connected device:\n{output}"
        );
        assert!(!output.contains("status: connected"));
    }

    #[test]
    fn list_omits_the_serial_line_when_neither_side_has_one() {
        let mut printer = netum_usb_printer(vec![0x01], vec![0x81]);
        printer.serial_number = None;
        let mut inventory = FixedInventory {
            printers: vec![printer],
        };
        let configuration = PrinterConfiguration::parse(
            "\
[unserialized-usb]
transport = \"usb\"
vendor_id = \"0x0416\"
product_id = \"0x5011\"
interface_number = 0
out_endpoint = \"0x01\"
",
        )
        .expect("an unserialized configuration should be valid");
        let mut output = Vec::new();

        execute(&mut inventory, &configuration, &[], None, &mut output)
            .expect("listing should succeed");

        let output = String::from_utf8(output).expect("the listing should be UTF-8");
        assert!(output.contains("status: connected"));
        assert!(!output.contains("serial:"));
    }

    #[test]
    fn list_omits_the_model_line_when_the_identity_has_no_product_string() {
        let mut printer = netum_usb_printer(vec![0x01], vec![0x81]);
        printer.product = None;
        printer.manufacturer = None;
        let mut inventory = FixedInventory {
            printers: vec![printer],
        };
        let configuration = PrinterConfiguration::parse(
            "\
[netum-usb]
transport = \"usb\"
vendor_id = \"0x0416\"
product_id = \"0x5011\"
serial_number = \"B120300001\"
interface_number = 0
out_endpoint = \"0x01\"
",
        )
        .expect("the printer configuration should be valid");
        let mut output = Vec::new();

        execute(&mut inventory, &configuration, &[], None, &mut output)
            .expect("listing should succeed");

        let output = String::from_utf8(output).expect("the listing should be UTF-8");
        assert!(output.contains("status: connected"));
        assert!(
            !output.contains("model:"),
            "no product string means no model line, matching `write_usb_listing`'s own `model: None` handling:\n{output}"
        );
    }

    #[test]
    fn list_sources_interface_and_endpoints_from_configuration_not_the_device() {
        // `UsbDeviceIdentity` carries no interface or endpoint fields at
        // all, so this is also a type-level guarantee; this test pins the
        // observable behavior against a configuration whose interface and
        // endpoints are deliberately unlike the usual fixtures.
        let mut inventory = FixedInventory {
            printers: vec![netum_usb_printer(vec![0x01], vec![0x81])],
        };
        let configuration = PrinterConfiguration::parse(
            "\
[netum-usb]
transport = \"usb\"
vendor_id = \"0x0416\"
product_id = \"0x5011\"
serial_number = \"B120300001\"
interface_number = 3
out_endpoint = \"0x05\"
in_endpoint = \"0x86\"
",
        )
        .expect("the printer configuration should be valid");
        let mut output = Vec::new();

        execute(&mut inventory, &configuration, &[], None, &mut output)
            .expect("listing should succeed");

        assert_eq!(
            String::from_utf8(output).expect("the listing should be UTF-8"),
            "\
[1] netum-usb
    status: connected
    model: USB Portable Printer (YICHIP3121)
    profile: unassigned
    transport: usb
    usb: 0416:5011; bus 003 address 60; interface 3
    endpoints: out 0x05; in 0x86
    serial: B120300001
"
        );
    }

    #[test]
    fn list_skips_usb_enumeration_entirely_when_no_usb_printers_are_configured() {
        // Structural proof of requirement 1: a double whose `list()` and
        // `identities()` both panic proves `execute` never touches USB at
        // all when `configuration.usb_printers()` is empty, even though the
        // registry is not otherwise empty (a network printer is configured).
        struct PanicsIfUsbIsQueried;
        impl UsbInventory for PanicsIfUsbIsQueried {
            fn list(&mut self) -> Result<Vec<UsbPrinter>, CliError> {
                panic!("list() must not run when no USB printers are configured");
            }

            fn identities(&mut self) -> Result<Vec<UsbDeviceIdentity>, CliError> {
                panic!("identities() must not run when no USB printers are configured");
            }
        }
        let configuration = PrinterConfiguration::parse(
            r#"
[kitchen]
transport = "network"
host = "10.42.0.71"
port = 9100
"#,
        )
        .expect("the network-only configuration should parse");
        let mut output = Vec::new();

        execute(
            &mut PanicsIfUsbIsQueried,
            &configuration,
            &[false],
            None,
            &mut output,
        )
        .expect("listing should succeed without touching USB at all");

        assert!(
            String::from_utf8(output)
                .expect("the listing should be UTF-8")
                .contains("] kitchen")
        );
    }

    #[test]
    fn list_never_opens_usb_devices_to_check_presence() {
        // Structural proof of requirement 2: a double whose `list()` panics
        // but whose `identities()` succeeds proves `execute` resolves USB
        // presence purely from metadata, the same way
        // `NusbInventory::identities` never calls `.open()`.
        struct MetadataOnlyInventory;
        impl UsbInventory for MetadataOnlyInventory {
            fn list(&mut self) -> Result<Vec<UsbPrinter>, CliError> {
                panic!("printers list must never call the open-based list()");
            }

            fn identities(&mut self) -> Result<Vec<UsbDeviceIdentity>, CliError> {
                Ok(vec![UsbDeviceIdentity {
                    vendor_id: 0x0416,
                    product_id: 0x5011,
                    bus: "3".to_owned(),
                    address: 57,
                    manufacturer: Some("YICHIP3121".to_owned()),
                    product: Some("USB Portable Printer".to_owned()),
                    serial_number: Some("B120300001".to_owned()),
                }])
            }
        }
        let configuration = PrinterConfiguration::parse(
            "\
[netum-usb]
transport = \"usb\"
vendor_id = \"0x0416\"
product_id = \"0x5011\"
serial_number = \"B120300001\"
interface_number = 0
out_endpoint = \"0x01\"
",
        )
        .expect("the printer configuration should be valid");
        let mut output = Vec::new();

        execute(
            &mut MetadataOnlyInventory,
            &configuration,
            &[],
            None,
            &mut output,
        )
        .expect("listing should succeed without opening any device");

        assert!(
            String::from_utf8(output)
                .expect("the listing should be UTF-8")
                .contains("status: connected")
        );
    }

    #[test]
    fn only_printer_class_bulk_endpoints_are_listed() {
        let descriptor_bytes = [
            9, 2, 55, 0, 2, 1, 0, 0x80, 50, // configuration
            9, 4, 0, 0, 3, 7, 1, 2, 0, // printer interface
            7, 5, 0x01, 2, 64, 0, 0, // bulk OUT
            7, 5, 0x81, 2, 64, 0, 0, // bulk IN
            7, 5, 0x82, 3, 8, 0, 10, // interrupt IN, not a print endpoint
            9, 4, 1, 0, 1, 0xff, 0, 0, 0, // vendor-specific interface
            7, 5, 0x02, 2, 64, 0, 0, // bulk OUT, but not printer class
        ];
        let configuration = nusb::descriptors::ConfigurationDescriptor::new(&descriptor_bytes)
            .expect("the descriptor should be valid");

        let interfaces = printer_interfaces(configuration);

        assert_eq!(
            interfaces,
            vec![UsbPrinterInterface {
                interface_number: 0,
                out_endpoints: vec![0x01],
                in_endpoints: vec![0x81],
            }]
        );
    }

    #[test]
    fn usb_selector_requires_both_vendor_and_product() {
        assert!(
            usb_selector(None, None, None)
                .expect("no selector is valid")
                .is_none()
        );
        assert!(matches!(
            usb_selector(Some(0x0416), None, None),
            Err(CliError::IncompleteUsbSelector)
        ));
        assert!(matches!(
            usb_selector(None, Some(0x5011), None),
            Err(CliError::IncompleteUsbSelector)
        ));
        assert!(matches!(
            usb_selector(None, None, Some("B120300001".to_owned())),
            Err(CliError::IncompleteUsbSelector)
        ));
    }

    #[test]
    fn a_serial_selector_narrows_identical_usb_models() {
        let mut first = netum_usb_printer(vec![0x01], vec![0x81]);
        first.serial_number = Some("FIRST".to_owned());
        let mut second = netum_usb_printer(vec![0x01], vec![0x81]);
        second.serial_number = Some("SECOND".to_owned());
        second.address = 61;
        let targets = usb_add_targets(vec![first, second], &PrinterConfiguration::default());

        let matched = filter_usb_targets(
            targets,
            &UsbSelector {
                vendor_id: 0x0416,
                product_id: 0x5011,
                serial: Some("SECOND".to_owned()),
            },
        );

        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].serial_number.as_deref(), Some("SECOND"));
    }

    #[test]
    fn a_non_interactive_selector_uses_a_unique_match_without_prompting() {
        let targets = usb_add_targets(
            vec![netum_usb_printer(vec![0x01], vec![0x81])],
            &PrinterConfiguration::default(),
        );

        let target = select_usb_target(
            targets,
            Some(&UsbSelector {
                vendor_id: 0x0416,
                product_id: 0x5011,
                serial: None,
            }),
            false,
            &mut UnexpectedAddPrompter,
        )
        .expect("a unique descriptor match should resolve without a menu");

        assert_eq!(target.vendor_id, 0x0416);
        assert_eq!(target.out_endpoint, 0x01);
    }

    #[test]
    fn a_non_interactive_selector_that_matches_nothing_is_an_error() {
        let targets = usb_add_targets(
            vec![netum_usb_printer(vec![0x01], vec![0x81])],
            &PrinterConfiguration::default(),
        );

        let error = select_usb_target(
            targets,
            Some(&UsbSelector {
                vendor_id: 0x1234,
                product_id: 0x5678,
                serial: None,
            }),
            false,
            &mut UnexpectedAddPrompter,
        )
        .expect_err("an unmatched selector must not save anything");

        assert!(matches!(error, CliError::NoMatchingUsbPrinter));
    }

    #[test]
    fn a_non_interactive_ambiguous_selector_refuses_to_guess() {
        let targets = usb_add_targets(
            vec![netum_usb_printer(vec![0x01, 0x02], vec![0x81])],
            &PrinterConfiguration::default(),
        );

        let error = select_usb_target(
            targets,
            Some(&UsbSelector {
                vendor_id: 0x0416,
                product_id: 0x5011,
                serial: None,
            }),
            false,
            &mut UnexpectedAddPrompter,
        )
        .expect_err("two bulk OUT endpoints must not be silently reduced to one");

        assert!(matches!(error, CliError::AmbiguousUsbPrinter));
    }

    #[test]
    fn an_interactive_ambiguous_selector_defers_the_endpoint_choice() {
        let targets = usb_add_targets(
            vec![netum_usb_printer(vec![0x01, 0x02], vec![0x81])],
            &PrinterConfiguration::default(),
        );

        let target = select_usb_target(
            targets,
            Some(&UsbSelector {
                vendor_id: 0x0416,
                product_id: 0x5011,
                serial: None,
            }),
            true,
            &mut FirstUsbPrompter,
        )
        .expect("a terminal can still pick among the narrowed routes");

        assert_eq!(target.out_endpoint, 0x01);
    }

    #[test]
    fn non_interactive_usb_add_saves_the_selected_descriptor_coordinates() {
        let directory = temporary_directory("non-interactive-add-usb");
        let configuration = directory.join("printers.toml");
        let arguments = AddPrinterArgs {
            name: Some("counter-usb".to_owned()),
            transport: Some(PrinterTransport::Usb),
            host: None,
            port: None,
            vendor_id: Some(0x0416),
            product_id: Some(0x5011),
            serial: Some("B120300001".to_owned()),
            profile: Some("NT-5890K".to_owned()),
            discover: false,
            subnet: Vec::new(),
            timeout: None,
        };
        let mut inventory = FixedInventory {
            printers: vec![netum_usb_printer(vec![0x01], vec![0x81])],
        };

        let name = execute_add(
            Some(&configuration),
            arguments,
            false,
            &mut UnexpectedAddPrompter,
            &mut inventory,
        )
        .expect("a matched USB printer should be saved without prompting");

        assert_eq!(name, "counter-usb");
        let document = fs::read_to_string(&configuration)
            .expect("the printer configuration should be readable");
        let table =
            toml::from_str::<toml::Table>(&document).expect("the configuration should be TOML");
        let printer = table["counter-usb"]
            .as_table()
            .expect("the configured printer should be a table");
        assert_eq!(printer["transport"].as_str(), Some("usb"));
        assert_eq!(printer["profile"].as_str(), Some("NT-5890K"));
        assert_eq!(printer["vendor_id"].as_str(), Some("0x0416"));
        assert_eq!(printer["product_id"].as_str(), Some("0x5011"));
        assert_eq!(printer["serial_number"].as_str(), Some("B120300001"));
        assert_eq!(printer["out_endpoint"].as_str(), Some("0x01"));
        assert_eq!(printer["in_endpoint"].as_str(), Some("0x81"));
        fs::remove_dir_all(directory).expect("the test directory should be removable");
    }

    #[test]
    fn non_interactive_usb_add_without_selectors_requires_a_terminal() {
        let arguments = AddPrinterArgs {
            name: Some("counter-usb".to_owned()),
            transport: Some(PrinterTransport::Usb),
            host: None,
            port: None,
            vendor_id: None,
            product_id: None,
            serial: None,
            profile: None,
            discover: false,
            subnet: Vec::new(),
            timeout: None,
        };

        let error = resolve_add(
            arguments,
            false,
            &mut UnexpectedAddPrompter,
            &mut FixedInventory {
                printers: vec![netum_usb_printer(vec![0x01], vec![0x81])],
            },
            &PrinterConfiguration::default(),
        )
        .expect_err("choosing a device without a selector needs a terminal");

        assert!(matches!(
            error,
            CliError::UsbRegistrationRequiresInteractive
        ));
    }

    #[test]
    fn usb_selectors_are_rejected_for_a_network_printer() {
        let arguments = AddPrinterArgs {
            name: Some("kitchen".to_owned()),
            transport: Some(PrinterTransport::Network),
            host: Some("10.42.0.71".to_owned()),
            port: None,
            vendor_id: Some(0x0416),
            product_id: Some(0x5011),
            serial: None,
            profile: None,
            discover: false,
            subnet: Vec::new(),
            timeout: None,
        };

        let error = resolve_add(
            arguments,
            false,
            &mut UnexpectedAddPrompter,
            &mut FixedInventory {
                printers: Vec::new(),
            },
            &PrinterConfiguration::default(),
        )
        .expect_err("a network printer must not accept USB descriptors");

        assert!(matches!(error, CliError::UsbSelectorForNetworkPrinter));
    }

    struct FirstUsbPrompter;

    impl AddPrompter for FirstUsbPrompter {
        fn name(&mut self) -> Result<String, CliError> {
            panic!("name prompt was not expected")
        }

        fn transport(&mut self) -> Result<PrinterTransport, CliError> {
            panic!("transport prompt was not expected")
        }

        fn usb_printer(
            &mut self,
            mut printers: Vec<UsbAddTarget>,
        ) -> Result<UsbAddTarget, CliError> {
            assert!(
                printers.len() > 1,
                "a unique match should not reach the menu"
            );
            Ok(printers.remove(0))
        }

        fn host(&mut self) -> Result<String, CliError> {
            panic!("host prompt was not expected")
        }

        fn port(&mut self) -> Result<u16, CliError> {
            panic!("port prompt was not expected")
        }

        fn profile(&mut self) -> Result<Option<String>, CliError> {
            panic!("profile prompt was not expected")
        }
    }

    struct FixedInventory {
        printers: Vec<UsbPrinter>,
    }

    struct FixedAddPrompter {
        names: VecDeque<String>,
        rejected_names: Vec<String>,
        port_prompts: usize,
    }

    struct UsbAddPrompter;

    struct UnexpectedAddPrompter;

    impl FixedAddPrompter {
        fn with_names<const N: usize>(names: [&str; N]) -> Self {
            Self {
                names: names.map(str::to_owned).into(),
                rejected_names: Vec::new(),
                port_prompts: 0,
            }
        }
    }

    impl AddPrompter for FixedAddPrompter {
        fn name(&mut self) -> Result<String, CliError> {
            Ok(self
                .names
                .pop_front()
                .expect("the resolver should not exhaust test names"))
        }

        fn reject_name(&mut self, error: &CliError) {
            self.rejected_names.push(error.to_string());
        }

        fn transport(&mut self) -> Result<PrinterTransport, CliError> {
            Ok(PrinterTransport::Network)
        }

        fn usb_printer(&mut self, _printers: Vec<UsbAddTarget>) -> Result<UsbAddTarget, CliError> {
            panic!("a network printer must not ask for a USB device")
        }

        fn host(&mut self) -> Result<String, CliError> {
            Ok("10.42.0.71".to_owned())
        }

        fn port(&mut self) -> Result<u16, CliError> {
            self.port_prompts += 1;
            Ok(9200)
        }

        fn profile(&mut self) -> Result<Option<String>, CliError> {
            Ok(Some("REFERENCE".to_owned()))
        }
    }

    impl AddPrompter for UsbAddPrompter {
        fn name(&mut self) -> Result<String, CliError> {
            Ok("counter-usb".to_owned())
        }

        fn transport(&mut self) -> Result<PrinterTransport, CliError> {
            Ok(PrinterTransport::Usb)
        }

        fn usb_printer(
            &mut self,
            mut printers: Vec<UsbAddTarget>,
        ) -> Result<UsbAddTarget, CliError> {
            assert_eq!(printers.len(), 1);
            Ok(printers.remove(0))
        }

        fn host(&mut self) -> Result<String, CliError> {
            panic!("a USB printer must not ask for a network host")
        }

        fn port(&mut self) -> Result<u16, CliError> {
            panic!("a USB printer must not ask for a network port")
        }

        fn profile(&mut self) -> Result<Option<String>, CliError> {
            Ok(Some("REFERENCE".to_owned()))
        }
    }

    impl AddPrompter for UnexpectedAddPrompter {
        fn name(&mut self) -> Result<String, CliError> {
            panic!("name prompt was not expected")
        }

        fn transport(&mut self) -> Result<PrinterTransport, CliError> {
            panic!("transport prompt was not expected")
        }

        fn usb_printer(&mut self, _printers: Vec<UsbAddTarget>) -> Result<UsbAddTarget, CliError> {
            panic!("USB printer prompt was not expected")
        }

        fn host(&mut self) -> Result<String, CliError> {
            panic!("host prompt was not expected")
        }

        fn port(&mut self) -> Result<u16, CliError> {
            panic!("port prompt was not expected")
        }

        fn profile(&mut self) -> Result<Option<String>, CliError> {
            panic!("profile prompt was not expected")
        }
    }

    impl UsbInventory for FixedInventory {
        fn list(&mut self) -> Result<Vec<UsbPrinter>, CliError> {
            Ok(self.printers.clone())
        }

        fn identities(&mut self) -> Result<Vec<UsbDeviceIdentity>, CliError> {
            Ok(self.printers.iter().map(usb_printer_identity).collect())
        }
    }

    /// A `UsbInventory` double that exercises `list_tolerant`'s partial-
    /// failure path directly: some devices enumerate fine, others report a
    /// canned warning, mirroring what `NusbInventory::list_tolerant` does
    /// when a real device cannot be opened or inspected. `list()` stays
    /// strict (as `printers list`/`add` need), returning only the printers
    /// that "succeeded".
    struct PartiallyFailingInventory {
        printers: Vec<UsbPrinter>,
        warnings: Vec<String>,
    }

    impl UsbInventory for PartiallyFailingInventory {
        fn list(&mut self) -> Result<Vec<UsbPrinter>, CliError> {
            Ok(self.printers.clone())
        }

        fn list_tolerant(&mut self) -> Result<UsbEnumeration, CliError> {
            Ok(UsbEnumeration {
                printers: self.printers.clone(),
                warnings: self.warnings.clone(),
            })
        }

        fn identities(&mut self) -> Result<Vec<UsbDeviceIdentity>, CliError> {
            Ok(self.printers.iter().map(usb_printer_identity).collect())
        }
    }

    /// Derive a metadata-only device identity from a `UsbPrinter` test
    /// fixture, the same fields `NusbInventory::identities` would read from
    /// `nusb::DeviceInfo` without opening the device. Test doubles use this
    /// so one fixture can drive both the open-based `list()`/`list_tolerant()`
    /// paths (discover, add) and the metadata-only `identities()` path
    /// (list) without keeping two copies of the same descriptor in sync.
    fn usb_printer_identity(printer: &UsbPrinter) -> UsbDeviceIdentity {
        UsbDeviceIdentity {
            vendor_id: printer.vendor_id,
            product_id: printer.product_id,
            bus: printer.bus.clone(),
            address: printer.address,
            manufacturer: printer.manufacturer.clone(),
            product: printer.product.clone(),
            serial_number: printer.serial_number.clone(),
        }
    }

    fn netum_usb_printer(out_endpoints: Vec<u8>, in_endpoints: Vec<u8>) -> UsbPrinter {
        UsbPrinter {
            vendor_id: 0x0416,
            product_id: 0x5011,
            bus: "003".to_owned(),
            address: 60,
            manufacturer: Some("YICHIP3121".to_owned()),
            product: Some("USB Portable Printer".to_owned()),
            serial_number: Some("B120300001".to_owned()),
            interface_number: 0,
            out_endpoints,
            in_endpoints,
        }
    }

    fn temporary_directory(case: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the clock should be after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "escpost-printers-{case}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("the test directory should be creatable");
        path
    }

    #[test]
    fn discovered_hosts_print_full_listing_blocks_by_configuration_state() {
        let configuration = PrinterConfiguration::parse(
            r#"
[kitchen]
transport = "network"
host = "10.42.0.71"
port = 9100
profile = "TM-T88V"

[office]
transport = "network"
host = "10.42.0.9"
port = 9100

[counter]
transport = "network"
host = "10.42.0.20"
port = 9100
profile = "EPSON-TM88"

[counter-spare]
transport = "network"
host = "10.42.0.20"
port = 9100
"#,
        )
        .expect("the existing printers should parse");
        let hosts = vec![
            DiscoveredHost {
                address: Ipv4Addr::new(10, 42, 0, 5),
                port: 9100,
                interface: None,
            },
            DiscoveredHost {
                address: Ipv4Addr::new(10, 42, 0, 71),
                port: 9100,
                interface: Some("enx0".to_owned()),
            },
            DiscoveredHost {
                address: Ipv4Addr::new(10, 42, 0, 9),
                port: 9100,
                interface: None,
            },
            DiscoveredHost {
                address: Ipv4Addr::new(10, 42, 0, 20),
                port: 9100,
                interface: None,
            },
        ];
        let mut output = Vec::new();

        write_discovered_network_printers(&mut output, &hosts, &configuration, 1)
            .expect("writing the listing should succeed");

        assert_eq!(
            String::from_utf8(output).expect("the listing should be UTF-8"),
            "\
[1] 10.42.0.5:9100
    status: new
    transport: network
    network: 10.42.0.5:9100
[2] kitchen
    status: configured
    profile: TM-T88V
    transport: network
    network: 10.42.0.71:9100
    interface: enx0
[3] office
    status: configured
    profile: unassigned
    transport: network
    network: 10.42.0.9:9100
[4] counter
    status: configured
    profile: EPSON-TM88
    transport: network
    network: 10.42.0.20:9100
    also configured as: counter-spare
"
        );
    }

    struct UnexpectedDiscoverPicker;

    impl DiscoverPicker for UnexpectedDiscoverPicker {
        fn discovered_host(
            &mut self,
            _choices: Vec<DiscoverChoice>,
        ) -> Result<DiscoverChoice, CliError> {
            panic!("no discovery selection prompt was expected");
        }
    }

    struct FirstChoiceDiscoverPicker;

    impl DiscoverPicker for FirstChoiceDiscoverPicker {
        fn discovered_host(
            &mut self,
            mut choices: Vec<DiscoverChoice>,
        ) -> Result<DiscoverChoice, CliError> {
            Ok(choices.remove(0))
        }
    }

    fn discovered(address: [u8; 4], port: u16) -> DiscoveredHost {
        DiscoveredHost {
            address: Ipv4Addr::from(address),
            port,
            interface: Some("enx0".to_owned()),
        }
    }

    #[test]
    fn registration_hint_for_a_new_host_at_the_default_port() {
        let hosts = vec![discovered([10, 42, 0, 71], 9100)];

        let hint = registration_hint(&hosts, &PrinterConfiguration::default(), 9100);

        assert_eq!(
            hint,
            Some(
                "Register a new printer with: escpost printers add <NAME> --transport network --discover"
                    .to_owned()
            )
        );
    }

    #[test]
    fn registration_hint_for_a_new_host_at_a_non_default_port() {
        let hosts = vec![discovered([10, 42, 0, 71], 9200)];

        let hint = registration_hint(&hosts, &PrinterConfiguration::default(), 9200);

        assert_eq!(
            hint,
            Some(
                "Register a new printer with: escpost printers add <NAME> --transport network --discover --port 9200"
                    .to_owned()
            )
        );
    }

    #[test]
    fn registration_hint_does_not_depend_on_how_many_new_hosts_were_found() {
        let one_new_host = vec![discovered([10, 42, 0, 71], 9100)];
        let several_new_hosts = vec![
            discovered([10, 42, 0, 5], 9100),
            discovered([10, 42, 0, 71], 9100),
        ];

        let hint_for_one = registration_hint(&one_new_host, &PrinterConfiguration::default(), 9100);
        let hint_for_several =
            registration_hint(&several_new_hosts, &PrinterConfiguration::default(), 9100);

        assert_eq!(hint_for_one, hint_for_several);
        assert_eq!(
            hint_for_one,
            Some(
                "Register a new printer with: escpost printers add <NAME> --transport network --discover"
                    .to_owned()
            )
        );
    }

    #[test]
    fn registration_hint_is_none_when_every_discovered_host_is_already_configured() {
        let configuration = PrinterConfiguration::parse(
            r#"
[kitchen]
transport = "network"
host = "10.42.0.71"
port = 9100
"#,
        )
        .expect("the existing printer should parse");
        let hosts = vec![discovered([10, 42, 0, 71], 9100)];

        let hint = registration_hint(&hosts, &configuration, 9100);

        assert_eq!(hint, None);
    }

    #[test]
    fn registration_hint_is_none_for_an_empty_sweep() {
        let hint = registration_hint(&[], &PrinterConfiguration::default(), 9100);

        assert_eq!(hint, None);
    }

    #[test]
    fn a_single_discovered_host_is_chosen_without_prompting() {
        let choices = vec![DiscoverChoice {
            host: discovered([10, 42, 0, 71], 9100),
            configured_as: Vec::new(),
        }];

        let chosen = choose_discovered_host(choices, 9100, false, &mut UnexpectedDiscoverPicker)
            .expect("one candidate needs no prompt");

        assert_eq!(chosen, discovered([10, 42, 0, 71], 9100));
    }

    #[test]
    fn zero_discovered_hosts_is_an_error() {
        let error = choose_discovered_host(Vec::new(), 9100, true, &mut UnexpectedDiscoverPicker)
            .expect_err("nothing to add must fail");

        assert!(matches!(error, CliError::NoDiscoveredPrinters(9100)));
    }

    #[test]
    fn several_discovered_hosts_without_a_terminal_is_an_error_naming_them() {
        let choices = vec![
            DiscoverChoice {
                host: discovered([10, 42, 0, 5], 9100),
                configured_as: Vec::new(),
            },
            DiscoverChoice {
                host: discovered([10, 42, 0, 71], 9100),
                configured_as: vec!["kitchen".to_owned()],
            },
        ];

        let error = choose_discovered_host(choices, 9100, false, &mut UnexpectedDiscoverPicker)
            .expect_err("an implicit choice among several hosts must be refused");

        let CliError::AmbiguousDiscoveredPrinters(names) = error else {
            panic!("expected AmbiguousDiscoveredPrinters, got {error:?}");
        };
        assert_eq!(
            names,
            vec![
                "10.42.0.5:9100 (via enx0)".to_owned(),
                "10.42.0.71:9100 (via enx0; configured as kitchen)".to_owned(),
            ]
        );
    }

    #[test]
    fn several_discovered_hosts_with_a_terminal_are_prompted() {
        let choices = vec![
            DiscoverChoice {
                host: discovered([10, 42, 0, 5], 9100),
                configured_as: Vec::new(),
            },
            DiscoverChoice {
                host: discovered([10, 42, 0, 71], 9100),
                configured_as: Vec::new(),
            },
        ];

        let chosen = choose_discovered_host(choices, 9100, true, &mut FirstChoiceDiscoverPicker)
            .expect("the prompted selection should resolve");

        assert_eq!(chosen, discovered([10, 42, 0, 5], 9100));
    }

    #[test]
    fn discover_reports_a_new_usb_printer_with_no_model_or_profile_line() {
        let mut inventory = FixedInventory {
            printers: vec![UsbPrinter {
                vendor_id: 0x0416,
                product_id: 0x5011,
                bus: "3".to_owned(),
                address: 57,
                manufacturer: Some("YICHIP3121".to_owned()),
                product: Some("USB Portable Printer".to_owned()),
                serial_number: Some("B120300001".to_owned()),
                interface_number: 0,
                out_endpoints: vec![0x01],
                in_endpoints: vec![0x81],
            }],
        };
        let mut output = Vec::new();

        execute_discover(
            &mut inventory,
            &PrinterConfiguration::default(),
            &[],
            None,
            &mut output,
            &mut Vec::new(),
        )
        .expect("discovery should succeed");

        assert_eq!(
            String::from_utf8(output).expect("the listing should be UTF-8"),
            "\
[1] USB Portable Printer (YICHIP3121)
    status: new
    transport: usb
    usb: 0416:5011; bus 3 address 57; interface 0
    endpoints: out 0x01; in 0x81
    serial: B120300001
"
        );
    }

    #[test]
    fn discover_reports_a_configured_usb_printer_with_model_and_profile_lines() {
        let mut inventory = FixedInventory {
            printers: vec![netum_usb_printer(vec![0x01], vec![0x81])],
        };
        let configuration = PrinterConfiguration::parse(
            "\
[netum-usb]
transport = \"usb\"
profile = \"NT-5890K\"
vendor_id = \"0x0416\"
product_id = \"0x5011\"
serial_number = \"B120300001\"
interface_number = 0
out_endpoint = \"0x01\"
in_endpoint = \"0x81\"
",
        )
        .expect("the printer configuration should be valid");
        let mut output = Vec::new();

        execute_discover(
            &mut inventory,
            &configuration,
            &[],
            None,
            &mut output,
            &mut Vec::new(),
        )
        .expect("discovery should succeed");

        assert_eq!(
            String::from_utf8(output).expect("the listing should be UTF-8"),
            "\
[1] netum-usb
    status: configured
    model: USB Portable Printer (YICHIP3121)
    profile: NT-5890K
    transport: usb
    usb: 0416:5011; bus 003 address 60; interface 0
    endpoints: out 0x01; in 0x81
    serial: B120300001
"
        );
    }

    #[test]
    fn discover_configured_usb_printer_can_remain_unprofiled() {
        let mut inventory = FixedInventory {
            printers: vec![netum_usb_printer(vec![0x01], vec![0x81])],
        };
        let configuration = PrinterConfiguration::parse(
            "\
[uncalibrated-usb]
transport = \"usb\"
vendor_id = \"0x0416\"
product_id = \"0x5011\"
interface_number = 0
out_endpoint = \"0x01\"
",
        )
        .expect("an unprofiled USB printer should be valid");
        let mut output = Vec::new();

        execute_discover(
            &mut inventory,
            &configuration,
            &[],
            None,
            &mut output,
            &mut Vec::new(),
        )
        .expect("discovery should succeed");

        assert!(
            String::from_utf8(output)
                .expect("the listing should be UTF-8")
                .contains("profile: unassigned")
        );
    }

    #[test]
    fn discover_one_saved_identity_names_at_most_one_connected_interface() {
        let mut inventory = FixedInventory {
            printers: vec![
                UsbPrinter {
                    vendor_id: 0x1000,
                    product_id: 0x0001,
                    bus: "2".to_owned(),
                    address: 2,
                    manufacturer: None,
                    product: Some("Second Model".to_owned()),
                    serial_number: None,
                    interface_number: 0,
                    out_endpoints: vec![0x01],
                    in_endpoints: Vec::new(),
                },
                UsbPrinter {
                    vendor_id: 0x1000,
                    product_id: 0x0001,
                    bus: "1".to_owned(),
                    address: 1,
                    manufacturer: None,
                    product: Some("First Model".to_owned()),
                    serial_number: None,
                    interface_number: 0,
                    out_endpoints: vec![0x01],
                    in_endpoints: Vec::new(),
                },
            ],
        };
        let configuration = PrinterConfiguration::parse(
            "\
[shared-identity]
transport = \"usb\"
profile = \"GENERIC\"
vendor_id = \"0x1000\"
product_id = \"0x0001\"
interface_number = 0
out_endpoint = \"0x01\"
",
        )
        .expect("the printer configuration should be valid");
        let mut output = Vec::new();

        execute_discover(
            &mut inventory,
            &configuration,
            &[],
            None,
            &mut output,
            &mut Vec::new(),
        )
        .expect("discovery should succeed");

        let output = String::from_utf8(output).expect("the listing should be UTF-8");
        assert_eq!(output.matches("] shared-identity\n").count(), 1);
        assert_eq!(output.matches("status: configured").count(), 1);
        assert_eq!(output.matches("status: new").count(), 1);
    }

    #[test]
    fn discover_numbers_usb_blocks_before_network_blocks_continuously() {
        let mut inventory = FixedInventory {
            printers: vec![
                UsbPrinter {
                    vendor_id: 0x0416,
                    product_id: 0x5011,
                    bus: "3".to_owned(),
                    address: 57,
                    manufacturer: Some("YICHIP3121".to_owned()),
                    product: Some("USB Portable Printer".to_owned()),
                    serial_number: Some("B120300001".to_owned()),
                    interface_number: 0,
                    out_endpoints: vec![0x01],
                    in_endpoints: vec![0x81],
                },
                UsbPrinter {
                    vendor_id: 0x0416,
                    product_id: 0x5011,
                    bus: "3".to_owned(),
                    address: 60,
                    manufacturer: Some("YICHIP3121".to_owned()),
                    product: Some("USB Portable Printer".to_owned()),
                    serial_number: Some("B120300002".to_owned()),
                    interface_number: 0,
                    out_endpoints: vec![0x01],
                    in_endpoints: vec![0x81],
                },
            ],
        };
        let configuration = PrinterConfiguration::parse(
            "\
[netum-usb]
transport = \"usb\"
profile = \"NT-5890K\"
vendor_id = \"0x0416\"
product_id = \"0x5011\"
serial_number = \"B120300002\"
interface_number = 0
out_endpoint = \"0x01\"
in_endpoint = \"0x81\"
",
        )
        .expect("the printer configuration should be valid");
        let hosts = vec![discovered([10, 42, 0, 5], 9100)];
        let mut output = Vec::new();

        execute_discover(
            &mut inventory,
            &configuration,
            &hosts,
            None,
            &mut output,
            &mut Vec::new(),
        )
        .expect("discovery should succeed");

        let headings = String::from_utf8(output)
            .expect("the listing should be UTF-8")
            .lines()
            .filter(|line| line.starts_with('['))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(
            headings,
            vec![
                "[1] USB Portable Printer (YICHIP3121)".to_owned(),
                "[2] netum-usb".to_owned(),
                "[3] 10.42.0.5:9100".to_owned(),
            ]
        );
    }

    #[test]
    fn discover_transport_usb_skips_the_network_section() {
        let mut inventory = FixedInventory {
            printers: vec![netum_usb_printer(vec![0x01], vec![0x81])],
        };
        let hosts = vec![discovered([10, 42, 0, 5], 9100)];
        let mut output = Vec::new();

        execute_discover(
            &mut inventory,
            &PrinterConfiguration::default(),
            &hosts,
            Some(InventoryTransport::Usb),
            &mut output,
            &mut Vec::new(),
        )
        .expect("discovery should succeed");

        let output = String::from_utf8(output).expect("the listing should be UTF-8");
        assert!(output.contains("transport: usb"));
        assert!(
            !output.contains("transport: network"),
            "--transport usb must not scan or report network hosts:\n{output}"
        );
    }

    #[test]
    fn discover_transport_network_skips_the_usb_section() {
        let mut inventory = FixedInventory {
            printers: vec![netum_usb_printer(vec![0x01], vec![0x81])],
        };
        let hosts = vec![discovered([10, 42, 0, 5], 9100)];
        let mut output = Vec::new();

        execute_discover(
            &mut inventory,
            &PrinterConfiguration::default(),
            &hosts,
            Some(InventoryTransport::Network),
            &mut output,
            &mut Vec::new(),
        )
        .expect("discovery should succeed");

        let output = String::from_utf8(output).expect("the listing should be UTF-8");
        assert!(
            !output.contains("transport: usb"),
            "--transport network must not enumerate or report USB printers:\n{output}"
        );
        assert!(output.contains("transport: network"));
    }

    #[test]
    fn discover_reports_an_empty_combined_result() {
        let mut inventory = FixedInventory {
            printers: Vec::new(),
        };
        let mut output = Vec::new();

        execute_discover(
            &mut inventory,
            &PrinterConfiguration::default(),
            &[],
            None,
            &mut output,
            &mut Vec::new(),
        )
        .expect("discovery should succeed");

        assert_eq!(
            String::from_utf8(output).expect("the listing should be UTF-8"),
            "No printers discovered.\n"
        );
    }

    #[test]
    fn usb_registration_hint_for_a_new_usb_printer() {
        let connected = vec![ConnectedUsbPrinter {
            printer: netum_usb_printer(vec![0x01], vec![0x81]),
            configuration_index: None,
        }];

        let hint = usb_registration_hint(&connected);

        assert_eq!(
            hint,
            Some("Register a new printer with: escpost printers add <NAME> --transport usb")
        );
    }

    #[test]
    fn usb_registration_hint_is_none_when_every_connected_usb_printer_is_configured() {
        let connected = vec![ConnectedUsbPrinter {
            printer: netum_usb_printer(vec![0x01], vec![0x81]),
            configuration_index: Some(0),
        }];

        let hint = usb_registration_hint(&connected);

        assert_eq!(hint, None);
    }

    #[test]
    fn usb_registration_hint_is_none_for_no_connected_usb_printers() {
        assert_eq!(usb_registration_hint(&[]), None);
    }

    #[test]
    fn both_transports_can_hint_at_once() {
        let connected = vec![ConnectedUsbPrinter {
            printer: netum_usb_printer(vec![0x01], vec![0x81]),
            configuration_index: None,
        }];
        let hosts = vec![discovered([10, 42, 0, 71], 9100)];

        let usb_hint = usb_registration_hint(&connected);
        let network_hint = registration_hint(&hosts, &PrinterConfiguration::default(), 9100);

        assert!(usb_hint.is_some(), "a new USB printer should hint");
        assert!(network_hint.is_some(), "a new network host should hint");
    }

    #[test]
    fn discover_surfaces_a_per_device_warning_and_still_lists_the_rest() {
        let mut inventory = PartiallyFailingInventory {
            printers: vec![netum_usb_printer(vec![0x01], vec![0x81])],
            warnings: vec![
                "could not open USB device 0416:5012: permission denied (errno 13)".to_owned(),
            ],
        };
        let mut output = Vec::new();
        let mut warnings_output = Vec::new();

        execute_discover(
            &mut inventory,
            &PrinterConfiguration::default(),
            &[],
            None,
            &mut output,
            &mut warnings_output,
        )
        .expect("a per-device enumeration failure must not abort discovery");

        let output = String::from_utf8(output).expect("the listing should be UTF-8");
        assert!(
            output.contains("[1] USB Portable Printer (YICHIP3121)"),
            "the device that enumerated fine should still be listed:\n{output}"
        );
        let warnings_output =
            String::from_utf8(warnings_output).expect("the warnings should be UTF-8");
        assert_eq!(
            warnings_output,
            "Warning: could not open USB device 0416:5012: permission denied (errno 13)\n"
        );
    }
}
