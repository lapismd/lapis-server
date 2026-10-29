use super::super::{
    query::{Expr, Kind, Query},
    scopes, text,
};
use super::{Eval, combine, mapped};
use lapis_app_protocol::database::search::retrieval::{SearchField, SearchRange};
pub(in crate::database::search::retrieval) fn ranges(
    query: &Query,
    input: &str,
    field: SearchField,
    sensitive: bool,
) -> Vec<SearchRange> {
    text::merge(if let Some(e) = &query.expr {
        evaluate_text(e, input, Some(field), sensitive).ranges
    } else {
        super::super::query::legacy(&query.raw)
            .into_iter()
            .flat_map(|term| text::ranges(input, &term, sensitive))
            .collect()
    })
}
pub(super) fn evaluate_text(
    e: &Expr,
    input: &str,
    field: Option<SearchField>,
    sensitive: bool,
) -> Eval {
    match e {
        Expr::Empty => Eval::yes(0., vec![]),
        Expr::Literal(l) => {
            let ranges = text::literal(input, l, sensitive);
            if ranges.is_empty() {
                Eval::default()
            } else {
                Eval::yes(
                    ranges.len() as f64
                        * if matches!(l.kind, Kind::Regex) {
                            20.
                        } else {
                            text::len(&l.text).max(1) as f64
                        },
                    ranges,
                )
            }
        }
        Expr::And(items) | Expr::Or(items) => combine(
            items
                .iter()
                .map(|e| evaluate_text(e, input, field, sensitive))
                .collect(),
            matches!(e, Expr::And(_)),
        ),
        Expr::Not(e) => {
            if evaluate_text(e, input, field, sensitive).matched {
                Eval::default()
            } else {
                Eval::yes(1., vec![])
            }
        }
        Expr::Field(name, value) => {
            let name = name.to_lowercase();
            if name == "match-case" || name == "ignore-case" {
                return evaluate_text(value, input, field, name == "match-case");
            }
            if let Some(target) = mapped(&name) {
                return if field == Some(target) {
                    evaluate_text(value, input, field, sensitive)
                } else {
                    Eval::default()
                };
            }
            if field == Some(SearchField::Content) {
                evaluate_scopes(&name, input, value, sensitive)
            } else {
                Eval::default()
            }
        }
        Expr::Property(name, value) => {
            if field != Some(SearchField::Metadata) {
                return Eval::default();
            }
            let names = name_ranges(input, name, sensitive);
            if names.is_empty() {
                return Eval::default();
            }
            let Some(value) = value else {
                return Eval::yes(names.len() as f64 * 18., names);
            };
            let e = evaluate_text(value, input, field, sensitive);
            if !e.matched {
                return e;
            }
            Eval::yes(
                e.score + 18.,
                text::merge(names.into_iter().chain(e.ranges).collect()),
            )
        }
        Expr::Compare(_, _) => Eval::default(),
    }
}
pub(super) fn name_ranges(input: &str, name: &str, sensitive: bool) -> Vec<SearchRange> {
    let ranges = text::ranges(input, name, sensitive);
    if !ranges.is_empty() {
        return ranges;
    }
    let clean = name.replace("[]", "");
    let parts = clean
        .split('.')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    if parts.len() < 2 {
        return ranges;
    }
    text::merge(
        parts
            .into_iter()
            .flat_map(|p| text::ranges(input, p, sensitive))
            .collect(),
    )
}
pub(super) fn evaluate_scopes(kind: &str, input: &str, e: &Expr, sensitive: bool) -> Eval {
    let matches = scopes::scopes(kind, input)
        .into_iter()
        .map(|s| {
            let mut e = evaluate_text(e, &s.text, Some(SearchField::Content), sensitive);
            if e.matched {
                e.score = e.score.max(1.)
            }
            for r in &mut e.ranges {
                r.start += s.offset;
                r.end += s.offset
            }
            e
        })
        .collect();
    combine(matches, false)
}
