use axum::extract::{Request, State};
use axum::http::header;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::ApiState;
use super::error::ApiFailure;

/// One per browser engine. A scheme missing here is a browser whose extension
/// is refused before anything else can go wrong.
const EXTENSION_SCHEMES: [&str; 3] = [
    "chrome-extension://",     // Chrome, Edge, and other Chromium builds
    "moz-extension://",        // Firefox
    "safari-web-extension://", // Safari
];

/// Accept an extension, a local process, or nothing; reject every other origin.
///
/// A negative filter is all this can be, because an absent `Origin` has to be
/// accepted and so nothing can be proved to come from the extension. What it
/// buys is that a remote page cannot reach the print route.
pub(super) fn origin_allowed(origin: Option<&str>, pinned_extension_id: Option<&str>) -> bool {
    // curl, a local backend, or an extension GET.
    let Some(origin) = origin else {
        return true;
    };
    if origin == "null" {
        return true;
    }
    let Some(id) = extension_id(origin) else {
        return false;
    };
    // An origin has no path; a slash here means this is not one.
    if id.is_empty() || id.contains('/') {
        return false;
    }
    match pinned_extension_id {
        Some(expected) => id == expected,
        // The id's shape is deliberately not validated. The header is
        // unauthenticated either way, so strictness buys nothing and a
        // reinstall changing the id would lock the extension out.
        None => true,
    }
}

/// The id an extension origin carries, or None when the origin is not one.
fn extension_id(origin: &str) -> Option<&str> {
    EXTENSION_SCHEMES
        .iter()
        .find_map(|scheme| origin.strip_prefix(scheme))
}

pub(super) async fn guard(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    if origin_allowed(origin, state.extension_id.as_deref()) {
        return next.run(request).await;
    }
    ApiFailure::origin_not_granted(origin.unwrap_or_default()).into_response()
}

#[cfg(test)]
mod tests {
    use super::origin_allowed;

    #[test]
    fn an_absent_origin_is_accepted_because_local_processes_send_none() {
        // L3: curl, a Node backend, a cron job. Chrome also sends no Origin on
        // an extension's GET requests, so rejecting this would break the
        // extension's own printer list.
        assert!(origin_allowed(None, None));
    }

    #[test]
    fn a_null_origin_is_accepted() {
        assert!(origin_allowed(Some("null"), None));
    }

    #[test]
    fn every_browser_engine_has_its_own_extension_scheme() {
        for origin in [
            "chrome-extension://cnifebiebidolpmlmgcghpopggfcklmc",
            "moz-extension://a1b2c3d4-0000-4000-8000-000000000000",
            "safari-web-extension://A1B2C3D4-0000-4000-8000-000000000000",
        ] {
            assert!(
                origin_allowed(Some(origin), None),
                "{origin} should be accepted"
            );
        }
    }

    #[test]
    fn a_scheme_that_merely_resembles_an_extension_is_rejected() {
        assert!(!origin_allowed(Some("web-extension://abc"), None));
        assert!(!origin_allowed(Some("https://moz-extension.example"), None));
    }

    #[test]
    fn pinning_an_id_applies_whatever_the_scheme() {
        assert!(origin_allowed(
            Some("moz-extension://the-id"),
            Some("the-id")
        ));
        assert!(!origin_allowed(
            Some("moz-extension://another-id"),
            Some("the-id")
        ));
    }

    #[test]
    fn any_extension_origin_is_accepted_when_none_is_pinned() {
        assert!(origin_allowed(
            Some("chrome-extension://cnifebiebidolpmlmgcghpopggfcklmc"),
            None
        ));
        assert!(origin_allowed(
            Some("chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            None
        ));
    }

    #[test]
    fn a_web_page_is_rejected() {
        for origin in [
            "https://evil.example",
            "http://localhost:3000",
            "https://127.0.0.1",
            "file://",
        ] {
            assert!(
                !origin_allowed(Some(origin), None),
                "{origin} should be rejected"
            );
        }
    }

    #[test]
    fn a_web_origin_that_merely_mentions_the_scheme_is_rejected() {
        assert!(!origin_allowed(
            Some("https://evil.example/chrome-extension://x"),
            None
        ));
    }

    #[test]
    fn an_extension_origin_with_no_id_is_rejected() {
        assert!(!origin_allowed(Some("chrome-extension://"), None));
        assert!(!origin_allowed(Some("chrome-extension://a/b"), None));
    }

    #[test]
    fn pinning_an_id_narrows_to_exactly_that_extension() {
        let pinned = Some("cnifebiebidolpmlmgcghpopggfcklmc");
        assert!(origin_allowed(
            Some("chrome-extension://cnifebiebidolpmlmgcghpopggfcklmc"),
            pinned
        ));
        assert!(!origin_allowed(
            Some("chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            pinned
        ));
    }

    #[test]
    fn pinning_still_accepts_local_processes() {
        // Pinning narrows which extension may call, not whether a local
        // backend may. L1–L4 do not depend on an extension being installed.
        let pinned = Some("cnifebiebidolpmlmgcghpopggfcklmc");
        assert!(origin_allowed(None, pinned));
        assert!(origin_allowed(Some("null"), pinned));
    }
}
