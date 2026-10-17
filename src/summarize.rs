//! Summarize stage: the transcript through a local Ollama model into Markdown notes.

use std::error::Error;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

const PROMPT: &str = "You are a meeting assistant. Summarize the meeting transcript below (\"Me\" is the user, \"Them\" is everyone else on the call) in Markdown, \
in the same language as the transcript, with sections: ## TL;DR, ## Key points, ## Decisions, \
## Action items (- [ ] owner: task, due if mentioned), ## Open questions. \
Do not invent facts not in the transcript.\n\nTRANSCRIPT:\n";

/// Summarizes the transcript with `OLLAMA_MODEL` and writes `<base>.md` (summary + transcript).
pub(crate) fn notes(base: &str, transcript: &str) -> Result<(), Box<dyn Error>> {
    let llm = std::env::var("OLLAMA_MODEL").map_err(|_| "set OLLAMA_MODEL (see `ollama list`)")?;
    println!("Summarizing with {llm}...");
    let mut child = Command::new("ollama")
        .args([
            "run",
            &llm,
            "--nowordwrap",
            "--think=false",
            "--hidethinking",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    // whole transcript in one prompt; chunk + merge if meetings outgrow the model context.
    child
        .stdin
        .take()
        .unwrap() // stdin was set to Stdio::piped() above, so it is always present
        .write_all(format!("{PROMPT}{transcript}").as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err("ollama failed".into());
    }
    let summary = clean_summary(&String::from_utf8_lossy(&out.stdout)).to_string();
    let notes = format!(
        "{summary}\n\n<details><summary>Transcript</summary>\n\n{transcript}\n</details>\n"
    );
    fs::write(format!("{base}.md"), &notes)?;
    println!("Notes: {base}.md");
    Ok(())
}

/// Keeps only the model's answer: drops leaked reasoning and a ```markdown fence.
fn clean_summary(raw: &str) -> &str {
    // Some GGUF templates leak raw reasoning anyway; keep only what follows it.
    let summary = raw.rsplit("</think>").next().unwrap_or_default().trim();
    // ...and some wrap the answer in a ```markdown fence.
    summary
        .trim_start_matches("```markdown")
        .trim_end_matches("```")
        .trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_summary_drops_leaked_reasoning() {
        let raw = "Analyze request...\n</think>\n\n## TL;DR\nShip Friday.\n";
        assert_eq!(clean_summary(raw), "## TL;DR\nShip Friday.");
    }

    #[test]
    fn clean_summary_drops_markdown_fence() {
        let raw = "```markdown\n## TL;DR\nShip Friday.\n```\n";
        assert_eq!(clean_summary(raw), "## TL;DR\nShip Friday.");
    }

    #[test]
    fn clean_summary_keeps_plain_answer() {
        assert_eq!(
            clean_summary("## TL;DR\nShip Friday."),
            "## TL;DR\nShip Friday."
        );
    }
}
