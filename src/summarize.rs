//! Summarize stage: the transcript through a local Ollama model into Markdown notes.

use std::error::Error;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use crate::config;

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

const MERGE: &str = "Below are meeting notes for consecutive parts of one call, in order. Combine them \
into one set of notes with the same layout and language: \"### Action items\" with every task from all \
parts (merge duplicates, keep owners and due dates), \"### Overview\" with two to four bullets for the \
whole call, then one \"### \" section per topic in the order first discussed, merging sections about \
the same topic from different parts. Keep every decision, number, name and owner and the **bold** marks. \
Do not add anything that is not in the parts.\n\nPARTS:\n";

// Characters per summary part: well inside qwen3:8b's 40k-token window, and small models write
// better notes from shorter input. A model with a bigger window can take more.
const CHUNK_CHARS: usize = 24_000;

/// Summarizes the transcript with `OLLAMA_MODEL` and writes `<base>.md` (summary + transcript).
/// A transcript longer than `SUMMARY_CHUNK_CHARS` (default 24,000) is summarized part by part,
/// then the partial notes are merged in one more pass.
pub(crate) fn notes(base: &str, transcript: &str) -> Result<(), Box<dyn Error>> {
    let llm = config::var("OLLAMA_MODEL").ok_or("set OLLAMA_MODEL (see `ollama list`)")?;
    let max_chars = config::var("SUMMARY_CHUNK_CHARS")
        .and_then(|v| v.parse().ok())
        .unwrap_or(CHUNK_CHARS);
    let parts = split_lines(transcript, max_chars);
    let summary = if parts.len() <= 1 {
        println!("Summarizing with {llm}...");
        ask(&llm, &format!("{PROMPT}{transcript}"))?
    } else {
        // one merge pass; merge in a tree if the joined part notes outgrow the context.
        let mut partials = Vec::new();
        for (i, part) in parts.iter().enumerate() {
            println!("Summarizing part {}/{} with {llm}...", i + 1, parts.len());
            partials.push(ask(&llm, &format!("{PROMPT}{part}"))?);
        }
        println!("Merging {} part summaries...", parts.len());
        ask(&llm, &format!("{MERGE}{}", partials.join("\n\n---\n\n")))?
    };
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

/// Cuts the transcript into parts of at most `max` bytes, only at line ends. A single line longer
/// than `max` becomes a part of its own rather than being cut.
fn split_lines(text: &str, max: usize) -> Vec<&str> {
    let (mut parts, mut start, mut end) = (Vec::new(), 0, 0);
    for line in text.split_inclusive('\n') {
        if end > start && end - start + line.len() > max {
            parts.push(&text[start..end]);
            start = end;
        }
        end += line.len();
    }
    if end > start {
        parts.push(&text[start..end]);
    }
    parts
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
    fn split_lines_keeps_short_transcript_whole() {
        assert_eq!(
            split_lines("[00:01] a\n[00:02] b\n", 100),
            vec!["[00:01] a\n[00:02] b\n"]
        );
        assert!(split_lines("", 100).is_empty());
    }

    #[test]
    fn split_lines_cuts_only_at_line_ends_and_loses_nothing() {
        let text = "[00:01] aaaa\n[00:02] bbbb\n[00:03] cccc\n[00:04] dd";
        let parts = split_lines(text, 26);
        assert_eq!(
            parts,
            vec!["[00:01] aaaa\n[00:02] bbbb\n", "[00:03] cccc\n[00:04] dd"]
        );
        assert_eq!(parts.concat(), text);
    }

    #[test]
    fn split_lines_keeps_an_overlong_line_whole() {
        assert_eq!(
            split_lines("short\nthis line is far too long\nend\n", 10),
            vec!["short\n", "this line is far too long\n", "end\n"]
        );
    }

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
