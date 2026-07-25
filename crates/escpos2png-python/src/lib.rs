//! Python bindings for escpos2png.

use escpos2png::{
    Completeness, DeviceEvent, DiagnosticEffect, DiagnosticSeverity, InitialStateAssumption,
    RenderResult, render as render_escpos,
};
use escpos2png_profiles::{PrinterProfile, ProfilePack, from_canonical_profile_pack_json};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};
use std::sync::OnceLock;

const PROFILE_PACK_JSON: &[u8] = include_bytes!("../../../profiles/generated/profiles.json");
static PROFILE_PACK: OnceLock<ProfilePack> = OnceLock::new();

#[derive(Debug)]
enum BindingError {
    UnknownProfile(String),
    LoadProfiles(String),
    Render(String),
}

#[pyfunction]
#[pyo3(signature = (data, *, profile))]
fn render<'py>(py: Python<'py>, data: &[u8], profile: &str) -> PyResult<Vec<Bound<'py, PyBytes>>> {
    let data = data.to_vec();
    let profile = profile.to_owned();
    let rendered = py
        .detach(move || render_with_profile(&data, &profile))
        .map_err(BindingError::into_py_err)?;

    Ok(rendered
        .sheets
        .iter()
        .map(|sheet| PyBytes::new(py, &sheet.png))
        .collect())
}

#[pyfunction]
#[pyo3(signature = (data, *, profile))]
fn render_result<'py>(py: Python<'py>, data: &[u8], profile: &str) -> PyResult<Bound<'py, PyDict>> {
    let data = data.to_vec();
    let profile = profile.to_owned();
    let rendered = py
        .detach(move || render_with_profile(&data, &profile))
        .map_err(BindingError::into_py_err)?;

    render_result_to_python(py, &rendered)
}

fn render_with_profile(data: &[u8], profile: &str) -> Result<RenderResult, BindingError> {
    let profile = load_profile(profile)?;
    let rendered =
        render_escpos(data, profile).map_err(|error| BindingError::Render(error.to_string()))?;

    Ok(rendered)
}

fn load_profile(profile_id: &str) -> Result<&'static PrinterProfile, BindingError> {
    if PROFILE_PACK.get().is_none() {
        let profile_pack = from_canonical_profile_pack_json(PROFILE_PACK_JSON)
            .map_err(|error| BindingError::LoadProfiles(error.to_string()))?;
        // Another render may win this race. Either verified pack is identical,
        // and the OnceLock gives every caller the one stored instance.
        let _ = PROFILE_PACK.set(profile_pack);
    }

    let profile_pack = PROFILE_PACK.get().ok_or_else(|| {
        BindingError::LoadProfiles("profile-pack initialization did not complete".to_owned())
    })?;
    profile_pack
        .get(profile_id)
        .ok_or_else(|| BindingError::UnknownProfile(profile_id.to_owned()))
}

fn render_result_to_python<'py>(
    py: Python<'py>,
    rendered: &RenderResult,
) -> PyResult<Bound<'py, PyDict>> {
    let result = PyDict::new(py);
    let sheets = rendered
        .sheets
        .iter()
        .map(|sheet| PyBytes::new(py, &sheet.png));
    result.set_item("sheets", PyList::new(py, sheets)?)?;
    result.set_item(
        "device_events",
        device_events_to_python(py, &rendered.device_events)?,
    )?;
    result.set_item(
        "diagnostics",
        diagnostics_to_python(py, &rendered.diagnostics)?,
    )?;
    result.set_item("completeness", completeness_name(rendered.completeness))?;
    result.set_item("metadata", metadata_to_python(py, rendered)?)?;
    Ok(result)
}

fn device_events_to_python<'py>(
    py: Python<'py>,
    events: &[DeviceEvent],
) -> PyResult<Bound<'py, PyList>> {
    let result = PyList::empty(py);
    for event in events {
        let item = PyDict::new(py);
        match event {
            DeviceEvent::CashDrawerPulse {
                connector,
                on_time_units,
                off_time_units,
            } => {
                item.set_item("type", "cash_drawer_pulse")?;
                item.set_item("connector", connector)?;
                item.set_item("on_time_units", on_time_units)?;
                item.set_item("off_time_units", off_time_units)?;
            }
        }
        result.append(item)?;
    }
    Ok(result)
}

fn diagnostics_to_python<'py>(
    py: Python<'py>,
    diagnostics: &[escpos2png::Diagnostic],
) -> PyResult<Bound<'py, PyList>> {
    let result = PyList::empty(py);
    for diagnostic in diagnostics {
        let item = PyDict::new(py);
        item.set_item("severity", diagnostic_severity_name(diagnostic.severity))?;
        item.set_item("byte_offset", diagnostic.byte_offset)?;
        item.set_item("command", diagnostic.command)?;
        item.set_item("message", &diagnostic.message)?;
        item.set_item("effect", diagnostic_effect_name(diagnostic.effect))?;
        result.append(item)?;
    }
    Ok(result)
}

fn metadata_to_python<'py>(
    py: Python<'py>,
    rendered: &RenderResult,
) -> PyResult<Bound<'py, PyDict>> {
    let metadata = PyDict::new(py);
    metadata.set_item("renderer_version", rendered.metadata.renderer_version)?;
    metadata.set_item("profile_id", &rendered.metadata.profile_id)?;
    metadata.set_item("profile_revision", rendered.metadata.profile_revision)?;
    metadata.set_item(
        "canonical_profile_sha256",
        &rendered.metadata.canonical_profile_sha256,
    )?;
    metadata.set_item(
        "upstream_repository",
        &rendered.metadata.upstream_repository,
    )?;
    metadata.set_item("upstream_commit", &rendered.metadata.upstream_commit)?;
    metadata.set_item(
        "upstream_profile_sha256",
        &rendered.metadata.upstream_profile_sha256,
    )?;
    metadata.set_item("enrichment_sha256", &rendered.metadata.enrichment_sha256)?;
    metadata.set_item(
        "initial_state",
        initial_state_name(rendered.metadata.initial_state),
    )?;
    Ok(metadata)
}

fn completeness_name(completeness: Completeness) -> &'static str {
    match completeness {
        Completeness::Complete => "complete",
        Completeness::CompleteWithNonVisualEvents => "complete_with_non_visual_events",
    }
}

fn diagnostic_severity_name(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Information => "information",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Error => "error",
    }
}

fn diagnostic_effect_name(effect: DiagnosticEffect) -> &'static str {
    match effect {
        DiagnosticEffect::None => "none",
        DiagnosticEffect::NonVisualBehaviorOnly => "non_visual_behavior_only",
        DiagnosticEffect::VisualOutputIncomplete => "visual_output_incomplete",
        DiagnosticEffect::ParsingAborted => "parsing_aborted",
    }
}

fn initial_state_name(initial_state: InitialStateAssumption) -> &'static str {
    match initial_state {
        InitialStateAssumption::ProfileResetDefaults => "profile_reset_defaults",
    }
}

impl BindingError {
    fn into_py_err(self) -> PyErr {
        match self {
            Self::UnknownProfile(profile) => {
                PyValueError::new_err(format!("unknown printer profile {profile:?}"))
            }
            Self::LoadProfiles(message) => {
                PyRuntimeError::new_err(format!("could not load canonical profile pack: {message}"))
            }
            Self::Render(message) => PyRuntimeError::new_err(message),
        }
    }
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(render, module)?)?;
    module.add_function(wrap_pyfunction!(render_result, module)?)?;
    Ok(())
}
