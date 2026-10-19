//! Suppress duplicated microphone text from speaker playback (SPEC 6.2).
use std::collections::HashSet;

/// Segment index mask: only microphone rows matching system audio within 1.5 s are removed.
pub fn duplicates(rows: &[(u64, u64, &str, bool)]) -> Vec<bool> {
    let tokens: Vec<HashSet<String>> = rows
        .iter()
        .map(|row| {
            row.2
                .split(|c: char| !c.is_alphanumeric())
                .filter(|word| !word.is_empty())
                .map(str::to_lowercase)
                .collect()
        })
        .collect();
    let mut system: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| !row.3)
        .map(|(i, _)| i)
        .collect();
    system.sort_by_key(|&i| rows[i].0);
    rows.iter()
        .enumerate()
        .map(|(i, mic)| {
            if !mic.3 || tokens[i].is_empty() {
                return false;
            }
            let end = system.partition_point(|&j| rows[j].0 <= mic.1.saturating_add(1500));
            system[..end].iter().any(|&j| {
                if rows[j].1.saturating_add(1500) < mic.0 {
                    return false;
                }
                let common = tokens[i].intersection(&tokens[j]).count();
                let union = tokens[i].len() + tokens[j].len() - common;
                common * 5 >= union * 4
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drops_echo_but_keeps_distinct_speech_and_system_track() {
        let rows = [
            (0, 1000, "Hello, this is the video.", false),
            (200, 1300, "hello THIS is the video!", true),
            (300, 1400, "I disagree with the video", true),
            (2501, 3000, "Hello this is the video", true),
            (0, 1000, "", true),
        ];
        assert_eq!(duplicates(&rows), [false, true, false, false, false]);
    }
    #[test]
    fn threshold_and_tolerance_are_inclusive() {
        assert_eq!(
            duplicates(&[
                (0, 1000, "one two three four five", false),
                (2500, 3000, "one two three four", true),
                (0, 1000, "one two three", true)
            ]),
            [false, true, false]
        );
    }
    #[test]
    fn order_and_unicode_do_not_change_matching() {
        assert_eq!(
            duplicates(&[
                (100, 900, "Xin chào bạn", true),
                (0, 1000, "XIN CHÀO BẠN!", false)
            ]),
            [true, false]
        );
        assert_eq!(
            duplicates(&[(0, 1000, "same words", true), (0, 1000, "same words", true)]),
            [false, false]
        );
    }
}
