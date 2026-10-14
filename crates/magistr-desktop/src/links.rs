//! Authored links may open only web addresses in the system browser. The
//! privileged conversation window accepts only its bundled document.
use tauri_plugin_opener::OpenerExt;

pub(crate) fn web_address(value: &str) -> Option<tauri::Url> {
    if value.len() > 8192 || value.chars().any(char::is_control) || value.trim() != value {
        return None;
    }
    let url = tauri::Url::parse(value).ok()?;
    (matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none())
    .then_some(url)
}

pub(crate) fn local_document(url: &tauri::Url, use_https: bool) -> bool {
    let (scheme, host) = if cfg!(any(windows, target_os = "android")) {
        (if use_https { "https" } else { "http" }, "tauri.localhost")
    } else {
        ("tauri", "localhost")
    };
    url.scheme() == scheme
        && url.host_str() == Some(host)
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.path(), "" | "/" | "/index.html")
        && url.query().is_none()
}

#[tauri::command]
pub(crate) fn open_link(app: tauri::AppHandle, destination: String) -> Result<(), String> {
    let url = web_address(&destination).ok_or("Only web links can be opened.")?;
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|error| format!("Could not open the link: {error}"))
}

#[cfg(test)]
#[path = "links.tests.rs"]
mod tests;
