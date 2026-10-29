use super::text;

pub(super) struct Scope {
    pub text: String,
    pub offset: usize,
}
pub(super) fn scopes(kind: &str, input: &str) -> Vec<Scope> {
    let units: Vec<_> = input.encode_utf16().collect();
    let matches = |pattern: &str, flags: &str| {
        regress::Regex::with_flags(pattern, flags)
            .expect("static Search expression")
            .find_from_utf16(&units, 0)
            .map(|m| m.range)
            .collect::<Vec<_>>()
    };
    match kind {
        "line" => lines(input),
        "block" => {
            let ranges = matches(r"\S(?:.|\n)*?(?=\n\s*\n|$)", "");
            if ranges.is_empty() {
                vec![Scope {
                    text: input.into(),
                    offset: 0,
                }]
            } else {
                ranges
                    .into_iter()
                    .map(|r| Scope {
                        text: text::slice(input, r.start, r.end),
                        offset: r.start,
                    })
                    .collect()
            }
        }
        "section" => {
            let headings = matches(r"^#{1,6}\s+.*$", "m");
            if headings.is_empty() {
                return vec![Scope {
                    text: input.into(),
                    offset: 0,
                }];
            }
            let levels = headings
                .iter()
                .map(|r| {
                    units[r.start..r.end]
                        .iter()
                        .take_while(|c| **c == u16::from(b'#'))
                        .count()
                })
                .collect::<Vec<_>>();
            headings
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let end = headings
                        .iter()
                        .enumerate()
                        .skip(i + 1)
                        .find(|(j, _)| levels[*j] <= levels[i])
                        .map_or(units.len(), |(_, r)| r.start);
                    Scope {
                        text: text::slice(input, r.start, end),
                        offset: r.start,
                    }
                })
                .collect()
        }
        "task" | "task-todo" | "task-done" => {
            let regex =
                regress::Regex::new(r"^\s*[-*+]\s+\[([^\]])\]\s+").expect("task expression");
            lines(input)
                .into_iter()
                .filter(|line| {
                    let units = line.text.encode_utf16().collect::<Vec<_>>();
                    let Some(m) = regex.find_from_utf16(&units, 0).next() else {
                        return false;
                    };
                    let done = m
                        .group(1)
                        .is_some_and(|r| matches!(units[r.start], 88 | 120));
                    kind == "task" || done == (kind == "task-done")
                })
                .collect()
        }
        _ => vec![],
    }
}
fn lines(input: &str) -> Vec<Scope> {
    let mut offset = 0;
    input
        .split('\n')
        .map(|line| {
            let scope = Scope {
                text: line.into(),
                offset,
            };
            offset += text::len(line) + 1;
            scope
        })
        .collect()
}
