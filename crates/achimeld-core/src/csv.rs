//! Strict bounded CSV shared by application and project localization.
pub fn records(text: &str) -> Result<Vec<Vec<String>>, String> {
    if text.len() > 64 * 1024 * 1024 {
        return Err("CSV exceeds its byte limit".into());
    }
    #[derive(PartialEq)]
    enum State {
        Start,
        Plain,
        Quoted,
        Closed,
    }
    let mut state = State::Start;
    let mut records = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = text.trim_start_matches('\u{feff}').chars().peekable();
    while let Some(c) = chars.next() {
        if state == State::Quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    state = State::Closed;
                }
            } else {
                field.push(c);
            }
        } else if c == ',' {
            row.push(std::mem::take(&mut field));
            state = State::Start;
        } else if c == '\r' || c == '\n' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut field));
            if row.len() > 128 {
                return Err("CSV exceeds its row or field limit".into());
            }
            records.push(std::mem::take(&mut row));
            state = State::Start;
        } else if c == '"' && state == State::Start {
            state = State::Quoted;
        } else if state == State::Closed || c == '"' {
            return Err("Invalid quoting in CSV field".into());
        } else {
            field.push(c);
            state = State::Plain;
        }
        if field.len() > 1_048_576 || row.len() > 128 || records.len() > 100_001 {
            return Err("CSV exceeds its row or field limit".into());
        }
    }
    if state == State::Quoted {
        return Err("Unterminated quoted CSV field".into());
    }
    if !field.is_empty() || !row.is_empty() || state != State::Start {
        row.push(field);
        if row.len() > 128 || records.len() >= 100_001 {
            return Err("CSV exceeds its row or field limit".into());
        }
        records.push(row);
    }
    Ok(records)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn limits_apply_to_the_final_record_without_a_newline() {
        assert!(records(&vec!["x"; 128].join(",")).is_ok());
        assert!(records(&vec!["x"; 129].join(",")).is_err());
        assert!(records(&(vec!["x"; 129].join(",") + "\n")).is_err());
        assert!(records(&("x\n".repeat(100_001) + "x")).is_err());
    }
    #[test]
    fn quoted_lines_and_unicode_are_preserved_and_malformed_quotes_are_rejected() {
        assert_eq!(
            records("a,\"a,\"\"b\nż\"\r\nc,d\r\n").unwrap(),
            vec![vec!["a", "a,\"b\nż"], vec!["c", "d"]]
        );
        assert!(records("bad\"quote,x").is_err());
        assert!(records("\"closed\"garbage,x").is_err());
        assert!(records("\"unfinished").is_err());
    }
}
