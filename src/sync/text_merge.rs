//! Lossless source reconciliation at the library boundary.
use std::ops::Range;
use std::{collections::HashMap, fmt::Write};

use gix_imara_diff::{Algorithm, Diff, InternedInput};
use gix_merge::blob::{
    Resolution,
    builtin_driver::text::{self, Conflict, Labels},
};
use unicode_segmentation::UnicodeSegmentation;

use crate::diagnostics::{self, DiagnosticEvent, MergeFallbackReason};

const MAX_INPUT_BYTES: usize = 1_048_576;
const MAX_TOKENS: usize = 20_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeDisposition {
    Clean,
    PreservedOverlap,
    BudgetFallback,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEdit {
    /// UTF-8 byte coordinates in the original local source; edits are ordered.
    pub range: Range<usize>,
    pub replacement: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextMergeResult {
    pub body: String,
    pub disposition: MergeDisposition,
    pub edits: Vec<SourceEdit>,
}

pub fn merge_text(base: &str, local: &str, remote: &str) -> TextMergeResult {
    if let Some(body) = unambiguous(base, local, remote) {
        return TextMergeResult {
            body: body.to_owned(),
            disposition: MergeDisposition::Clean,
            edits: if local.len() > MAX_INPUT_BYTES || body.len() > MAX_INPUT_BYTES {
                whole_edit(local, body)
            } else {
                source_edits(local, body).unwrap_or_else(|| whole_edit(local, body))
            },
        };
    }
    match merge_changed(base, local, remote) {
        Ok(result) => result,
        Err(reason) => fallback(local, remote, reason),
    }
}

pub(super) fn unambiguous<'a>(base: &str, local: &'a str, remote: &'a str) -> Option<&'a str> {
    if local == remote || base == remote {
        Some(local)
    } else if base == local {
        Some(remote)
    } else {
        None
    }
}

pub(super) fn whole_edit(local: &str, body: &str) -> Vec<SourceEdit> {
    if local == body {
        Vec::new()
    } else {
        vec![SourceEdit {
            range: 0..local.len(),
            replacement: body.to_owned(),
        }]
    }
}

pub(super) fn fallback(local: &str, remote: &str, reason: MergeFallbackReason) -> TextMergeResult {
    diagnostics::record(DiagnosticEvent::MergeFallback {
        id: uuid::Uuid::new_v4(),
        reason,
    });
    TextMergeResult {
        body: remote.to_owned(),
        disposition: MergeDisposition::BudgetFallback,
        edits: whole_edit(local, remote),
    }
}

fn admitted_tokens<'a>(tokens: impl Iterator<Item = &'a str>) -> bool {
    tokens.take(MAX_TOKENS + 1).count() <= MAX_TOKENS
}

fn merge_changed(
    base: &str,
    local: &str,
    remote: &str,
) -> Result<TextMergeResult, MergeFallbackReason> {
    let sources = [base, local, remote];
    if sources.iter().any(|source| source.len() > MAX_INPUT_BYTES) {
        return Err(MergeFallbackReason::InputLimit);
    }
    let limit = sources
        .iter()
        .try_fold(0usize, |size, source| size.checked_add(source.len()))
        .ok_or(MergeFallbackReason::OutputLimit)?;
    if !admitted_tokens(
        sources
            .iter()
            .flat_map(|source| source.split_inclusive('\n')),
    ) {
        return Err(MergeFallbackReason::TokenLimit);
    }
    let (raw, resolution) = run_engine(base, local, remote, limit)?;
    let (body, disposition) = if resolution == Resolution::CompleteWithAutoResolvedConflict {
        // Only line collisions need refinement; ordinary source is never interpreted as token IDs.
        if !admitted_tokens(sources.iter().flat_map(|source| source.split_word_bounds())) {
            return Err(MergeFallbackReason::TokenLimit);
        }
        let mut dictionary = Vec::new();
        let mut ids = HashMap::new();
        let base = encode(base, &mut ids, &mut dictionary)?;
        let local = encode(local, &mut ids, &mut dictionary)?;
        let remote = encode(remote, &mut ids, &mut dictionary)?;
        let encoded_limit = base
            .len()
            .checked_add(local.len())
            .and_then(|n| n.checked_add(remote.len()))
            .ok_or(MergeFallbackReason::OutputLimit)?;
        let (encoded, resolution) = run_engine(&base, &local, &remote, encoded_limit)?;
        let body = decode(&encoded, &dictionary, limit)?;
        let disposition = if resolution == Resolution::CompleteWithAutoResolvedConflict {
            MergeDisposition::PreservedOverlap
        } else {
            MergeDisposition::Clean
        };
        (body, disposition)
    } else {
        (raw, MergeDisposition::Clean)
    };
    let edits = source_edits(local, &body).ok_or(MergeFallbackReason::TokenLimit)?;
    Ok(TextMergeResult {
        body,
        disposition,
        edits,
    })
}

fn run_engine(
    base: &str,
    local: &str,
    remote: &str,
    limit: usize,
) -> Result<(String, Resolution), MergeFallbackReason> {
    let mut input = InternedInput::default();
    let merge = text::Merge::new(
        &mut input,
        local.as_bytes(),
        base.as_bytes(),
        remote.as_bytes(),
        Algorithm::Histogram,
    );
    let mut output = Vec::new();
    // ResolveWithTheirs emits source slices only, so the sum of inputs bounds engine output.
    let resolution = merge.run(&mut output, Labels::default(), Conflict::ResolveWithTheirs);
    if output.len() > limit {
        return Err(MergeFallbackReason::OutputLimit);
    }
    let output = String::from_utf8(output).map_err(|_| MergeFallbackReason::InvalidEncoding)?;
    Ok((output, resolution))
}

fn encode<'a>(
    source: &'a str,
    ids: &mut HashMap<&'a str, usize>,
    dictionary: &mut Vec<&'a str>,
) -> Result<String, MergeFallbackReason> {
    let mut encoded = String::new();
    for token in source.split_word_bounds() {
        let id = *ids.entry(token).or_insert_with(|| {
            let id = dictionary.len();
            dictionary.push(token);
            id
        });
        writeln!(encoded, "{id}").map_err(|_| MergeFallbackReason::InvalidEncoding)?;
    }
    Ok(encoded)
}

fn decode(encoded: &str, dictionary: &[&str], limit: usize) -> Result<String, MergeFallbackReason> {
    let mut decoded = String::new();
    for line in encoded.split_inclusive('\n') {
        let digits = line
            .strip_suffix('\n')
            .ok_or(MergeFallbackReason::InvalidEncoding)?;
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(MergeFallbackReason::InvalidEncoding);
        }
        let id = digits
            .parse::<usize>()
            .map_err(|_| MergeFallbackReason::InvalidEncoding)?;
        let token = dictionary
            .get(id)
            .ok_or(MergeFallbackReason::InvalidEncoding)?;
        if decoded
            .len()
            .checked_add(token.len())
            .is_none_or(|len| len > limit)
        {
            return Err(MergeFallbackReason::OutputLimit);
        }
        decoded.push_str(token);
    }
    Ok(decoded)
}

fn source_edits(local: &str, body: &str) -> Option<Vec<SourceEdit>> {
    if local == body {
        return Some(Vec::new());
    }
    if local.len() > MAX_INPUT_BYTES || body.len() > MAX_INPUT_BYTES * 3 {
        return None;
    }
    if admitted_tokens(local.split_word_bounds().chain(body.split_word_bounds())) {
        Some(diff_edits(
            local.split_word_bounds(),
            body.split_word_bounds(),
            body,
        ))
    } else if admitted_tokens(
        local
            .split_inclusive('\n')
            .chain(body.split_inclusive('\n')),
    ) {
        Some(diff_edits(
            local.split_inclusive('\n'),
            body.split_inclusive('\n'),
            body,
        ))
    } else {
        None
    }
}

fn diff_edits<'a>(
    before: impl Iterator<Item = &'a str>,
    after: impl Iterator<Item = &'a str>,
    body: &str,
) -> Vec<SourceEdit> {
    let mut before_offsets = vec![0];
    let mut after_offsets = vec![0];
    let mut input = InternedInput::default();
    input.update_before(before.inspect(|token| {
        before_offsets.push(before_offsets.last().copied().unwrap_or(0) + token.len())
    }));
    input.update_after(after.inspect(|token| {
        after_offsets.push(after_offsets.last().copied().unwrap_or(0) + token.len())
    }));
    Diff::compute(Algorithm::Histogram, &input)
        .hunks()
        .map(|hunk| SourceEdit {
            range: before_offsets[hunk.before.start as usize]
                ..before_offsets[hunk.before.end as usize],
            replacement: body
                [after_offsets[hunk.after.start as usize]..after_offsets[hunk.after.end as usize]]
                .to_owned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoder_checks_known_ids_complete_lines_and_output_budget() {
        let dictionary = ["🦊", "\r\n"];
        assert_eq!(decode("0\n1\n", &dictionary, 6), Ok("🦊\r\n".into()));
        assert_eq!(decode("", &dictionary, 0), Ok(String::new()));
        for invalid in [
            "2\n",
            "0",
            "\n",
            "-1\n",
            "+0\n",
            "0\r\n",
            "0\ntext\n",
            "999999999999999999999999999999999999\n",
        ] {
            assert_eq!(
                decode(invalid, &dictionary, 100),
                Err(MergeFallbackReason::InvalidEncoding)
            );
        }
        assert_eq!(
            decode("0\n1\n", &dictionary, 5),
            Err(MergeFallbackReason::OutputLimit)
        );
        assert_eq!(
            decode("0\n0\n", &dictionary, 6),
            Err(MergeFallbackReason::OutputLimit)
        );
    }
}
