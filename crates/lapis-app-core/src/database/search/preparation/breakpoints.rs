use super::whitespace;
use lapis_app_protocol::database::search::source::SearchSourceMetadata;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub(super) struct Point {
    pub position: usize,
    pub score: f64,
}

pub(super) fn points(text: &[u16], source: &SearchSourceMetadata, start: usize) -> Vec<Point> {
    let mut points = BTreeMap::<usize, f64>::new();
    let mut add = |position: usize, score: f64| {
        let value = points.entry(position).or_insert(score);
        if score > *value {
            *value = score;
        }
    };
    for section in &source.sections {
        let position = section.position.start.offset as usize;
        if position <= start {
            continue;
        }
        let score = match section.r#type.as_str() {
            "heading" => {
                let level = source
                    .headings
                    .iter()
                    .rev()
                    .find(|h| h.position.start.offset == section.position.start.offset)
                    .map_or(1., |h| h.level);
                (110. - level * 10.).max(50.)
            }
            "code" => 80.,
            "thematicBreak" => 60.,
            "list" => 5.,
            "blockquote" | "callout" => 10.,
            "paragraph" | "text" => 20.,
            _ => 1.,
        };
        add(position, score);
    }
    for (position, unit) in text.iter().enumerate().skip(start) {
        if *unit != 10 {
            continue;
        }
        add(position, 1.);
        let tail = &text[position + 1..];
        let hashes = tail.iter().take_while(|c| **c == 35).count();
        if (1..=6).contains(&hashes) {
            add(position, (110 - hashes * 10) as f64);
        }
        if tail.starts_with(&[96, 96, 96]) {
            add(position, 80.);
        }
    }
    // Each pass preserves RegExp's non-overlapping match order. Whitespace may
    // itself contain newlines, so one combined per-line scan is not equivalent.
    for (kind, score) in [(0, 60.), (1, 20.), (2, 5.), (3, 5.)] {
        let mut cursor = start;
        while cursor < text.len() {
            let Some(relative) = text[cursor..].iter().position(|c| *c == 10) else {
                break;
            };
            let position = cursor + relative;
            let tail = &text[position + 1..];
            let prefix = match kind {
                0 if tail.starts_with(&[45, 45, 45])
                    || tail.starts_with(&[42, 42, 42])
                    || tail.starts_with(&[95, 95, 95]) =>
                {
                    Some(3)
                }
                1 => Some(0),
                2 if matches!(tail.first(), Some(45 | 42)) => Some(1),
                3 => {
                    let digits = tail.iter().take_while(|c| (48..=57).contains(*c)).count();
                    (digits > 0 && tail.get(digits) == Some(&46)).then_some(digits + 1)
                }
                _ => None,
            };
            let end = prefix.and_then(|prefix| {
                let following = position + 1 + prefix;
                if kind < 2 {
                    let last_newline = text[following..]
                        .iter()
                        .take_while(|c| whitespace(**c))
                        .enumerate()
                        .filter(|(_, c)| **c == 10)
                        .map(|(i, _)| following + i)
                        .last();
                    last_newline.map(|i| i + 1)
                } else {
                    text.get(following)
                        .filter(|c| whitespace(**c))
                        .map(|_| following + 1)
                }
            });
            if let Some(end) = end {
                add(position, score);
                cursor = end;
            } else {
                cursor = position + 1;
            }
        }
    }
    points
        .into_iter()
        .map(|(position, score)| Point { position, score })
        .collect()
}

pub(super) fn fences(
    text: &[u16],
    source: &SearchSourceMetadata,
    start: usize,
) -> Vec<(usize, usize)> {
    let sections: Vec<_> = source
        .sections
        .iter()
        .filter(|s| s.r#type == "code")
        .map(|s| {
            (
                start.max(s.position.start.offset as usize),
                s.position.end.offset as usize,
            )
        })
        .collect();
    if !sections.is_empty() {
        return sections;
    }
    let mut markers = vec![];
    if text.starts_with(&[96, 96, 96]) {
        markers.push(0);
    }
    for (i, unit) in text.iter().enumerate() {
        if *unit == 10 && text[i + 1..].starts_with(&[96, 96, 96]) {
            markers.push(i);
        }
    }
    markers
        .chunks(2)
        .filter(|pair| pair[0] >= start)
        .map(|pair| (pair[0], *pair.get(1).unwrap_or(&text.len())))
        .collect()
}

pub(super) fn cutoff(
    points: &[Point],
    start: usize,
    target: usize,
    window: usize,
    decay: f64,
    fences: &[(usize, usize)],
) -> usize {
    let lower = (start + 1).max(target.saturating_sub(window));
    let mut best = target;
    let mut score = -1.;
    for point in points {
        if point.position < lower {
            continue;
        }
        if point.position > target {
            break;
        }
        if fences
            .iter()
            .any(|(s, e)| point.position > *s && point.position < *e)
        {
            continue;
        }
        let distance = (target - point.position) as f64 / window as f64;
        let weighted = point.score * (1. - distance * distance * decay);
        if weighted > score || (weighted == score && point.position > best) {
            best = point.position;
            score = weighted;
        }
    }
    best
}
