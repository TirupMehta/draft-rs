//! Deterministic formatting for a finished transcript.
//!
//! Two rules live here:
//!
//! 1. **Voice commands.** An utterance that is exactly a command, surrounded by
//!    real silence, becomes the structure it names. The command words are the
//!    only words that disappear.
//! 2. **Adaptive paragraphs.** The text breaks where the speaker paused,
//!    measured against that speaker's own rhythm rather than a stopwatch.
//!
//! Both rules work on byte ranges of the transcript, never on rebuilt strings,
//! so a formatting pass cannot silently reorder or respell a word. The invariant
//! is tested: with commands off, deleting every whitespace character from the
//! input and the output must produce the same string. `scratch that` is the one
//! deliberate exception, because the speaker asked for words to be removed.
//!
//! Every rule needs the decoder's segment timings. When the engine reports no
//! rows, or when the rows cannot be matched back to the transcript text, the
//! function returns the text untouched. Guessing is worse than doing nothing.

use crate::rhythm::Rhythm;

/// A paragraph break needs at least this many words behind it. Below that a
/// "paragraph" is a fragment, and fragments are how a helpful feature turns
/// into a broken one.
const MIN_PARAGRAPH_WORDS: usize = 12;

/// A break needs this many words in front of it too, so the last line of a
/// dictation never becomes a two-word orphan.
const MIN_TRAILING_WORDS: usize = 4;

/// Past this many words, a paragraph is hard to read whatever the speaker's
/// rhythm is, so the rule accepts a much smaller pause than usual.
const LONG_PARAGRAPH_WORDS: usize = 110;

/// The smallest pause that can be used as a relief break. Under this the
/// decoder is splitting rows for its own reasons, not because anyone paused.
const RELIEF_GAP_MS: i64 = 150;

/// A command has to be its own utterance: this much silence on each side.
const COMMAND_GAP_MS: i64 = 300;

/// One timed row of a finished transcript, in milliseconds from the start of
/// the clip.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Segment {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

impl Segment {
    pub fn new(text: impl Into<String>, start_ms: i64, end_ms: i64) -> Self {
        Self {
            text: text.into(),
            start_ms,
            end_ms,
        }
    }

    /// Silence between this row and the one before it. A negative value means
    /// the decoder overlapped its own rows, which describes no pause at all.
    fn gap_before(&self, previous: &Segment) -> i64 {
        (self.start_ms - previous.end_ms).max(0)
    }
}

/// How eagerly the paragraph rule breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParagraphStyle {
    /// Only clear topic changes.
    Conservative,
    /// The shipping default.
    #[default]
    Balanced,
    /// More structure, at the cost of the occasional break mid-thought.
    Aggressive,
}

impl ParagraphStyle {
    pub fn from_name(name: &str) -> Self {
        match name {
            "conservative" => Self::Conservative,
            "aggressive" => Self::Aggressive,
            _ => Self::Balanced,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Conservative => "conservative",
            Self::Balanced => "balanced",
            Self::Aggressive => "aggressive",
        }
    }

    /// Multiplier on the speaker's median pause, in tenths.
    fn multiplier_tenths(self) -> i64 {
        match self {
            Self::Conservative => 24,
            Self::Balanced => 19,
            Self::Aggressive => 14,
        }
    }

    /// The absolute floor, used before anything has been measured.
    fn floor_ms(self) -> i64 {
        match self {
            Self::Conservative => 700,
            Self::Balanced => 450,
            Self::Aggressive => 350,
        }
    }
}

/// Everything the formatter is allowed to do on this job.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub paragraphs: bool,
    pub style: ParagraphStyle,
    pub commands: bool,
    /// The language the engine detected, if it reported one. Commands are
    /// English only, so any other language turns them off for this job.
    pub language: Option<String>,
}

/// The result of one formatting pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Formatted {
    pub text: String,
    /// Pause statistics measured in this clip, when it had enough boundaries to
    /// measure. The caller decides whether to store them.
    pub observed: Option<Rhythm>,
    /// Paragraph breaks inserted, for the log line.
    pub breaks: usize,
    /// Commands recognised, for the log line.
    pub commands: usize,
}

impl Formatted {
    fn unchanged(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            ..Self::default()
        }
    }
}

/// Format a finished transcript.
///
/// `text` must already be cleaned, because the rules rely on its sentence
/// punctuation and on single spaces between words.
pub fn format(text: &str, segments: &[Segment], options: &Options, prior: &Rhythm) -> Formatted {
    if text.trim().is_empty() || segments.is_empty() {
        return Formatted::unchanged(text);
    }

    let Some(offsets) = align(text, segments) else {
        tracing::debug!("segment rows do not line up with the transcript; leaving it alone");
        return Formatted::unchanged(text);
    };

    let gaps = gaps(segments);
    let observed = measure(&gaps);
    let threshold = threshold(options.style, observed.as_ref(), prior);
    let actions = actions(segments, options);

    let mut auto_break = vec![false; segments.len()];
    if options.paragraphs {
        auto_break = paragraph_breaks(text, segments, &offsets, &gaps, threshold);
    }

    let mut pieces: Vec<(String, Separator)> = Vec::with_capacity(segments.len());
    let mut pending: Option<Separator> = None;
    let mut commands = 0;

    for index in 0..segments.len() {
        match actions[index] {
            Action::None => {}
            Action::Paragraph => {
                pending = Some(Separator::Paragraph);
                commands += 1;
                continue;
            }
            Action::Line => {
                pending = Some(Separator::Line);
                commands += 1;
                continue;
            }
            Action::Scratch => {
                commands += 1;
                scratch_last_sentence(&mut pieces);
                pending = Some(Separator::Space);
                continue;
            }
        }

        if pieces.is_empty() {
            // A command before the first word has nothing to separate.
            pending = None;
        } else {
            let separator = pending.take().unwrap_or(if auto_break[index] {
                Separator::Paragraph
            } else {
                Separator::Space
            });
            if let Some(last) = pieces.last_mut() {
                last.1 = separator;
            }
        }

        let end = offsets.get(index + 1).copied().unwrap_or(text.len());
        let piece = text[offsets[index]..end].trim();
        if piece.is_empty() {
            continue;
        }
        pieces.push((piece.to_owned(), Separator::Space));
    }

    if pieces.is_empty() {
        return Formatted {
            text: String::new(),
            observed,
            breaks: 0,
            commands,
        };
    }

    let mut out = String::with_capacity(text.len() + pieces.len() * 2);
    // A leading punctuation-only token (a dash, an opening quote on its own)
    // sits before the first matched word. Keep it rather than losing it.
    let lead = text[..offsets[0]].trim();
    if !lead.is_empty() && !has_alphanumeric(lead) {
        out.push_str(lead);
        out.push(' ');
    }
    let mut breaks = 0;
    for (index, (piece, _)) in pieces.iter().enumerate() {
        if index > 0 {
            match pieces[index - 1].1 {
                Separator::Space => out.push(' '),
                Separator::Line => out.push('\n'),
                Separator::Paragraph => {
                    out.push_str("\n\n");
                    breaks += 1;
                }
            }
        }
        out.push_str(piece);
    }

    Formatted {
        text: out,
        observed,
        breaks,
        commands,
    }
}

/// What a recognised command does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    None,
    Paragraph,
    Line,
    Scratch,
}

/// What sits between two pieces of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Separator {
    Space,
    Line,
    Paragraph,
}

fn actions(segments: &[Segment], options: &Options) -> Vec<Action> {
    let mut actions = vec![Action::None; segments.len()];
    if !options.commands || !commands_allowed(options.language.as_deref()) {
        return actions;
    }

    for (index, segment) in segments.iter().enumerate() {
        let Some(action) = command_for(&segment.text) else {
            continue;
        };
        if !is_isolated(segments, index) {
            continue;
        }
        actions[index] = action;
    }
    actions
}

/// Commands are English only. A detected language that is not English turns
/// them off, so a Hindi or Spanish dictation is never restructured by a phrase
/// that happens to sound like English.
fn commands_allowed(language: Option<&str>) -> bool {
    language.is_none_or(|code| {
        code.get(..2)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("en"))
    })
}

/// The exact phrases that act as commands, and nothing else.
///
/// "paragraph" on its own is deliberately absent: a single common noun is too
/// easy to say on purpose, and the whole safety of this feature is that a
/// command has to be a phrase nobody says by accident.
fn command_for(text: &str) -> Option<Action> {
    let phrase = normalize_command(text);
    if phrase.split(' ').count() > 4 {
        return None;
    }
    match phrase.as_str() {
        "new paragraph" | "new paragraphs" | "next paragraph" => Some(Action::Paragraph),
        "new line" | "new lines" | "newline" => Some(Action::Line),
        "scratch that" | "scratch this" | "scratch the last" => Some(Action::Scratch),
        _ => None,
    }
}

/// Lowercase, punctuation removed, single spaces.
fn normalize_command(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.extend(ch.to_lowercase());
        } else {
            space = true;
        }
    }
    out
}

/// A command counts only when the decoder put real silence on both sides of it.
/// A phrase inside a sentence is prose, not an instruction.
fn is_isolated(segments: &[Segment], index: usize) -> bool {
    let before = match index.checked_sub(1) {
        Some(previous) => segments[index].gap_before(&segments[previous]),
        None => i64::MAX,
    };
    let after = segments
        .get(index + 1)
        .map_or(i64::MAX, |next| next.gap_before(&segments[index]));
    before >= COMMAND_GAP_MS && after >= COMMAND_GAP_MS
}

/// Remove the last sentence from the last piece, or the whole piece when it has
/// only one. Returns nothing when there is no piece left.
fn scratch_last_sentence(pieces: &mut Vec<(String, Separator)>) {
    let Some((piece, separator)) = pieces.pop() else {
        return;
    };
    if let Some(head) = drop_last_sentence(&piece) {
        pieces.push((head, separator));
    }
}

/// The text with its final sentence removed, when it has more than one.
fn drop_last_sentence(piece: &str) -> Option<String> {
    let trimmed = piece.trim_end();
    let mut cut = None;
    for (index, ch) in trimmed.char_indices() {
        if !matches!(ch, '.' | '!' | '?' | '…' | '。' | '！' | '？') {
            continue;
        }
        let rest = trimmed[index + ch.len_utf8()..].trim();
        if !rest.is_empty() {
            cut = Some(index + ch.len_utf8());
        }
    }
    let cut = cut?;
    let head = trimmed[..cut].trim_end();
    if head.is_empty() {
        None
    } else {
        Some(head.to_owned())
    }
}

/// The pause statistics of one clip, when it has enough boundaries to measure.
fn measure(gaps: &[i64]) -> Option<Rhythm> {
    if gaps.len() < 4 {
        return None;
    }
    let mut sorted = gaps.to_vec();
    sorted.sort_unstable();
    Some(Rhythm {
        median_gap_ms: sorted[sorted.len() / 2],
        p90_gap_ms: sorted[(sorted.len() * 9 / 10).min(sorted.len() - 1)],
        clips: 1,
    })
}

/// The pause that becomes a paragraph break for this speaker.
fn threshold(style: ParagraphStyle, observed: Option<&Rhythm>, prior: &Rhythm) -> i64 {
    let median = observed
        .map(|rhythm| rhythm.median_gap_ms)
        .filter(|median| *median > 0)
        .unwrap_or(prior.median_gap_ms);
    if median <= 0 {
        return style.floor_ms();
    }
    (median.saturating_mul(style.multiplier_tenths()) / 10).max(style.floor_ms())
}

fn gaps(segments: &[Segment]) -> Vec<i64> {
    segments
        .windows(2)
        .map(|pair| pair[1].gap_before(&pair[0]))
        .collect()
}

/// Decide which boundaries become paragraph breaks.
///
/// A boundary has to be a sentence end, must not be followed by a word that
/// continues the previous thought, and needs either the speaker's own idea of a
/// long pause or a paragraph that has grown too long to read.
fn paragraph_breaks(
    text: &str,
    segments: &[Segment],
    offsets: &[usize],
    gaps: &[i64],
    threshold: i64,
) -> Vec<bool> {
    let mut breaks = vec![false; segments.len()];
    let total_words = content_words(text);
    let mut consumed = 0;
    let mut words_since_break = 0;

    for index in 0..segments.len() {
        let end = offsets.get(index + 1).copied().unwrap_or(text.len());
        let words = content_words(&text[offsets[index]..end]);

        if index > 0 {
            let gap = gaps[index - 1];
            let before = &text[..offsets[index]];
            let next_word = first_content_word(&segments[index].text);
            let eligible = ends_sentence(before) && !continues_thought(next_word);
            let remaining = total_words.saturating_sub(consumed);

            let strong = gap >= threshold
                && words_since_break >= MIN_PARAGRAPH_WORDS
                && remaining >= MIN_TRAILING_WORDS;
            // A speaker who never leaves a long gap still deserves readable
            // structure. Past a hundred words, any real sentence pause counts.
            let relief = words_since_break >= LONG_PARAGRAPH_WORDS && gap >= RELIEF_GAP_MS;

            if eligible && (strong || relief) {
                breaks[index] = true;
                words_since_break = 0;
            }
        }

        consumed += words;
        words_since_break += words;
    }

    breaks
}

/// Whether the text ends a sentence, ignoring a closing quote or bracket.
fn ends_sentence(text: &str) -> bool {
    for ch in text.trim_end().chars().rev() {
        match ch {
            '"' | '\'' | ')' | ']' | '}' | '”' | '’' | '»' | '›' => continue,
            '.' | '!' | '?' | '…' | '。' | '！' | '？' => return true,
            _ => return false,
        }
    }
    false
}

/// Words that mean the sentence is still going. Breaking in front of one of
/// these is the single most common way an automatic paragraph rule looks
/// broken, because the pause before "and" is a breath, not a topic change.
const CONTINUATIONS: &[&str] = &[
    "and", "but", "so", "because", "which", "that", "then", "or", "nor", "yet", "plus", "also",
    "anyway", "though", "whereas",
];

fn continues_thought(word: &str) -> bool {
    CONTINUATIONS
        .iter()
        .any(|continuation| word.eq_ignore_ascii_case(continuation))
}

fn first_content_word(text: &str) -> &str {
    text.split_whitespace()
        .map(core_word)
        .find(|word| !word.is_empty())
        .unwrap_or("")
}

fn content_words(text: &str) -> usize {
    text.split_whitespace()
        .filter(|token| has_alphanumeric(token))
        .count()
}

fn has_alphanumeric(text: &str) -> bool {
    text.chars().any(char::is_alphanumeric)
}

/// The word without its surrounding punctuation, which is what two rows can be
/// compared on without caring how the model split quotes and commas.
fn core_word(token: &str) -> &str {
    token.trim_matches(|ch: char| !ch.is_alphanumeric())
}

/// Map every segment back to the byte offset where it starts in `text`.
///
/// The comparison is on content words only, so the model may attach a comma to
/// the end of one row and the start of the next. Any mismatch fails the whole
/// alignment: a rule that cannot find its own boundary must not guess one.
fn align(text: &str, segments: &[Segment]) -> Option<Vec<usize>> {
    let tokens = tokens(text);
    let mut offsets = Vec::with_capacity(segments.len());
    let mut cursor = 0;

    for segment in segments {
        let words = segment
            .text
            .split_whitespace()
            .map(core_word)
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        if words.is_empty() {
            return None;
        }

        while cursor < tokens.len() && core_word(tokens[cursor].1).is_empty() {
            cursor += 1;
        }
        let start = cursor;
        for word in words {
            while cursor < tokens.len() && core_word(tokens[cursor].1).is_empty() {
                cursor += 1;
            }
            let token = tokens.get(cursor)?;
            if !core_word(token.1).eq_ignore_ascii_case(word) {
                return None;
            }
            cursor += 1;
        }
        offsets.push(tokens[start].0);
    }

    Some(offsets)
}

/// Byte ranges of every whitespace-separated token.
fn tokens(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (index, ch) in text.char_indices() {
        match (ch.is_whitespace(), start) {
            (false, None) => start = Some(index),
            (true, Some(from)) => {
                out.push((from, &text[from..index]));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        out.push((from, &text[from..]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Options {
        Options {
            paragraphs: true,
            style: ParagraphStyle::Balanced,
            commands: true,
            language: Some("en".to_owned()),
        }
    }

    /// A transcript whose words are spread over rows with the given gaps.
    ///
    /// Each entry is `(row text, silence before the row)`, and every row lasts
    /// one second, so the timings are easy to read in a failing assertion.
    fn build(rows: &[(&str, i64)]) -> (String, Vec<Segment>) {
        let mut text = String::new();
        let mut segments = Vec::new();
        let mut clock = 0;
        for (row, gap) in rows {
            clock += gap;
            let start = clock;
            clock += 1_000;
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(row);
            segments.push(Segment::new(*row, start, clock));
        }
        (text, segments)
    }

    /// The invariant: formatting never changes a word.
    fn assert_words_preserved(input: &str, output: &str) {
        let squash = |text: &str| {
            text.chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>()
        };
        assert_eq!(
            squash(input),
            squash(output),
            "formatting changed the words\n  in:  {input}\n  out: {output}"
        );
    }

    #[test]
    fn a_short_clip_is_left_alone() {
        let (text, segments) = build(&[("Hello there.", 0)]);
        let formatted = format(&text, &segments, &options(), &Rhythm::default());
        assert_eq!(formatted.text, text);
        assert_eq!(formatted.breaks, 0);
    }

    #[test]
    fn no_segments_means_no_guessing() {
        let formatted = format("One. Two. Three.", &[], &options(), &Rhythm::default());
        assert_eq!(formatted.text, "One. Two. Three.");
        assert!(formatted.observed.is_none());
    }

    #[test]
    fn a_real_pause_becomes_a_paragraph() {
        let rows = [
            (
                "We started the morning with a long walk along the river.",
                0,
            ),
            ("The water was higher than it had been all spring.", 0),
            ("We drove into town for lunch at the old bakery.", 2_400),
            ("It was busy, but the bread was worth the wait.", 0),
        ];
        let (text, segments) = build(&rows);
        let formatted = format(&text, &segments, &options(), &Rhythm::default());
        assert_eq!(formatted.breaks, 1, "{}", formatted.text);
        assert_eq!(
            formatted.text,
            "We started the morning with a long walk along the river. \
             The water was higher than it had been all spring.\n\n\
             We drove into town for lunch at the old bakery. \
             It was busy, but the bread was worth the wait."
        );
        assert_words_preserved(&text, &formatted.text);
    }

    #[test]
    fn a_speaker_who_pauses_everywhere_is_not_shredded() {
        // Every row is separated by 1.2 s, which a fixed 800 ms rule would turn
        // into a paragraph per sentence. The clip's own median is 1.2 s, so the
        // threshold is about 2.3 s and only the one real gap breaks.
        let rows = [
            ("I wanted to talk about the release plan for next week.", 0),
            ("The build is already on the staging machine.", 1_200),
            (
                "The installer needs one more check on the signing step.",
                1_200,
            ),
            ("And then we can publish the notes to the site.", 1_200),
            ("After that I will start on the next milestone.", 3_400),
            ("It should take a couple of days of work.", 1_200),
        ];
        let (text, segments) = build(&rows);
        let formatted = format(&text, &segments, &options(), &Rhythm::default());
        assert_eq!(formatted.breaks, 1, "{}", formatted.text);
        assert_words_preserved(&text, &formatted.text);
    }

    #[test]
    fn never_breaks_before_a_continuation() {
        let rows = [
            (
                "The first version of the app shipped in the spring of that year.",
                0,
            ),
            (
                "and it was well received by the people who tried it early.",
                3_000,
            ),
        ];
        let (text, segments) = build(&rows);
        let formatted = format(&text, &segments, &options(), &Rhythm::default());
        assert_eq!(formatted.breaks, 0, "{}", formatted.text);
    }

    #[test]
    fn never_breaks_without_a_sentence_end() {
        let rows = [
            (
                "The first version of the app shipped in the spring of that year",
                0,
            ),
            (
                "which was earlier than anyone had planned for the team.",
                3_000,
            ),
        ];
        let (text, segments) = build(&rows);
        let formatted = format(&text, &segments, &options(), &Rhythm::default());
        assert_eq!(formatted.breaks, 0, "{}", formatted.text);
    }

    #[test]
    fn a_long_monotone_paragraph_still_gets_structure() {
        // Six rows, no long pauses at all: the median is 200 ms, so the normal
        // threshold never fires. Past 110 words the relief rule takes over.
        let sentence = "This is one more sentence in a very long stretch of speech \
                        that keeps going without any real pause in it at all.";
        let rows = [
            (sentence, 0),
            (sentence, 200),
            (sentence, 200),
            (sentence, 200),
            (sentence, 200),
            (sentence, 200),
        ];
        let (text, segments) = build(&rows);
        let formatted = format(&text, &segments, &options(), &Rhythm::default());
        assert!(formatted.breaks >= 1, "{}", formatted.text);
        assert_words_preserved(&text, &formatted.text);
    }

    #[test]
    fn paragraphs_off_leaves_the_text_alone() {
        let rows = [
            (
                "We started the morning with a long walk along the river.",
                0,
            ),
            (
                "Then we drove into town for lunch at the old bakery.",
                2_400,
            ),
        ];
        let (text, segments) = build(&rows);
        let mut off = options();
        off.paragraphs = false;
        let formatted = format(&text, &segments, &off, &Rhythm::default());
        assert_eq!(formatted.text, text);
    }

    #[test]
    fn a_spoken_new_paragraph_is_its_own_utterance() {
        let rows = [
            ("That covers the first part of the plan.", 0),
            ("New paragraph.", 1_400),
            ("Now I want to talk about the second part.", 1_400),
        ];
        let (text, segments) = build(&rows);
        let formatted = format(&text, &segments, &options(), &Rhythm::default());
        assert_eq!(formatted.commands, 1);
        assert_eq!(
            formatted.text,
            "That covers the first part of the plan.\n\n\
             Now I want to talk about the second part."
        );
    }

    #[test]
    fn the_same_phrase_inside_a_sentence_is_prose() {
        let rows = [
            ("I wrote a new paragraph yesterday and deleted it again.", 0),
            ("That is the whole story of the afternoon.", 200),
        ];
        let (text, segments) = build(&rows);
        let formatted = format(&text, &segments, &options(), &Rhythm::default());
        assert_eq!(formatted.commands, 0);
        assert_words_preserved(&text, &formatted.text);
    }

    #[test]
    fn a_command_without_a_pause_around_it_is_ignored() {
        let rows = [
            ("That covers the first part of the plan.", 0),
            ("New paragraph.", 0),
            ("Now I want to talk about the second part.", 0),
        ];
        let (text, segments) = build(&rows);
        let formatted = format(&text, &segments, &options(), &Rhythm::default());
        assert_eq!(formatted.commands, 0);
        assert_words_preserved(&text, &formatted.text);
    }

    #[test]
    fn scratch_that_removes_the_last_sentence() {
        let rows = [
            (
                "We should meet on Tuesday at the office. Bring the printed report.",
                0,
            ),
            ("Scratch that.", 1_200),
        ];
        let (text, segments) = build(&rows);
        let formatted = format(&text, &segments, &options(), &Rhythm::default());
        assert_eq!(formatted.commands, 1);
        assert_eq!(formatted.text, "We should meet on Tuesday at the office.");
    }

    #[test]
    fn scratch_that_removes_a_whole_row_when_it_has_one_sentence() {
        let rows = [
            ("We should meet on Tuesday at the office.", 0),
            ("Actually the room is double booked.", 900),
            ("Scratch that.", 900),
        ];
        let (text, segments) = build(&rows);
        let formatted = format(&text, &segments, &options(), &Rhythm::default());
        assert_eq!(formatted.text, "We should meet on Tuesday at the office.");
    }

    #[test]
    fn scratch_that_on_an_empty_dictation_leaves_nothing() {
        let rows = [("Scratch that.", 0)];
        let (text, segments) = build(&rows);
        let formatted = format(&text, &segments, &options(), &Rhythm::default());
        assert_eq!(formatted.text, "");
    }

    #[test]
    fn commands_are_english_only() {
        let rows = [
            ("यह पहला हिस्सा है।", 0),
            ("New paragraph.", 1_400),
            ("यह दूसरा हिस्सा है।", 1_400),
        ];
        let (text, segments) = build(&rows);
        let mut hindi = options();
        hindi.language = Some("hi".to_owned());
        let formatted = format(&text, &segments, &hindi, &Rhythm::default());
        assert_eq!(formatted.commands, 0);
        assert_words_preserved(&text, &formatted.text);
    }

    #[test]
    fn commands_off_leaves_the_command_words_in_place() {
        let rows = [
            ("That covers the first part of the plan.", 0),
            ("New paragraph.", 1_400),
            ("Now I want to talk about the second part.", 1_400),
        ];
        let (text, segments) = build(&rows);
        let mut off = options();
        off.commands = false;
        let formatted = format(&text, &segments, &off, &Rhythm::default());
        assert_eq!(formatted.commands, 0);
        assert_words_preserved(&text, &formatted.text);
    }

    #[test]
    fn a_new_line_is_one_break_not_two() {
        let rows = [
            ("First item is the build.", 0),
            ("New line.", 1_000),
            ("Second item is the installer.", 1_000),
        ];
        let (text, segments) = build(&rows);
        let formatted = format(&text, &segments, &options(), &Rhythm::default());
        assert_eq!(
            formatted.text,
            "First item is the build.\nSecond item is the installer."
        );
    }

    #[test]
    fn a_leading_command_does_not_open_with_a_blank_line() {
        let rows = [
            ("New paragraph.", 0),
            ("The meeting moved to Thursday afternoon.", 1_000),
        ];
        let (text, segments) = build(&rows);
        let formatted = format(&text, &segments, &options(), &Rhythm::default());
        assert_eq!(formatted.text, "The meeting moved to Thursday afternoon.");
    }

    #[test]
    fn a_trailing_command_does_not_close_with_a_blank_line() {
        let rows = [
            ("The meeting moved to Thursday afternoon.", 0),
            ("New paragraph.", 1_000),
        ];
        let (text, segments) = build(&rows);
        let formatted = format(&text, &segments, &options(), &Rhythm::default());
        assert_eq!(formatted.text, "The meeting moved to Thursday afternoon.");
    }

    #[test]
    fn the_threshold_follows_the_style() {
        let prior = Rhythm {
            median_gap_ms: 1_000,
            p90_gap_ms: 2_000,
            clips: 3,
        };
        assert_eq!(threshold(ParagraphStyle::Conservative, None, &prior), 2_400);
        assert_eq!(threshold(ParagraphStyle::Balanced, None, &prior), 1_900);
        assert_eq!(threshold(ParagraphStyle::Aggressive, None, &prior), 1_400);
    }

    #[test]
    fn an_empty_profile_falls_back_to_the_floor() {
        let prior = Rhythm::default();
        assert_eq!(
            threshold(ParagraphStyle::Balanced, None, &prior),
            ParagraphStyle::Balanced.floor_ms()
        );
    }

    #[test]
    fn a_measured_clip_beats_the_stored_prior() {
        let prior = Rhythm {
            median_gap_ms: 300,
            p90_gap_ms: 900,
            clips: 5,
        };
        let observed = Rhythm {
            median_gap_ms: 1_200,
            p90_gap_ms: 2_000,
            clips: 1,
        };
        assert_eq!(
            threshold(ParagraphStyle::Balanced, Some(&observed), &prior),
            2_280
        );
    }

    #[test]
    fn short_clips_report_no_measurement() {
        assert!(measure(&[100, 200, 300]).is_none());
        let measured = measure(&[100, 200, 300, 900, 1_000]).unwrap();
        assert_eq!(measured.median_gap_ms, 300);
        assert_eq!(measured.p90_gap_ms, 1_000);
    }

    #[test]
    fn alignment_survives_punctuation_moved_between_rows() {
        let text = "Hello, world. This is fine.";
        let segments = vec![
            Segment::new("Hello world.", 0, 1_000),
            Segment::new("This is fine.", 1_200, 2_000),
        ];
        let offsets = align(text, &segments).unwrap();
        assert_eq!(offsets, vec![0, 14]);
    }

    #[test]
    fn alignment_refuses_rows_that_are_not_there() {
        let text = "Hello, world.";
        let segments = vec![
            Segment::new("Hello world.", 0, 1_000),
            Segment::new("Something else.", 1_200, 2_000),
        ];
        assert!(align(text, &segments).is_none());
    }

    #[test]
    fn alignment_keeps_a_leading_quote_with_its_word() {
        let text = "\"Hello there,\" he said.";
        let segments = vec![Segment::new("\"Hello there,\"", 0, 1_000)];
        assert_eq!(align(text, &segments).unwrap(), vec![0]);
    }

    #[test]
    fn every_word_survives_a_torture_clip() {
        let rows = [
            (
                "Um, so the thing is that we need to ship this by Friday.",
                0,
            ),
            ("(And the installer has to be signed before that.)", 1_900),
            (
                "\"Right,\" she said, \"but the model is not ready.\"",
                2_600,
            ),
            ("OK.", 1_500),
            ("We will revisit it on Monday at the latest.", 2_900),
        ];
        let (text, segments) = build(&rows);
        for style in [
            ParagraphStyle::Conservative,
            ParagraphStyle::Balanced,
            ParagraphStyle::Aggressive,
        ] {
            let mut options = options();
            options.style = style;
            let formatted = format(&text, &segments, &options, &Rhythm::default());
            assert_words_preserved(&text, &formatted.text);
        }
    }
}
