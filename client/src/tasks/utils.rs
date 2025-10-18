use std::mem::take;

/// Splits a string into exactly `n` chunks, treating quoted substrings as single tokens.
/// Optionally discards the first token, which is useful if the input string begins with a command.
///
/// # Args
/// * `n` - The expected number of resulting tokens.  
/// * `strs` - The input string slice to be tokenised.  
/// * `discard_first` - Whether the first discovered token should be discarded (`Chop`) or kept (`DontChop`).  
///
/// # Returns
/// Returns `Some(Vec<String>)` if exactly `n` tokens are produced after processing,  
/// otherwise returns `None`.
///
/// # Example
/// ```
/// let s = "a b  \"c d\" e".to_string();
/// assert_eq!(
///     split_string_slices_to_n(4, &s, DiscardFirst::DontChop),
///     Some(vec![
///         "a".to_string(),
///         "b".to_string(),
///         "c d".to_string(),
///         "e".to_string(),
///     ])
/// )
/// ```
pub fn split_string_slices_to_n(
    mut n: usize,
    strs: &str,
    mut discard_first: DiscardFirst,
) -> Option<Vec<String>> {
    // Account for chopping first 2 params
    let mut discarding_two = false;
    if discard_first == DiscardFirst::ChopTwo {
        discard_first = DiscardFirst::Chop;
        discarding_two = true;
    }

    // Flatten the slices
    let mut chunks: Vec<String> = Vec::new();
    let mut s = String::new();
    let mut toggle: bool = false;

    for (_, c) in strs.chars().enumerate() {
        if c == '"' {
            if toggle {
                toggle = false;
                if !s.is_empty() {
                    chunks.push(take(&mut s));
                }
                s.clear();
            } else {
                // Start of a quoted string
                toggle = true;
            }
        } else if c == ' ' && !toggle {
            if discard_first == DiscardFirst::Chop && chunks.is_empty() {
                discard_first = DiscardFirst::DontChop;
                s.clear();
            }

            if !s.is_empty() {
                chunks.push(take(&mut s));
            }
            s.clear();
        } else {
            s.push(c);
        }
    }

    // Handle the very last chunk which didn't get pushed by the loop
    if !s.is_empty() {
        chunks.push(s);
    }

    // Account for chopping first 2 params
    if discarding_two {
        chunks.remove(0);
    }

    println!("CHUNKS: {chunks:#?}");

    if chunks.len() != n {
        return None;
    }

    Some(chunks)
}

/// Determines whether the [`split_string_slices_to_n`] function should discard the first
/// found substring or not - this would be useful where the command is present in the input
/// string.
#[derive(PartialEq, Eq)]
pub enum DiscardFirst {
    Chop,
    ChopTwo,
    DontChop,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_with_no_quotes() {
        let s = "a b  c d e f    g    ".to_string();
        assert_eq!(
            split_string_slices_to_n(7, &s, DiscardFirst::DontChop),
            Some(vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
                "f".to_string(),
                "g".to_string()
            ])
        )
    }

    #[test]
    fn tokens_with_quotes_space() {
        let s = "a b  \"c  d\" e".to_string();
        assert_eq!(
            split_string_slices_to_n(4, &s, DiscardFirst::DontChop),
            Some(vec![
                "a".to_string(),
                "b".to_string(),
                "c  d".to_string(),
                "e".to_string(),
            ])
        )
    }

    #[test]
    fn tokens_with_quotes() {
        let s = "a b  \"c d\" e".to_string();
        assert_eq!(
            split_string_slices_to_n(4, &s, DiscardFirst::DontChop),
            Some(vec![
                "a".to_string(),
                "b".to_string(),
                "c d".to_string(),
                "e".to_string(),
            ])
        )
    }

    #[test]
    fn tokens_bad_count() {
        let s = "a b  \"c d\" e".to_string();
        assert_eq!(
            split_string_slices_to_n(5, &s, DiscardFirst::DontChop),
            None
        )
    }
}
