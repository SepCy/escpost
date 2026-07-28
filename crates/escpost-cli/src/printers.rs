use std::io::{self, Write};

use crate::cli::{PrintersArgs, PrintersCommand};
use crate::configuration::{self, ConfiguredUsbPrinter, PrinterConfiguration};
use crate::error::CliError;
use nusb::MaybeFuture;
use nusb::descriptors::{ConfigurationDescriptor, TransferType};
use nusb::transfer::Direction;

const USB_CLASS_PRINTER: u8 = 0x07;

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

#[derive(Debug, PartialEq, Eq)]
struct UsbPrinterInterface {
    interface_number: u8,
    out_endpoints: Vec<u8>,
    in_endpoints: Vec<u8>,
}

struct MergedUsbInventory {
    connected: Vec<ConnectedUsbPrinter>,
    unavailable_configuration_indexes: Vec<usize>,
}

struct ConnectedUsbPrinter {
    printer: UsbPrinter,
    configuration_index: Option<usize>,
}

trait UsbInventory {
    fn list(&mut self) -> Result<Vec<UsbPrinter>, CliError>;
}

pub(crate) fn run(arguments: PrintersArgs) -> Result<(), CliError> {
    match arguments.command {
        PrintersCommand::List => {
            let configuration = configuration::load(arguments.config.as_deref())?;
            let mut inventory = NusbInventory;
            execute(&mut inventory, &configuration, &mut io::stdout().lock())
        }
    }
}

fn execute(
    inventory: &mut impl UsbInventory,
    configuration: &PrinterConfiguration,
    output: &mut impl Write,
) -> Result<(), CliError> {
    let listing = merge_usb_inventory(inventory.list()?, configuration);
    if listing.connected.is_empty() && listing.unavailable_configuration_indexes.is_empty() {
        writeln!(output, "No usable printers found.").map_err(CliError::WriteHumanOutput)?;
        return Ok(());
    }

    for (index, connected) in listing.connected.iter().enumerate() {
        let configured = connected
            .configuration_index
            .map(|index| &configuration.usb_printers()[index]);
        write_printer(output, index + 1, &connected.printer, configured)?;
    }
    for (offset, configuration_index) in listing
        .unavailable_configuration_indexes
        .into_iter()
        .enumerate()
    {
        write_unavailable_printer(
            output,
            listing.connected.len() + offset + 1,
            &configuration.usb_printers()[configuration_index],
        )?;
    }
    Ok(())
}

fn merge_usb_inventory(
    mut printers: Vec<UsbPrinter>,
    configuration: &PrinterConfiguration,
) -> MergedUsbInventory {
    // Assign ambiguous saved identities by stable USB location before sorting
    // for display. This keeps one saved alias from naming several identical
    // connected interfaces when the configuration has no serial number.
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
    connected.sort_by_cached_key(|connected| {
        let configured = connected
            .configuration_index
            .map(|index| &configuration.usb_printers()[index]);
        let display_name = connected_display_name(&connected.printer, configured);
        (
            display_name.to_lowercase(),
            display_name,
            connected.printer.bus.clone(),
            connected.printer.address,
            connected.printer.interface_number,
            connected.printer.vendor_id,
            connected.printer.product_id,
        )
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

    MergedUsbInventory {
        connected,
        unavailable_configuration_indexes,
    }
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
            let device = device_info
                .open()
                .wait()
                .map_err(|source| CliError::OpenUsbDevice {
                    vendor_id: device_info.vendor_id(),
                    product_id: device_info.product_id(),
                    source,
                })?;
            let configuration = device.active_configuration().map_err(|source| {
                CliError::InspectUsbConfiguration {
                    vendor_id: device_info.vendor_id(),
                    product_id: device_info.product_id(),
                    source,
                }
            })?;

            for interface in printer_interfaces(configuration) {
                printers.push(UsbPrinter {
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
                });
            }
        }

        Ok(printers)
    }
}

fn is_printer_device(device: &nusb::DeviceInfo) -> bool {
    device.class() == USB_CLASS_PRINTER
        || device
            .interfaces()
            .any(|interface| interface.class() == USB_CLASS_PRINTER)
}

fn write_printer(
    output: &mut impl Write,
    number: usize,
    printer: &UsbPrinter,
    configured: Option<&ConfiguredUsbPrinter>,
) -> Result<(), CliError> {
    let model = usb_printer_label(printer);

    writeln!(
        output,
        "[{number}] {}",
        configured.map_or(model.as_str(), |printer| &printer.name)
    )
    .map_err(CliError::WriteHumanOutput)?;
    writeln!(output, "    status: connected").map_err(CliError::WriteHumanOutput)?;
    if let Some(configured) = configured {
        writeln!(output, "    model: {model}").map_err(CliError::WriteHumanOutput)?;
        writeln!(output, "    profile: {}", configured.profile)
            .map_err(CliError::WriteHumanOutput)?;
    }
    writeln!(output, "    transport: usb").map_err(CliError::WriteHumanOutput)?;
    writeln!(
        output,
        "    usb: {:04x}:{:04x}; bus {} address {}; interface {}",
        printer.vendor_id,
        printer.product_id,
        printer.bus,
        printer.address,
        printer.interface_number
    )
    .map_err(CliError::WriteHumanOutput)?;
    write!(
        output,
        "    endpoints: out {}",
        format_endpoints(&printer.out_endpoints)
    )
    .map_err(CliError::WriteHumanOutput)?;
    if !printer.in_endpoints.is_empty() {
        write!(output, "; in {}", format_endpoints(&printer.in_endpoints))
            .map_err(CliError::WriteHumanOutput)?;
    }
    writeln!(output).map_err(CliError::WriteHumanOutput)?;
    if let Some(serial_number) = &printer.serial_number {
        writeln!(output, "    serial: {serial_number}").map_err(CliError::WriteHumanOutput)?;
    }
    Ok(())
}

fn write_unavailable_printer(
    output: &mut impl Write,
    number: usize,
    printer: &ConfiguredUsbPrinter,
) -> Result<(), CliError> {
    writeln!(output, "[{number}] {}", printer.name).map_err(CliError::WriteHumanOutput)?;
    writeln!(output, "    status: unavailable").map_err(CliError::WriteHumanOutput)?;
    writeln!(output, "    profile: {}", printer.profile).map_err(CliError::WriteHumanOutput)?;
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

fn connected_display_name(
    printer: &UsbPrinter,
    configured: Option<&ConfiguredUsbPrinter>,
) -> String {
    configured.map_or_else(
        || usb_printer_label(printer),
        |printer| printer.name.clone(),
    )
}

fn compare_display_names(left: &str, right: &str) -> std::cmp::Ordering {
    left.to_lowercase()
        .cmp(&right.to_lowercase())
        .then_with(|| left.cmp(right))
}

fn usb_printer_label(printer: &UsbPrinter) -> String {
    let product = printer.product.as_deref().unwrap_or("USB printer");
    printer.manufacturer.as_deref().map_or_else(
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
    use super::{UsbInventory, UsbPrinter, UsbPrinterInterface, execute, printer_interfaces};
    use crate::configuration::PrinterConfiguration;
    use crate::error::CliError;

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
        let mut output = Vec::new();

        execute(
            &mut inventory,
            &PrinterConfiguration::default(),
            &mut output,
        )
        .expect("listing should succeed");

        assert_eq!(
            String::from_utf8(output).expect("the listing should be UTF-8"),
            "\
[1] USB Portable Printer (YICHIP3121)
    status: connected
    transport: usb
    usb: 0416:5011; bus 3 address 57; interface 0
    endpoints: out 0x01; in 0x81
    serial: B120300001
"
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
            &mut output,
        )
        .expect("an empty listing should succeed");

        assert_eq!(
            String::from_utf8(output).expect("the listing should be UTF-8"),
            "No usable printers found.\n"
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

        execute(&mut inventory, &configuration, &mut output).expect("listing should succeed");

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

        execute(&mut inventory, &configuration, &mut output).expect("listing should succeed");

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

        execute(&mut inventory, &configuration, &mut output).expect("listing should succeed");

        let headings = String::from_utf8(output)
            .expect("the listing should be UTF-8")
            .lines()
            .filter(|line| line.starts_with('['))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(
            headings,
            vec!["[1] Alpha Model", "[2] Zulu", "[3] Bravo", "[4] charlie",]
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

        execute(&mut inventory, &configuration, &mut output).expect("listing should succeed");

        let output = String::from_utf8(output).expect("the listing should be UTF-8");
        assert_eq!(output.matches("] shared-identity\n").count(), 1);
        assert_eq!(output.matches("status: connected").count(), 2);
        assert!(!output.contains("status: unavailable"));
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

    struct FixedInventory {
        printers: Vec<UsbPrinter>,
    }

    impl UsbInventory for FixedInventory {
        fn list(&mut self) -> Result<Vec<UsbPrinter>, CliError> {
            Ok(self.printers.clone())
        }
    }
}
