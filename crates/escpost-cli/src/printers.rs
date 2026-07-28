use std::io::{self, Write};

use crate::cli::{PrintersArgs, PrintersCommand};
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

trait UsbInventory {
    fn list(&mut self) -> Result<Vec<UsbPrinter>, CliError>;
}

pub(crate) fn run(arguments: PrintersArgs) -> Result<(), CliError> {
    match arguments.command {
        PrintersCommand::List => {
            let mut inventory = NusbInventory;
            execute(&mut inventory, &mut io::stdout().lock())
        }
    }
}

fn execute(inventory: &mut impl UsbInventory, output: &mut impl Write) -> Result<(), CliError> {
    let printers = inventory.list()?;
    if printers.is_empty() {
        writeln!(output, "No usable printers found.").map_err(CliError::WriteHumanOutput)?;
        return Ok(());
    }

    for (index, printer) in printers.iter().enumerate() {
        write_printer(output, index + 1, printer)?;
    }
    Ok(())
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
) -> Result<(), CliError> {
    let product = printer.product.as_deref().unwrap_or("USB printer");
    let manufacturer = printer
        .manufacturer
        .as_deref()
        .map_or(String::new(), |value| format!(" ({value})"));

    writeln!(output, "[{number}] {product}{manufacturer}").map_err(CliError::WriteHumanOutput)?;
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

        execute(&mut inventory, &mut output).expect("listing should succeed");

        assert_eq!(
            String::from_utf8(output).expect("the listing should be UTF-8"),
            "\
[1] USB Portable Printer (YICHIP3121)
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

        execute(&mut inventory, &mut output).expect("an empty listing should succeed");

        assert_eq!(
            String::from_utf8(output).expect("the listing should be UTF-8"),
            "No usable printers found.\n"
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

    struct FixedInventory {
        printers: Vec<UsbPrinter>,
    }

    impl UsbInventory for FixedInventory {
        fn list(&mut self) -> Result<Vec<UsbPrinter>, CliError> {
            Ok(self.printers.clone())
        }
    }
}
