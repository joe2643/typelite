//! Plan `dictionary-to-speech`: the personal dictionary, given to the speech recognizer.
//!
//! Polish has always seen the dictionary, but by then a name the recognizer misheard ("Senate"
//! for "Sennett", "or type" for "OraType") is already wrong and polish can only guess. Each
//! engine can be told which words to expect:
//!
//! - an OpenAI-compatible transcription upload sends them as the `prompt` field. Whisper servers
//!   read it as earlier text; `llama-server` (Qwen3-ASR) uses it as the instruction before the
//!   audio, and Qwen3-ASR then spells those words as given;
//! - built-in whisper.cpp gets them as its initial prompt.
//!
//! The words go as a plain comma-separated list, capped so a long dictionary neither slows every
//! run nor pushes Whisper's prompt past what it reads (224 tokens).

/// At most this many words are sent.
pub const MAX_WORDS: usize = 40;
/// And at most this many characters in total.
pub const MAX_CHARS: usize = 400;
/// A dictionary entry longer than this is a phrase or a note, not a word to listen for.
const MAX_WORD_CHARS: usize = 40;

/// The words as the recognizer gets them, "OraType, Sennett, Tauri", or `None` when there is
/// nothing to send. Blank and over-long entries are skipped, repeats (any case) are sent once,
/// and the list stops at [`MAX_WORDS`] words or [`MAX_CHARS`] characters, in dictionary order.
pub fn speech_prompt(words: &[String]) -> Option<String> {
    let mut kept: Vec<String> = Vec::new();
    let mut chars = 0;
    for word in words {
        let word = word.split_whitespace().collect::<Vec<_>>().join(" ");
        let length = word.chars().count();
        if word.is_empty() || length > MAX_WORD_CHARS || word.chars().any(char::is_control) {
            continue;
        }
        let lower = word.to_lowercase();
        if kept.iter().any(|seen| seen.to_lowercase() == lower) {
            continue;
        }
        let added = if kept.is_empty() { length } else { length + 2 };
        if kept.len() == MAX_WORDS || chars + added > MAX_CHARS {
            break;
        }
        chars += added;
        kept.push(word);
    }
    (!kept.is_empty()).then(|| kept.join(", "))
}

/// Minimum normalised characters for a punctuation-free contiguous prompt-prefix match.
const MIN_CONTIGUOUS_PREFIX_ECHO_CHARS: usize = 12;

/// True when a transcript is the word list read back rather than speech: two or more items,
/// split at commas, each one of the dictionary's words. Whisper can repeat its prompt when the
/// audio has little to say; a person rarely dictates only a list of their dictionary words.
pub fn is_prompt_echo(text: &str, words: &[String]) -> bool {
    let known: Vec<String> = words
        .iter()
        .map(|w| super::hallucination::normalise(w))
        .filter(|w| !w.is_empty())
        .collect();
    let items: Vec<String> = text
        .split([',', '，', '、'])
        .map(super::hallucination::normalise)
        .filter(|item| !item.is_empty())
        .collect();
    items.len() >= 2 && items.iter().all(|item| known.contains(item))
}

/// Prefix-aware variant of [`is_prompt_echo`] for the built-in `llama-server` provider.
///
/// When `max_tokens` cuts off a prompt-echo response mid-word, the trailing item is only a prefix
/// of a dictionary entry (or the entire transcript is a truncated prefix of the formatted prompt),
/// so strict [`is_prompt_echo`] misses it. High specificity is preserved by keeping strict
/// single-word/short-address guards intact so genuine Cantonese dictation is never dropped.
pub fn is_prompt_echo_prefix_aware(text: &str, words: &[String]) -> bool {
    if is_prompt_echo(text, words) {
        return true;
    }
    let Some(prompt) = speech_prompt(words) else {
        return false;
    };
    let known: Vec<String> = words
        .iter()
        .map(|w| super::hallucination::normalise(w))
        .filter(|w| !w.is_empty())
        .collect();
    let sent_words: Vec<String> = prompt
        .split([',', '，', '、'])
        .map(super::hallucination::normalise)
        .filter(|w| !w.is_empty())
        .collect();
    if sent_words.len() < 2 {
        return false;
    }

    let items: Vec<String> = text
        .split([',', '，', '、'])
        .map(super::hallucination::normalise)
        .filter(|item| !item.is_empty())
        .collect();
    if let Some((last, prefix_items)) = items.split_last() {
        if prefix_items.len() >= 2
            && prefix_items.iter().all(|item| known.contains(item))
            && known.iter().any(|w| w.starts_with(last))
        {
            return true;
        }
        if prefix_items.len() == 1
            && last.chars().count() >= 2
            && sent_words
                .windows(2)
                .any(|pair| pair[0] == prefix_items[0] && pair[1].starts_with(last))
        {
            return true;
        }
    }

    if sent_words.len() >= 3 {
        let norm_text = super::hallucination::normalise(text);
        let norm_prompt: String = sent_words.concat();
        if !norm_text.is_empty() && norm_prompt.starts_with(&norm_text) {
            let text_chars = norm_text.chars().count();
            let first_two_chars = sent_words[0].chars().count() + sent_words[1].chars().count();
            if text_chars >= MIN_CONTIGUOUS_PREFIX_ECHO_CHARS && text_chars > first_two_chars {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn joins_the_words_with_commas() {
        assert_eq!(
            speech_prompt(&words(&["OraType", "Sennett", "鰂魚涌"])).as_deref(),
            Some("OraType, Sennett, 鰂魚涌")
        );
        assert_eq!(speech_prompt(&[]), None);
        assert_eq!(speech_prompt(&words(&["  ", ""])), None);
    }

    #[test]
    fn cleans_and_skips_entries() {
        let long = "a".repeat(MAX_WORD_CHARS + 1);
        assert_eq!(
            speech_prompt(&words(&[
                " Ora   Type ",
                "oratype",
                "ORA TYPE",
                &long,
                "a\u{7}b",
                "Tauri"
            ]))
            .as_deref(),
            Some("Ora Type, oratype, Tauri")
        );
    }

    #[test]
    fn stops_at_the_word_and_character_caps() {
        let many: Vec<String> = (0..100).map(|i| format!("w{i}")).collect();
        let prompt = speech_prompt(&many).unwrap();
        assert_eq!(prompt.split(", ").count(), MAX_WORDS);
        assert!(prompt.starts_with("w0, w1, "));

        let wide: Vec<String> = (0..30).map(|i| format!("{i:0>30}")).collect();
        let prompt = speech_prompt(&wide).unwrap();
        assert!(prompt.chars().count() <= MAX_CHARS);
        assert_eq!(prompt.split(", ").count(), 12);
    }

    #[test]
    fn a_list_of_dictionary_words_is_an_echo() {
        let dictionary = words(&["OraType", "Sennett", "Tauri"]);
        assert!(is_prompt_echo("OraType, Sennett, Tauri.", &dictionary));
        assert!(is_prompt_echo("Sennett，Tauri", &dictionary));
        assert!(!is_prompt_echo("OraType", &dictionary));
        assert!(!is_prompt_echo("Ask Sennett about OraType.", &dictionary));
        assert!(!is_prompt_echo("OraType, and the rest", &dictionary));
        assert!(!is_prompt_echo("OraType, Sennett", &[]));
    }

    #[test]
    fn prefix_aware_echo_catches_truncated_prompt_tails_without_false_positives() {
        let dictionary = words(&[
            "OraType",
            "Sennett",
            "鰂魚涌",
            "太古坊",
            "Hong Kong Observatory",
            "港鐵",
            "我哋",
            "今日",
        ]);

        // Truncated comma-delimited prompt echoes are caught by prefix-aware check,
        // while strict `is_prompt_echo` stays false (preserving external behavior).
        for truncated in [
            "OraType, Sennett, 鰂魚",
            "OraType, Sennett, 鰂",
            "OraType, Senn",
            "鰂魚涌，太古",
            "OraType, Sennett, Hong Kong Obser",
            "OraType, Sennett, OraType, Senn",
            "OraType, Sennett, 鰂魚涌, 太古坊, 港",
            "OraType Sennett 鰂魚涌 太古",
        ] {
            assert!(
                !is_prompt_echo(truncated, &dictionary),
                "strict check must not match truncated {truncated:?}"
            );
            assert!(
                is_prompt_echo_prefix_aware(truncated, &dictionary),
                "prefix-aware check must catch {truncated:?}"
            );
        }

        // Real short Cantonese dictation & hotword-bearing speech must never be dropped.
        let hk_dict = words(&[
            "香港",
            "今日",
            "OraType",
            "Sennett",
            "鰂魚涌",
            "太古坊",
            "港鐵",
            "中環站",
            "我哋",
        ]);
        for genuine in [
            "OraType",
            "Ora",
            "OraType，我",
            "香港，今",
            "鰂魚涌太古坊",
            "港鐵中環站",
            "我陣間用 OraType 寫完份報告就去鰂魚涌食飯。",
            "OraType, Sennett，聽日見",
            "港鐵、八達通都好方便",
            "去鰂魚涌，太古坊嗰邊開會",
        ] {
            assert!(
                !is_prompt_echo_prefix_aware(genuine, &hk_dict),
                "must not drop real speech {genuine:?}"
            );
        }

        // Single-word and empty dictionaries never trigger prefix-aware suppression.
        let single = words(&["OraType"]);
        assert!(!is_prompt_echo_prefix_aware("OraType", &single));
        assert!(!is_prompt_echo_prefix_aware("OraTy", &single));
        assert!(!is_prompt_echo_prefix_aware("OraType, Sennett", &[]));
    }
}
