//! Pure text handling shared by every dictation path.
//!
//! These functions are deliberately dependency-free and deterministic so they
//! can be tested without a microphone, a model, or a window. The Windows shell
//! calls them after final transcription and before paste.

/// Whitespace and punctuation cleanup for a finished transcript.
///
/// - trims outer whitespace
/// - normalizes line endings and horizontal whitespace
/// - removes spaces before punctuation and after opening brackets
/// - adds a missing space after sentence punctuation
/// - capitalizes the first letter of each sentence and line
/// - appends one trailing space unless the text ends with a newline
pub fn clean_text(input: &str, trailing_space: bool) -> String {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(trimmed.len() + 1);
    let mut capitalize_next = true;

    for raw_line in trimmed.split('\n') {
        let line = raw_line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() {
            // A blank source line is a paragraph break. Keep it as two
            // newlines even when the previous line did not end with one.
            while out.ends_with(' ') {
                out.pop();
            }
            if !out.is_empty() {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                if !out.ends_with("\n\n") {
                    out.push('\n');
                }
            }
            capitalize_next = true;
            continue;
        }

        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }

        let mut chars = line.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == ' ' {
                let next = chars.peek().copied();
                if matches!(
                    next,
                    Some(',' | '.' | '!' | '?' | ';' | ':' | ')' | ']' | '}')
                ) {
                    continue;
                }
                if out.ends_with(' ') || out.ends_with('\n') {
                    continue;
                }
                out.push(' ');
                continue;
            }

            if matches!(ch, '(' | '[' | '{') {
                out.push(ch);
                while chars.peek() == Some(&' ') {
                    chars.next();
                }
                continue;
            }

            if capitalize_next && ch.is_alphabetic() {
                out.extend(ch.to_uppercase());
                capitalize_next = false;
                continue;
            }

            out.push(ch);
            if matches!(ch, '.' | '!' | '?') {
                capitalize_next = true;
            } else if ch.is_alphanumeric() {
                capitalize_next = false;
            }
        }

        // "hello,world" -> "hello, world"; leave decimals and times alone.
        let mut repaired = String::with_capacity(out.len() + 1);
        let chars: Vec<char> = out.chars().collect();
        let mut index = 0;
        while index < chars.len() {
            let ch = chars[index];
            repaired.push(ch);
            if matches!(ch, ',' | ';' | ':')
                && chars
                    .get(index + 1)
                    .is_some_and(|next| next.is_alphanumeric())
                && !chars
                    .get(index.wrapping_sub(1))
                    .is_some_and(|previous| previous.is_ascii_digit())
                && index + 1 < chars.len()
                && chars[index + 1] != '\n'
            {
                repaired.push(' ');
            }
            index += 1;
        }
        out = repaired;
        capitalize_next = true;
    }

    while out.ends_with(' ') || out.ends_with('\n') {
        out.pop();
    }

    if trailing_space {
        out.push(' ');
    }
    out
}

/// Whether a cleaned transcript contains something worth inserting.
///
/// The model can emit only punctuation for silence, noise, or a clipped breath.
/// Pasting a lone `.` is worse than treating the capture as empty, so every
/// shell uses this as the final gate before insertion.
pub fn is_meaningful(input: &str) -> bool {
    input.chars().any(char::is_alphanumeric)
}

/// A single taught correction: what the model tends to hear, and what should be
/// written instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correction {
    pub heard: String,
    pub written: String,
}

impl Correction {
    pub fn new(heard: impl Into<String>, written: impl Into<String>) -> Self {
        Self {
            heard: heard.into(),
            written: written.into(),
        }
    }
}

/// Replace taught phrases using longest-match-first, word-boundary matching.
///
/// Matching is ASCII case-insensitive. Punctuation at the edges of a phrase is
/// matched literally, so `"tirup dot in"` works while `"tirup"` does not fire
/// inside `"tirupin"`.
pub fn apply_corrections(input: &str, corrections: &[Correction]) -> String {
    if corrections.is_empty() || input.is_empty() {
        return input.to_string();
    }

    let mut ordered = corrections
        .iter()
        .filter(|correction| !correction.heard.is_empty())
        .collect::<Vec<_>>();
    ordered.sort_by_key(|left| std::cmp::Reverse(left.heard.len()));

    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    while cursor < input.len() {
        let rest = &input[cursor..];
        let mut matched = None;

        for correction in &ordered {
            if starts_with_ignore_ascii_case(rest, &correction.heard)
                && boundary_before(input, cursor)
                && boundary_after(input, cursor + correction.heard.len())
            {
                matched = Some(correction);
                break;
            }
        }

        if let Some(correction) = matched {
            out.push_str(&correction.written);
            cursor += correction.heard.len();
        } else {
            let ch = rest.chars().next().expect("cursor is on a char boundary");
            out.push(ch);
            cursor += ch.len_utf8();
        }
    }
    out
}

/// Remove standalone English filler words while keeping sentence punctuation.
pub fn remove_fillers(input: &str) -> String {
    const FILLERS: &[&str] = &["um", "umm", "uh", "uhh", "erm", "er", "ah", "hmm"];

    let mut out = String::with_capacity(input.len());
    for token in input.split_inclusive(char::is_whitespace) {
        let word = token.trim_end_matches(char::is_whitespace);
        let core = word.trim_matches(|ch: char| !ch.is_alphanumeric());
        if FILLERS
            .iter()
            .any(|filler| core.eq_ignore_ascii_case(filler))
        {
            // Keep a paragraph break that was attached to the filler, but drop
            // the word and its punctuation.
            for ch in token[word.len()..].chars() {
                if ch == '\n' || ch == '\r' {
                    out.push(ch);
                }
            }
            continue;
        }
        out.push_str(token);
    }
    normalize_spaces(&out)
}

/// Collapse horizontal whitespace to one space, trim spaces at the edges and
/// around newlines, and leave paragraph breaks intact.
fn normalize_spaces(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pending_space = false;
    for ch in input.chars() {
        match ch {
            ' ' | '\t' => pending_space = true,
            '\n' | '\r' => {
                pending_space = false;
                out.push(ch);
            }
            _ => {
                if pending_space && !out.is_empty() && !out.ends_with('\n') && !out.ends_with('\r')
                {
                    out.push(' ');
                }
                pending_space = false;
                out.push(ch);
            }
        }
    }
    out
}

/// Remove a final run of sentence punctuation. Closing quotes and brackets are
/// left alone: `Hello."` stays `Hello."` rather than losing the quote.
pub fn no_trailing_punctuation(input: &str) -> String {
    let mut out = input.trim_end().to_string();
    while let Some(last) = out.chars().last() {
        if matches!(last, '.' | '!' | '?' | '…' | '。' | '！' | '？') {
            out.pop();
        } else {
            break;
        }
    }
    out.trim_end().to_string()
}

fn starts_with_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    haystack
        .get(..needle.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(needle))
}

fn boundary_before(input: &str, index: usize) -> bool {
    input[..index]
        .chars()
        .next_back()
        .is_none_or(|ch| !is_word_char(ch))
}

fn boundary_after(input: &str, index: usize) -> bool {
    input[index..]
        .chars()
        .next()
        .is_none_or(|ch| !is_word_char(ch))
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_spacing_and_capitalization() {
        assert_eq!(clean_text("hello,world", true), "Hello, world ");
        assert_eq!(clean_text("hi , there .", true), "Hi, there. ");
        assert_eq!(clean_text("  spaced   out  ", false), "Spaced out");
        assert_eq!(clean_text("", true), "");
        assert_eq!(clean_text("   ", true), "");
    }

    #[test]
    fn capitalizes_each_sentence() {
        assert_eq!(
            clean_text("hello. world! again?", false),
            "Hello. World! Again?"
        );
    }

    #[test]
    fn preserves_paragraph_breaks() {
        assert_eq!(clean_text("hello\n\nworld", false), "Hello\n\nWorld");
        assert_eq!(clean_text("hello\nworld", false), "Hello\nWorld");
    }

    #[test]
    fn does_not_space_decimals() {
        assert_eq!(
            clean_text("version 1,000.50 is ready", false),
            "Version 1,000.50 is ready"
        );
    }

    #[test]
    fn applies_longest_correction_first() {
        let corrections = vec![
            Correction::new("tirup", "Tirup"),
            Correction::new("tirup mehta", "Tirup Mehta"),
        ];
        assert_eq!(
            apply_corrections("hello tirup mehta and tirup", &corrections),
            "hello Tirup Mehta and Tirup"
        );
    }

    #[test]
    fn corrections_respect_word_boundaries() {
        let corrections = vec![Correction::new("cat", "dog")];
        assert_eq!(
            apply_corrections("the cat concatenates cat.", &corrections),
            "the dog concatenates dog."
        );
    }

    #[test]
    fn corrections_are_case_insensitive() {
        let corrections = vec![Correction::new(
            "model registry tirup in",
            "modelregistry.tirup.in",
        )];
        assert_eq!(
            apply_corrections("go to Model Registry Tirup In now", &corrections),
            "go to modelregistry.tirup.in now"
        );
    }

    #[test]
    fn removes_fillers_but_keeps_punctuation() {
        assert_eq!(remove_fillers("um hello uh world."), "hello world.");
        assert_eq!(remove_fillers("hello, um, world"), "hello, world");
        assert_eq!(remove_fillers("hello um\nworld"), "hello\nworld");
        assert_eq!(remove_fillers("um\n\nhello"), "\n\nhello");
        assert_eq!(
            remove_fillers("humming is not a filler"),
            "humming is not a filler"
        );
    }

    #[test]
    fn removes_only_trailing_sentence_punctuation() {
        assert_eq!(no_trailing_punctuation("hello there."), "hello there");
        assert_eq!(no_trailing_punctuation("really?!"), "really");
        assert_eq!(
            no_trailing_punctuation("hello. keep this."),
            "hello. keep this"
        );
        assert_eq!(no_trailing_punctuation("hello.\""), "hello.\"");
        assert_eq!(no_trailing_punctuation("no punctuation"), "no punctuation");
    }

    #[test]
    fn silence_punctuation_is_not_meaningful() {
        assert!(!is_meaningful(""));
        assert!(!is_meaningful("."));
        assert!(!is_meaningful(" ... "));
        assert!(!is_meaningful("?!"));
        assert!(is_meaningful("Hi."));
        assert!(is_meaningful("42"));
        assert!(is_meaningful("Привет"));
    }

    #[test]
    fn pipeline_end_to_end() {
        let corrections = vec![Correction::new("tirup", "Tirup")];
        let text = remove_fillers("um hello tirup. this is it.");
        let text = apply_corrections(&text, &corrections);
        let text = clean_text(&text, false);
        assert_eq!(text, "Hello Tirup. This is it.");
    }
}
