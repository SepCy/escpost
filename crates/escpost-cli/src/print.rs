use std::fmt;
use std::io::Write;
use std::time::Duration;

use crate::cli::PrintArgs;
use crate::error::CliError;
use crate::source;
use nusb::MaybeFuture;
use nusb::transfer::{Bulk, Out};

const USB_WRITE_BUFFER_BYTES: usize = 16 * 1024;
const USB_TRANSFER_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct UsbTarget {
    vendor_id: u16,
    product_id: u16,
    interface: u8,
    out_endpoint: u8,
}

impl fmt::Display for UsbTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04x}:{:04x}, interface {}, OUT {:#04x}",
            self.vendor_id, self.product_id, self.interface, self.out_endpoint
        )
    }
}

struct PrintReport {
    target: UsbTarget,
    bytes_sent: usize,
}

trait UsbTransport {
    fn send(&mut self, target: UsbTarget, data: &[u8]) -> Result<(), CliError>;
}

pub(crate) fn run(arguments: PrintArgs) -> Result<(), CliError> {
    let mut transport = NusbTransport;
    let report = execute(arguments, &mut transport)?;
    eprintln!("USB target: {}", report.target);
    eprintln!("Bytes sent: {}", report.bytes_sent);
    Ok(())
}

fn execute(
    arguments: PrintArgs,
    transport: &mut impl UsbTransport,
) -> Result<PrintReport, CliError> {
    let target = UsbTarget {
        vendor_id: arguments.usb_vendor_id,
        product_id: arguments.usb_product_id,
        interface: arguments.usb_interface,
        out_endpoint: arguments.usb_out_endpoint,
    };
    if !(0x01..=0x0f).contains(&target.out_endpoint) {
        return Err(CliError::InvalidUsbOutEndpoint(target.out_endpoint));
    }

    let input = source::load(&arguments.source, arguments.format)?;
    transport.send(target, &input.bytes)?;
    Ok(PrintReport {
        target,
        bytes_sent: input.bytes.len(),
    })
}

struct NusbTransport;

impl UsbTransport for NusbTransport {
    fn send(&mut self, target: UsbTarget, data: &[u8]) -> Result<(), CliError> {
        let matches: Vec<_> = nusb::list_devices()
            .wait()
            .map_err(CliError::EnumerateUsb)?
            .filter(|device| {
                device.vendor_id() == target.vendor_id && device.product_id() == target.product_id
            })
            .collect();
        let device_info = require_unique_device(matches, target)?;
        let device = device_info
            .open()
            .wait()
            .map_err(|source| CliError::OpenUsbDevice {
                vendor_id: target.vendor_id,
                product_id: target.product_id,
                source,
            })?;
        // On Linux this temporarily detaches a kernel driver such as usblp.
        // nusb reattaches that driver when the claimed interface is dropped.
        let interface = device
            .detach_and_claim_interface(target.interface)
            .wait()
            .map_err(|source| CliError::ClaimUsbInterface {
                interface: target.interface,
                source,
            })?;
        let endpoint = interface
            .endpoint::<Bulk, Out>(target.out_endpoint)
            .map_err(|source| CliError::OpenUsbOutEndpoint {
                interface: target.interface,
                endpoint: target.out_endpoint,
                source,
            })?;
        let mut writer = endpoint
            .writer(USB_WRITE_BUFFER_BYTES)
            .with_write_timeout(USB_TRANSFER_TIMEOUT);

        // ESC/POS is already the wire format. Do not prepend initialization,
        // append paper motion, or otherwise alter the caller's bytes.
        writer
            .write_all(data)
            .map_err(|source| CliError::WriteUsb {
                endpoint: target.out_endpoint,
                source,
            })?;
        writer.flush().map_err(|source| CliError::FlushUsb {
            endpoint: target.out_endpoint,
            source,
        })
    }
}

fn require_unique_device<T>(mut matches: Vec<T>, target: UsbTarget) -> Result<T, CliError> {
    match matches.len() {
        0 => Err(CliError::UsbDeviceNotFound {
            vendor_id: target.vendor_id,
            product_id: target.product_id,
        }),
        1 => Ok(matches.remove(0)),
        count => Err(CliError::AmbiguousUsbDevices {
            vendor_id: target.vendor_id,
            product_id: target.product_id,
            count,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{UsbTarget, UsbTransport, execute, require_unique_device};
    use crate::cli::{InputFormat, PrintArgs};
    use crate::error::CliError;

    #[test]
    fn hexadecimal_source_bytes_reach_the_usb_boundary_unchanged() {
        let directory = temporary_directory("exact-hex");
        let source = directory.join("receipt.hex");
        fs::write(&source, "1b 40 00 ff 0a\n").expect("the source should be writable");
        let arguments = PrintArgs {
            source,
            format: InputFormat::Auto,
            usb_vendor_id: 0x0416,
            usb_product_id: 0x5011,
            usb_interface: 0,
            usb_out_endpoint: 0x01,
        };
        let mut transport = RecordingTransport::default();

        let report = execute(arguments, &mut transport).expect("printing should succeed");

        assert_eq!(
            transport.request,
            Some((
                UsbTarget {
                    vendor_id: 0x0416,
                    product_id: 0x5011,
                    interface: 0,
                    out_endpoint: 0x01,
                },
                vec![0x1b, 0x40, 0x00, 0xff, 0x0a],
            ))
        );
        assert_eq!(
            report.target,
            UsbTarget {
                vendor_id: 0x0416,
                product_id: 0x5011,
                interface: 0,
                out_endpoint: 0x01,
            }
        );
        assert_eq!(report.bytes_sent, 5);
        fs::remove_dir_all(directory).expect("the test directory should be removable");
    }

    #[test]
    fn several_matching_devices_are_rejected_instead_of_selecting_the_first() {
        let target = UsbTarget {
            vendor_id: 0x0416,
            product_id: 0x5011,
            interface: 0,
            out_endpoint: 0x01,
        };

        let error = require_unique_device(vec!["first", "second"], target)
            .expect_err("ambiguous devices must fail");

        assert!(matches!(
            error,
            CliError::AmbiguousUsbDevices {
                vendor_id: 0x0416,
                product_id: 0x5011,
                count: 2,
            }
        ));
    }

    #[test]
    fn no_matching_device_is_reported_without_opening_usb() {
        let target = UsbTarget {
            vendor_id: 0x0416,
            product_id: 0x5011,
            interface: 0,
            out_endpoint: 0x01,
        };

        let error = require_unique_device::<()>(Vec::new(), target)
            .expect_err("a missing device must fail");

        assert!(matches!(
            error,
            CliError::UsbDeviceNotFound {
                vendor_id: 0x0416,
                product_id: 0x5011,
            }
        ));
    }

    #[test]
    fn usb_target_uses_the_conventional_identifier_and_endpoint_notation() {
        let target = UsbTarget {
            vendor_id: 0x0416,
            product_id: 0x5011,
            interface: 0,
            out_endpoint: 0x01,
        };

        assert_eq!(target.to_string(), "0416:5011, interface 0, OUT 0x01");
    }

    #[derive(Default)]
    struct RecordingTransport {
        request: Option<(UsbTarget, Vec<u8>)>,
    }

    impl UsbTransport for RecordingTransport {
        fn send(&mut self, target: UsbTarget, data: &[u8]) -> Result<(), CliError> {
            self.request = Some((target, data.to_vec()));
            Ok(())
        }
    }

    fn temporary_directory(case: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the clock should be after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "escpost-print-{case}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("the test directory should be creatable");
        path
    }
}
