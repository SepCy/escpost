use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::CliError;

const CONFIG_DIRECTORY_ENV: &str = "ESCPOST_CONFIG_DIR";
const PRINTERS_FILE: &str = "printers.toml";

#[derive(Debug, Default)]
pub(crate) struct PrinterConfiguration {
    usb_printers: Vec<ConfiguredUsbPrinter>,
}

#[derive(Debug)]
pub(crate) struct ConfiguredUsbPrinter {
    pub(crate) name: String,
    pub(crate) profile: String,
    pub(crate) vendor_id: u16,
    pub(crate) product_id: u16,
    pub(crate) serial_number: Option<String>,
    pub(crate) interface_number: u8,
    pub(crate) out_endpoint: u8,
}

impl PrinterConfiguration {
    pub(crate) fn parse(content: &str) -> Result<Self, String> {
        let document = toml::from_str::<toml::Table>(content).map_err(|error| error.to_string())?;
        let mut usb_printers = Vec::new();

        for (name, value) in document {
            let table = value
                .as_table()
                .ok_or_else(|| format!("printer {name:?} must be a table"))?;
            let transport = required_string(table, "transport", &name)?;
            if transport != "usb" {
                continue;
            }

            usb_printers.push(ConfiguredUsbPrinter {
                profile: required_string(table, "profile", &name)?.to_owned(),
                vendor_id: required_integer(table, "vendor_id", &name)?,
                product_id: required_integer(table, "product_id", &name)?,
                serial_number: optional_string(table, "serial_number", &name)?,
                interface_number: required_integer(table, "interface_number", &name)?,
                out_endpoint: required_integer(table, "out_endpoint", &name)?,
                name,
            });
        }

        Ok(Self { usb_printers })
    }

    pub(crate) fn usb_printers(&self) -> &[ConfiguredUsbPrinter] {
        &self.usb_printers
    }
}

/// Load the selected printer configuration when it exists.
///
/// Missing implicit configuration is normal, but a file named explicitly by
/// the developer must be readable and valid. Keeping that distinction here
/// prevents read-only commands from creating configuration as a side effect.
pub(crate) fn load(explicit_path: Option<&Path>) -> Result<PrinterConfiguration, CliError> {
    let (path, required) = match explicit_path {
        Some(path) => (path.to_owned(), true),
        None => match config_directory_override() {
            Some(directory) => (directory.join(PRINTERS_FILE), false),
            None => (platform_config_directory()?.join(PRINTERS_FILE), false),
        },
    };

    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(source) if !required && source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PrinterConfiguration::default());
        }
        Err(source) => {
            return Err(CliError::ReadPrinterConfiguration { path, source });
        }
    };
    PrinterConfiguration::parse(&content)
        .map_err(|message| CliError::InvalidPrinterConfiguration { path, message })
}

fn config_directory_override() -> Option<PathBuf> {
    env::var_os(CONFIG_DIRECTORY_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn platform_config_directory() -> Result<PathBuf, CliError> {
    directories::ProjectDirs::from("io", "receiptful", "escpost")
        .map(|directories| directories.config_dir().to_owned())
        .ok_or(CliError::NoUserConfigDirectory)
}

fn required_string<'a>(
    table: &'a toml::Table,
    field: &str,
    printer: &str,
) -> Result<&'a str, String> {
    table
        .get(field)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("printer {printer:?} field {field:?} must be a non-empty string"))
}

fn optional_string(
    table: &toml::Table,
    field: &str,
    printer: &str,
) -> Result<Option<String>, String> {
    match table.get(field) {
        None => Ok(None),
        Some(value) => value
            .as_str()
            .filter(|value| !value.is_empty())
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| {
                format!("printer {printer:?} field {field:?} must be a non-empty string")
            }),
    }
}

fn required_integer<T>(table: &toml::Table, field: &str, printer: &str) -> Result<T, String>
where
    T: TryFrom<u64>,
{
    let value = table
        .get(field)
        .ok_or_else(|| format!("printer {printer:?} is missing field {field:?}"))?;
    let integer = match value {
        toml::Value::Integer(value) => u64::try_from(*value).ok(),
        toml::Value::String(value) => parse_integer_string(value),
        _ => None,
    }
    .ok_or_else(|| format!("printer {printer:?} field {field:?} must be a non-negative integer"))?;

    T::try_from(integer).map_err(|_| format!("printer {printer:?} field {field:?} is out of range"))
}

fn parse_integer_string(value: &str) -> Option<u64> {
    value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .map_or_else(
            || value.parse().ok(),
            |digits| u64::from_str_radix(digits, 16).ok(),
        )
}
