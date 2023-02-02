use indexmap::IndexMap;

pub(crate) fn parse_key_val_properties(text: &[String]) -> IndexMap<String, String> {
    // TODO: warn on duplicate values?
    text.into_iter()
        .map(String::as_str)
        .map(parse_key_val)
        .collect()
}

/// Parse a key-value pair in the form of `key=value`.
pub(crate) fn parse_key_val(text: &str) -> (String, String) {
    let mut parts = text.splitn(2, '=');
    let key = parts.next().unwrap().to_string();
    let value = parts.next().unwrap().to_string();
    (key, value)
}
