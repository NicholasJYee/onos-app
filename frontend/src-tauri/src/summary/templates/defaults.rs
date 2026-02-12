/// Embedded default templates using compile-time inclusion
///
/// These templates are bundled into the binary and serve as fallbacks
/// when custom templates are not available.

/// Follow-up note template (e.g. orthopaedic follow-ups)
pub const FOLLOW_UPS: &str = include_str!("../../../templates/follow_ups.json");

/// Consult note template (e.g. orthopaedic consults)
pub const CONSULTS: &str = include_str!("../../../templates/consults.json");

/// Registry of all built-in templates
///
/// Maps template identifiers to their embedded JSON content
pub fn get_builtin_templates() -> Vec<(&'static str, &'static str)> {
    vec![
        ("follow_ups", FOLLOW_UPS),
        ("consults", CONSULTS),
    ]
}

/// Get a built-in template by identifier
///
/// # Arguments
/// * `id` - Template identifier (e.g., "follow_ups", "consults")
///
/// # Returns
/// The template JSON content if found, None otherwise
pub fn get_builtin_template(id: &str) -> Option<&'static str> {
    match id {
        "follow_ups" => Some(FOLLOW_UPS),
        "consults" => Some(CONSULTS),
        _ => None,
    }
}

/// List all built-in template identifiers
pub fn list_builtin_template_ids() -> Vec<&'static str> {
    vec!["follow_ups", "consults"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_templates_valid_json() {
        for (id, content) in get_builtin_templates() {
            let result = serde_json::from_str::<serde_json::Value>(content);
            assert!(
                result.is_ok(),
                "Built-in template '{}' contains invalid JSON: {:?}",
                id,
                result.err()
            );
        }
    }

    #[test]
    fn test_get_builtin_template() {
        assert!(get_builtin_template("follow_ups").is_some());
        assert!(get_builtin_template("consults").is_some());
        assert!(get_builtin_template("nonexistent").is_none());
    }
}
