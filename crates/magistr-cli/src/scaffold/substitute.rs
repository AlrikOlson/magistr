use std::collections::HashMap;

/// Substitute `{{key}}` placeholders in a template string with values from the map.
///
/// Keys not found in the map are left as-is.
pub fn substitute(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        let placeholder = format!("{{{{{key}}}}}");
        result = result.replace(&placeholder, value);
    }
    result
}
