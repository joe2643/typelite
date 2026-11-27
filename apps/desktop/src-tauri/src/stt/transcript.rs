//! Clean-up applied to every transcript, whichever speech engine produced it.

/// Default consecutive repeat threshold matching Qwen3-ASR (Apache-2.0) official
/// `detect_and_fix_repetitions` post-processing.
pub const QWEN3_ASR_REPETITION_THRESHOLD: usize = 20;

/// Maximum pattern length (in Unicode scalar values) considered when collapsing consecutive
/// repeats in Qwen3-ASR output.
const MAX_REPETITION_PATTERN_LEN: usize = 20;

/// Returns `true` when Qwen3-ASR's language header indicates no speech (`"None"`).
pub fn is_no_speech_language_tag(tag: &str) -> bool {
    tag.eq_ignore_ascii_case("none")
}

/// Collapses degenerate character and short-pattern repetition runs produced by Qwen3-ASR
/// decoding loops, matching the Qwen3-ASR (Apache-2.0) `detect_and_fix_repetitions` behavior:
///
/// 1. Any run of strictly more than `threshold` identical Unicode scalar values collapses to 1.
/// 2. Any contiguous repeat of at least `threshold` copies of a `2..=20` char pattern collapses
///    to 1 copy, continuing greedily from the end of the repeated span.
///
/// Indexing uses `Vec<char>` rather than UTF-8 byte offsets so multi-byte CJK text never panics.
pub fn collapse_repetitions(text: &str, threshold: usize) -> String {
    if text.is_empty() || threshold == 0 {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let after_chars = collapse_char_runs(&chars, threshold);
    let after_patterns = collapse_pattern_runs(&after_chars, threshold, MAX_REPETITION_PATTERN_LEN);
    after_patterns.into_iter().collect()
}

fn collapse_char_runs(chars: &[char], threshold: usize) -> Vec<char> {
    let mut out = Vec::with_capacity(chars.len());
    let mut i = 0usize;
    while i < chars.len() {
        let mut count = 1usize;
        while i + count < chars.len() && chars[i + count] == chars[i] {
            count += 1;
        }
        if count > threshold {
            out.push(chars[i]);
        } else {
            out.extend_from_slice(&chars[i..i + count]);
        }
        i += count;
    }
    out
}

fn collapse_pattern_runs(chars: &[char], threshold: usize, max_len: usize) -> Vec<char> {
    let n = chars.len();
    let Some(min_repeat_chars) = threshold.checked_mul(2) else {
        return chars.to_vec();
    };
    if n < min_repeat_chars {
        return chars.to_vec();
    }

    let mut out = Vec::with_capacity(n);
    let mut i = 0usize;
    while i <= n - min_repeat_chars {
        let mut matched = None;
        for k in 2..=max_len {
            let Some(min_end) = k
                .checked_mul(threshold)
                .and_then(|span| i.checked_add(span))
            else {
                break;
            };
            if min_end > n {
                break;
            }
            let pattern = &chars[i..i + k];
            if (1..threshold).all(|rep| &chars[i + rep * k..i + (rep + 1) * k] == pattern) {
                let mut end = min_end;
                while end + k <= n && &chars[end..end + k] == pattern {
                    end += k;
                }
                matched = Some((k, end));
                break;
            }
        }
        if let Some((k, end)) = matched {
            out.extend_from_slice(&chars[i..i + k]);
            i = end;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out.extend_from_slice(&chars[i..]);
    out
}

/// Whisper returns its transcript as segments separated by line breaks, which are pauses, not
/// paragraphs. Join them into one line so they are never pasted as random line breaks; the AI
/// polish step adds structure where it belongs.
pub fn normalize_transcript(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for piece in text.split_whitespace() {
        let joins_without_space = match (out.chars().last(), piece.chars().next()) {
            (Some(prev), Some(next)) => is_cjk(prev) && is_cjk(next),
            _ => true,
        };
        if !out.is_empty() && !joins_without_space {
            out.push(' ');
        }
        out.push_str(piece);
    }
    out
}

/// Plan `qwen3-asr-support`: Qwen3-ASR servers (llama.cpp's `llama-server`, vLLM) start every
/// transcript with the language they heard, as `language Cantonese<asr_text>…`. Returns the text
/// after the tag and the language name in it; text without the tag comes back unchanged.
pub fn split_language_tag(text: &str) -> (&str, Option<&str>) {
    const MARKER: &str = "<asr_text>";
    let trimmed = text.trim_start();
    let Some(rest) = trimmed.strip_prefix("language") else {
        return (text, None);
    };
    let Some(end) = rest.find(MARKER) else {
        return (text, None);
    };
    let name = rest[..end].trim();
    if name.is_empty() || name.len() > 40 || !name.chars().all(|c| c.is_ascii_alphabetic()) {
        return (text, None);
    }
    (&rest[end + MARKER.len()..], Some(name))
}

/// Chinese, Japanese and Korean characters and their punctuation, which are written without
/// spaces between them.
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3000..=0x303F     // CJK punctuation
        | 0x3040..=0x30FF   // Hiragana, Katakana
        | 0x3400..=0x4DBF   // CJK Extension A
        | 0x4E00..=0x9FFF   // CJK Unified Ideographs
        | 0xAC00..=0xD7AF   // Hangul
        | 0xF900..=0xFAFF   // CJK Compatibility Ideographs
        | 0xFF00..=0xFFEF) // Full-width forms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_segments_are_joined_into_one_line() {
        assert_eq!(
            normalize_transcript(" Um, so let's meet on Monday.\n No wait, Tuesday at 3pm.\n"),
            "Um, so let's meet on Monday. No wait, Tuesday at 3pm."
        );
        assert_eq!(
            normalize_transcript("我们明天\n下午三点开会。"),
            "我们明天下午三点开会。"
        );
        assert_eq!(
            normalize_transcript("开会 at 3pm\n好的"),
            "开会 at 3pm 好的"
        );
    }

    #[test]
    fn qwen3_asr_language_tag_is_split_off() {
        assert_eq!(
            split_language_tag("language Cantonese<asr_text>我哋听日开会"),
            ("我哋听日开会", Some("Cantonese"))
        );
        assert_eq!(
            split_language_tag(" language English<asr_text>Hello there."),
            ("Hello there.", Some("English"))
        );
        assert_eq!(
            split_language_tag("language None<asr_text>"),
            ("", Some("None"))
        );
    }

    #[test]
    fn text_without_a_language_tag_is_unchanged() {
        for text in [
            "Hello there.",
            "language is hard to learn",
            "language two words<asr_text> here",
            "The <asr_text> tag",
        ] {
            assert_eq!(split_language_tag(text), (text, None));
        }
    }

    #[test]
    fn no_speech_language_tag_is_matched_case_insensitively() {
        for tag in ["None", "none", "NONE"] {
            assert!(is_no_speech_language_tag(tag), "{tag:?}");
        }
        for tag in ["Cantonese", "English", "", "NoneYet"] {
            assert!(!is_no_speech_language_tag(tag), "{tag:?}");
        }
    }

    #[test]
    fn collapse_repetitions_respects_threshold_boundaries_and_edge_cases() {
        assert_eq!(collapse_repetitions("", QWEN3_ASR_REPETITION_THRESHOLD), "");
        assert_eq!(collapse_repetitions("aaaaabbbbb", 0), "aaaaabbbbb");

        let exact_19 = format!("我{}再見", "啊".repeat(19));
        let exact_20 = format!("我{}再見", "啊".repeat(20));
        let over_21 = format!("我{}再見", "啊".repeat(21));
        assert_eq!(
            collapse_repetitions(&exact_19, QWEN3_ASR_REPETITION_THRESHOLD),
            exact_19
        );
        assert_eq!(
            collapse_repetitions(&exact_20, QWEN3_ASR_REPETITION_THRESHOLD),
            exact_20
        );
        assert_eq!(
            collapse_repetitions(&over_21, QWEN3_ASR_REPETITION_THRESHOLD),
            "我啊再見"
        );

        let exact_20_with_suffix = format!("我{}{}", "啊".repeat(20), "後面仲有一句。".repeat(5));
        assert_eq!(
            collapse_repetitions(&exact_20_with_suffix, QWEN3_ASR_REPETITION_THRESHOLD),
            exact_20_with_suffix
        );

        let pat_19 = format!("{}三", "一二".repeat(19));
        let pat_20 = "一二".repeat(20);
        let pat_21 = "一二".repeat(21);
        assert_eq!(
            collapse_repetitions(&pat_19, QWEN3_ASR_REPETITION_THRESHOLD),
            pat_19
        );
        assert_eq!(
            collapse_repetitions(&pat_20, QWEN3_ASR_REPETITION_THRESHOLD),
            "一二"
        );
        assert_eq!(
            collapse_repetitions(&pat_21, QWEN3_ASR_REPETITION_THRESHOLD),
            "一二"
        );

        let sequential = format!(
            "前綴{}之後又{}結束",
            "得啦得啦".repeat(25),
            "好呀".repeat(22)
        );
        assert_eq!(
            collapse_repetitions(&sequential, QWEN3_ASR_REPETITION_THRESHOLD),
            "前綴得啦之後又好呀結束"
        );
    }

    #[test]
    fn collapse_repetitions_recovers_four_observed_loop_structures() {
        assert_eq!(
            collapse_repetitions(&"啊！".repeat(126), QWEN3_ASR_REPETITION_THRESHOLD),
            "啊！"
        );
        assert_eq!(
            collapse_repetitions(&"係呀！".repeat(84), QWEN3_ASR_REPETITION_THRESHOLD),
            "係呀！"
        );

        let spaced_pair_tail = format!(
            "諗下先啦。 佢喺度搣嗰啲鬚呢，其實開始喺度縮嘅。 {}絲毫",
            "絲毫絲毫。 ".repeat(31)
        );
        assert_eq!(
            collapse_repetitions(&spaced_pair_tail, QWEN3_ASR_REPETITION_THRESHOLD),
            "諗下先啦。 佢喺度搣嗰啲鬚呢，其實開始喺度縮嘅。 絲毫絲毫。 絲毫"
        );

        let phrase_tail = format!("最後尾嗰首歌唔知乜嘢內容{}", " 唔好講".repeat(41));
        assert_eq!(
            collapse_repetitions(&phrase_tail, QWEN3_ASR_REPETITION_THRESHOLD),
            "最後尾嗰首歌唔知乜嘢內容 唔好講"
        );

        let code_switch_tail = format!(
            "加油啦！擘開隻眼。腫腫咗呀？Streaming spectator，哎呀幾好睇喎講真。冇啦冇啦冇啦，心態已崩。{}哎",
            "哎呀！".repeat(68)
        );
        assert_eq!(
            collapse_repetitions(&code_switch_tail, QWEN3_ASR_REPETITION_THRESHOLD),
            "加油啦！擘開隻眼。腫腫咗呀？Streaming spectator，哎呀幾好睇喎講真。冇啦冇啦冇啦，心態已崩。哎呀！哎"
        );
    }

    #[test]
    fn collapse_repetitions_preserves_natural_cantonese_reduplication_emphasis_and_code_switching()
    {
        for text in [
            "慢慢行",
            "睇睇先",
            "好似係咁樣樣",
            "唔好唔好唔好",
            "係呀係呀係呀係呀係呀",
            "我哋聽日 three o'clock 喺會議室開會，記得帶齊 laptop 同埋 HDMI adapter 呀。",
        ] {
            assert_eq!(
                collapse_repetitions(text, QWEN3_ASR_REPETITION_THRESHOLD),
                text,
                "{text:?}"
            );
        }
    }
}
