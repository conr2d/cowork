//! Built-in workspace instruction templates (v0.2 WP5). The canonical file is
//! AGENTS.md (codex/antigravity read it natively) with a CLAUDE.md symlink for
//! claude. Templates are English; they instruct the agent to follow the user's
//! language, so the body does not need localization.

/// Template body for a preset id; `None` for `blank` (nothing is written).
/// Unknown ids are the caller's validation problem, not handled here.
pub fn template(preset: &str) -> Option<&'static str> {
    match preset {
        "pdf" => Some(PDF_TEMPLATE),
        "proposal" => Some(PROPOSAL_TEMPLATE),
        _ => None,
    }
}

/// Preset ids `workspace create --preset` accepts (mirrors the frontend catalog).
pub const KNOWN_PRESETS: [&str; 3] = ["blank", "pdf", "proposal"];

const PDF_TEMPLATE: &str = r#"# Workspace: Document Translation

This workspace is a translation station. When the user drops a document into
`files/` and asks for a translation:

1. Read the source document in `files/`.
2. Translate into the language the user asks for; if unspecified, use the
   language the user writes in.
3. Preserve the original structure — headings, lists, tables, emphasis.
4. Write the result into `files/` as `<original-name>.<lang>.md`; never
   overwrite the source file.
5. Keep technical terms, proper nouns, and numbers accurate; when a term is
   ambiguous, keep the original in parentheses.

Do not create or modify anything outside `files/`.
"#;

const PROPOSAL_TEMPLATE: &str = r#"# Workspace: Proposal Drafting

This workspace drafts and iterates on proposals. When the user describes a
goal or drops reference material into `files/`:

1. Draft or revise the proposal in `files/proposal.md` (create it if missing).
2. Structure: summary, background, proposal, plan, expected impact.
3. Write the document in the language the user writes in.
4. Revise `files/proposal.md` in place rather than creating copies; the user
   relies on one living document.
5. Reference material lives in `files/`; reflect it where used.

Do not create or modify anything outside `files/`.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_presets_return_expected_templates() {
        assert!(
            template("pdf")
                .expect("pdf template")
                .starts_with("# Workspace: Document Translation")
        );
        assert!(
            template("proposal")
                .expect("proposal template")
                .starts_with("# Workspace: Proposal Drafting")
        );
    }

    #[test]
    fn blank_and_unknown_have_no_template() {
        assert!(template("blank").is_none());
        assert!(template("nope").is_none());
    }
}
