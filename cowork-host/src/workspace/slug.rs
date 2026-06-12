use cowork_errors::{Code, Envelope, Stage};

pub fn slug_from_name(name: &str, existing: &[String]) -> Result<String, Envelope> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(
            Envelope::new(Code::WorkspaceInvalidName, Stage::Workspace).with_context("name", name)
        );
    }

    let mut mapped = String::new();
    let mut last_was_dash = false;
    for ch in trimmed.to_lowercase().chars() {
        if ch.is_alphanumeric() {
            mapped.push(ch);
            last_was_dash = false;
        } else if ch == '-' || ch == '_' {
            mapped.push(ch);
            last_was_dash = ch == '-';
        } else if ch.is_whitespace() && !last_was_dash {
            mapped.push('-');
            last_was_dash = true;
        }
    }

    let collapsed = collapse_dashes(&mapped);
    let mut base = collapsed
        .trim_matches(['-', '_'])
        .chars()
        .take(40)
        .collect::<String>();
    if base.is_empty() {
        base = "workspace".to_string();
    }
    if !existing.iter().any(|s| s == &base) {
        return Ok(base);
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !existing.iter().any(|s| s == &candidate) {
            return Ok(candidate);
        }
        n += 1;
    }
}

fn collapse_dashes(input: &str) -> String {
    let mut out = String::new();
    let mut last_was_dash = false;
    for ch in input.chars() {
        if ch == '-' {
            if !last_was_dash {
                out.push(ch);
            }
            last_was_dash = true;
        } else {
            out.push(ch);
            last_was_dash = false;
        }
    }
    out
}
