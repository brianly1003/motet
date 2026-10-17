//! Summarize stage: the transcript through a local Ollama model into Markdown notes.

use std::error::Error;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

const PROMPT: &str = "You write meeting notes from a call transcript. \"Me\" is the user and \"Them\" is \
everyone else on the call; when the transcript says people's names, use the names. \
Write the notes in the language most of the transcript is in. Output Markdown only, in this order:\n\
1. The heading \"### Action items\", then one \"- [ ] \" line per follow-up task someone agreed to \
or was asked to do; a proposed next meeting is a task too. Start with the owner's name only when the \
transcript shows who takes the task. The owner is the person who does the work, not the person who \
receives it: \"Me: I will send the report to Anna\" means Me sends it, for Anna. News and facts are \
not tasks. Add a due date only if one was said. Only when there are no tasks at all, write \"- None\".\n\
2. The heading \"### Overview\", then two to four bullets on what the call covered and who is involved.\n\
3. One \"### \" section per main topic, titled with the topic, in the order discussed. Under each, \
bullets on what was said, what was decided and why. Put **bold** on decisions, numbers, names and key terms.\n\
Do not invent facts, owners or dates. Leave out small talk and anything unrelated to the meeting.\n\nTRANSCRIPT:\n";

/// Summarizes the transcript with `OLLAMA_MODEL` and writes `<base>.md` (summary + transcript).
pub(crate) fn notes(base: &str, transcript: &str) -> Result<(), Box<dyn Error>> {
    let llm = std::env::var("OLLAMA_MODEL").map_err(|_| "set OLLAMA_MODEL (see `ollama list`)")?;
    println!("Summarizing with {llm}...");
    // whole transcript in one prompt; chunk + merge if meetings outgrow the model context.
    let summary = ask(&llm, &format!("{PROMPT}{transcript}"))?;
    let notes = format!(
        "{summary}\n\n<details><summary>Transcript</summary>\n\n{transcript}\n</details>\n"
    );
    fs::write(format!("{base}.md"), &notes)?;
    println!("Notes: {base}.md");
    Ok(())
}

/// Runs one prompt through `ollama run <llm>` and returns the cleaned answer.
pub(crate) fn ask(llm: &str, prompt: &str) -> Result<String, Box<dyn Error>> {
    let mut child = Command::new("ollama")
        .args([
            "run",
            llm,
            "--nowordwrap",
            "--think=false",
            "--hidethinking",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .unwrap() // stdin was set to Stdio::piped() above, so it is always present
        .write_all(prompt.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err("ollama failed".into());
    }
    Ok(clean_summary(&String::from_utf8_lossy(&out.stdout)).to_string())
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
