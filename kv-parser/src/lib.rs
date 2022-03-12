//! This library parses the key-value pair text format that the jvm uses.
//! It uses it for various files, most notably the manifest file and the index file.
//! It is broken up into sections, which are just separated by an extra newline.
//! These sections have key/name and values, which are essentially strings until you actually parse
//! them.
//!
//! See: https://docs.oracle.com/javase/7/docs/technotes/guides/jar/jar.html for more.

// TODO: We could make a version that uses iterators, but this is easier
// an iterator version that did no allocations would also be harder, because of how the line
// breaks work and so would require some custom slice structure?
// This is probably not anywhere near a performance bottleneck, though, but it would be nice to
// make it not do as much expensive operations.

// TODO: It wouldn't be that hard to make a writer for this, if we needed that.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct KeyValueData {
    // TODO: Should this just be a HashMap<(Index, String), String>?
    // The issue with that is we would probably need something like IndexMap's equivalence trait
    // Also it makes it less obvious that there is actually an ordering, and probably is harder
    // to iterate over, if needed.
    data: Vec<HashMap<String, String>>,
}
impl KeyValueData {
    pub fn get(&self, index: usize, id: &str) -> Option<&str> {
        self.data
            .get(index)
            .and_then(|map| map.get(id))
            .map(String::as_str)
    }

    /// The number of sections
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum KeyValueParseError {
    ExpectedNewline,
    /// We expected the starting character for a name but got nothing
    ExpectedInitialNameCharacterGotEof,
    /// We expected the starting character for a name to be alphanumeric
    ExpectedInitialNameCharacterAlphanumeric(char),
    /// We expected this character to come next
    Expected(char),
    ValueContainedNull,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyValueWarning<'a> {
    DuplicateKey(&'a str),
}

pub fn parse_keyvalue_data<'a>(
    mut input_data: &'a str,
    mut warning_output: impl FnMut(KeyValueWarning<'a>),
) -> Result<KeyValueData, KeyValueParseError> {
    let mut result = KeyValueData { data: Vec::new() };

    // TODO: Can there be empty sections?

    loop {
        let mut section = HashMap::new();
        // If we immediately get a newline, then this is an empty section
        if let Ok(d) = expect_newline(input_data) {
            input_data = d;
        } else {
            loop {
                let d = input_data;
                // alphanum *(alphanum | - | _)
                let (d, name) = parse_name(d)?;

                let d = expect(d, ':')?;
                // SPACE
                let d = expect(d, ' ')?;

                let (d, value) = parse_value(d)?;

                if section.insert(name.to_owned(), value.to_owned()).is_some() {
                    warning_output(KeyValueWarning::DuplicateKey(name));
                }

                if let Ok(d) = expect_newline(d) {
                    // We got an extra newline
                    // This means the section is over
                    input_data = d;
                    break;
                } else {
                    input_data = d;
                }
            }
        }

        result.data.push(section);

        if input_data.is_empty() {
            break;
        }
    }

    Ok(result)
}

fn parse_otherchars(data: &str) -> Result<(&str, &str), KeyValueParseError> {
    let mut end = 0;
    for (i, c) in data.char_indices() {
        if c == '\0' {
            return Err(KeyValueParseError::ValueContainedNull);
        }

        let cur_data = &data[i..];
        if let Ok(after_data) = expect_newline(cur_data) {
            // We found a newline

            // The data before the newline
            let found_data = &data[..=end];

            return Ok((after_data, found_data));
        }

        end = i;
    }

    Err(KeyValueParseError::ExpectedNewline)
}

/// the beginning space should already be parsed
fn parse_value(data: &str) -> Result<(&str, String), KeyValueParseError> {
    let (data, initial_data) = parse_otherchars(data)?;

    if let Some(data) = data.strip_prefix(' ') {
        // It is a continuation!
        let (data, continuation) = parse_otherchars(data)?;

        let content = format!("{}{}", initial_data, continuation);

        Ok((data, content))
    } else {
        Ok((data, initial_data.to_owned()))
    }
}

// TODO: This parsing is purely ascii, so we could skip any utf8 validation
// It wouldn't be too hard, we just didn't actively do it since it was easier to write
/// Returns (data, name)
fn parse_name(data: &str) -> Result<(&str, &str), KeyValueParseError> {
    let first = data
        .chars()
        .next()
        .ok_or(KeyValueParseError::ExpectedInitialNameCharacterGotEof)?;
    if !first.is_ascii_alphanumeric() {
        return Err(KeyValueParseError::ExpectedInitialNameCharacterAlphanumeric(first));
    }

    let start = 0;
    let mut end = 1;
    for (i, x) in data.char_indices().skip(1) {
        if x.is_ascii_alphanumeric() || x == '-' || x == '_' {
            end = i + x.len_utf8();
        } else {
            // It was not a valid character, we'll let the thing which is parsing us
            // handle it
            break;
        }
    }

    Ok((&data[end..], &data[start..end]))
}

fn expect_newline(data: &str) -> Result<&str, KeyValueParseError> {
    if let Some(data) = data.strip_prefix("\r\n") {
        Ok(data)
    } else if let Some(data) = data.strip_prefix('\n') {
        Ok(data)
    } else if let Some(data) = data.strip_prefix('\r') {
        Ok(data)
    } else {
        Err(KeyValueParseError::ExpectedNewline)
    }
}

fn expect(data: &str, c: char) -> Result<&str, KeyValueParseError> {
    if let Some(data) = data.strip_prefix(c) {
        Ok(data)
    } else {
        Err(KeyValueParseError::Expected(c))
    }
}

#[cfg(test)]
mod tests {
    use crate::{parse_keyvalue_data, parse_name, KeyValueParseError};

    #[test]
    fn test_name_parsing() {
        assert_eq!(parse_name("ABCde"), Ok(("", "ABCde")));
        assert_eq!(
            parse_name(""),
            Err(KeyValueParseError::ExpectedInitialNameCharacterGotEof)
        );
        assert_eq!(parse_name("AB3C-e"), Ok(("", "AB3C-e")));
        assert_eq!(
            parse_name("-asdf"),
            Err(KeyValueParseError::ExpectedInitialNameCharacterAlphanumeric('-'))
        );
        assert_eq!(parse_name("a"), Ok(("", "a")));
        assert_eq!(parse_name("a: asdf"), Ok((": asdf", "a")));
    }

    #[test]
    fn test_simple_file_parsing() {
        let basic_file = "Manifest-Version: 1.0\nCreated-By: 1.8.0_332 (Oracle Corporation)\n\n";

        let result = parse_keyvalue_data(basic_file, |_| {}).unwrap();
        assert_eq!(result.get(0, "Manifest-Version"), Some("1.0"));
        assert_eq!(
            result.get(0, "Created-By"),
            Some("1.8.0_332 (Oracle Corporation)")
        );
    }

    #[test]
    fn test_basic_file_parsing() {
        let basic_file = "Manifest-Version: 1.0\nMain-Class: com.abcdefghijklmn.abcdefghijklmnopqrstu.oabcdef.Oabcdefgihi\n jklm\nSpecification-Title: Some Program\nSpecification-Version: 1.1.2\nImplementation-Version: 588\n\n";

        let result = parse_keyvalue_data(basic_file, |_| {}).unwrap();
        assert_eq!(result.get(0, "Manifest-Version"), Some("1.0"));
        assert_eq!(
            result.get(0, "Main-Class"),
            Some("com.abcdefghijklmn.abcdefghijklmnopqrstu.oabcdef.Oabcdefgihijklm")
        );
        assert_eq!(result.get(0, "Specification-Title"), Some("Some Program"));
        assert_eq!(result.get(0, "Specification-Version"), Some("1.1.2"));
        assert_eq!(result.get(0, "Implementation-Version"), Some("588"));
    }

    #[test]
    fn test_complex_file_parsing() {
        let complex_file2 = "Manifest-Version: 1.0\nCreated-By: 1.3.1 (Things)\n\nName: thing/firstclass.class\nSHA-256-Digest: data1\n\nName: thing/secondclass.class\nSHA1-Digest: somedata\nSHA-256-Digest: data\n\n";

        let result = parse_keyvalue_data(complex_file2, |_| {}).unwrap();
        assert_eq!(result.get(0, "Manifest-Version"), Some("1.0"));
        assert_eq!(result.get(0, "Created-By"), Some("1.3.1 (Things)"));

        assert_eq!(result.get(1, "Name"), Some("thing/firstclass.class"));
        assert_eq!(result.get(1, "SHA-256-Digest"), Some("data1"));

        assert_eq!(result.get(2, "Name"), Some("thing/secondclass.class"));
        assert_eq!(result.get(2, "SHA1-Digest"), Some("somedata"));
        assert_eq!(result.get(2, "SHA-256-Digest"), Some("data"));
    }
}
