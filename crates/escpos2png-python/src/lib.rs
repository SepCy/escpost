//! Python bindings for escpos2png.

use escpos2png::render as render_escpos;
use escpos2png_profiles::compile_profile;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/upstream/escpos-printer-db/dist/capabilities.json");
const NT_5890K_ENRICHMENT: &str = include_str!("../../../profiles/enrichments/NT-5890K.toml");

#[pyfunction]
#[pyo3(signature = (data, *, profile))]
fn render<'py>(py: Python<'py>, data: &[u8], profile: &str) -> PyResult<Vec<Bound<'py, PyBytes>>> {
    let data = data.to_vec();
    let profile = profile.to_owned();
    let result = py.detach(move || render_png_sheets(&data, &profile));
    let sheets = result.map_err(BindingError::into_py_err)?;

    Ok(sheets.iter().map(|sheet| PyBytes::new(py, sheet)).collect())
}

fn render_png_sheets(data: &[u8], profile: &str) -> Result<Vec<Vec<u8>>, BindingError> {
    let enrichment = match profile {
        "NT-5890K" => NT_5890K_ENRICHMENT,
        profile => return Err(BindingError::UnknownProfile(profile.to_owned())),
    };
    let profile = compile_profile(CAPABILITIES_JSON, enrichment)
        .map_err(|error| BindingError::CompileProfile(error.to_string()))?
        .profile;
    let rendered =
        render_escpos(data, &profile).map_err(|error| BindingError::Render(error.to_string()))?;

    Ok(rendered.sheets.into_iter().map(|sheet| sheet.png).collect())
}

#[derive(Debug)]
enum BindingError {
    UnknownProfile(String),
    CompileProfile(String),
    Render(String),
}

impl BindingError {
    fn into_py_err(self) -> PyErr {
        match self {
            Self::UnknownProfile(profile) => {
                PyValueError::new_err(format!("unknown printer profile {profile:?}"))
            }
            Self::CompileProfile(message) => {
                PyRuntimeError::new_err(format!("could not load printer profile: {message}"))
            }
            Self::Render(message) => PyRuntimeError::new_err(message),
        }
    }
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(render, module)?)?;
    Ok(())
}
