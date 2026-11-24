use std::fmt::Write as _;

/// Format whitespace without rewriting numbers, duplicate keys, or string escapes.
pub fn json(source: &str) -> Result<String, serde_json::Error> {
    serde_json::from_str::<serde::de::IgnoredAny>(source)?;
    let mut result = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut depth = 0;
    let mut string = false;
    let mut escaped = false;
    let newline = |output: &mut String, depth: usize| {
        let _ = write!(output, "\n{:width$}", "", width = depth * 2);
    };

    while let Some(ch) = chars.next() {
        if string {
            result.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                string = false;
            }
            continue;
        }
        match ch {
            '"' => {
                string = true;
                result.push(ch);
            }
            '{' | '[' => {
                result.push(ch);
                depth += 1;
                while chars.next_if(|next| next.is_ascii_whitespace()).is_some() {}
                if !matches!(chars.peek(), Some('}' | ']')) {
                    newline(&mut result, depth);
                }
            }
            '}' | ']' => {
                depth -= 1;
                if !result.ends_with(['{', '[']) {
                    newline(&mut result, depth);
                }
                result.push(ch);
            }
            ',' => {
                result.push(ch);
                newline(&mut result, depth);
            }
            ':' => result.push_str(": "),
            ch if ch.is_ascii_whitespace() => {}
            ch => result.push(ch),
        }
    }
    if source.ends_with('\n') {
        result.push('\n');
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    #[test]
    fn json_beautify_preserves_key_order_duplicates_and_original_literals() {
        let formatted = super::json(r#"{"z":1e+02,"z":"[  ]\u0061","a":[]}"#).unwrap();
        assert_eq!(
            formatted,
            "{\n  \"z\": 1e+02,\n  \"z\": \"[  ]\\u0061\",\n  \"a\": []\n}"
        );
        assert!(super::json("{\"a\":}").is_err());
    }
}
