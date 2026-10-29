use super::super::{
    properties::{self, Properties},
    query::{Expr, Kind, Query},
    text,
};
use super::text::evaluate_text;
use super::{Eval, combine, field, mapped, weight};
use icu_collator::CollatorBorrowed;
use lapis_app_protocol::database::search::{records::SearchDocumentRecord, retrieval::SearchField};
pub(super) fn document(
    e: &Expr,
    doc: &SearchDocumentRecord,
    props: &Properties,
    sensitive: bool,
    locale: &CollatorBorrowed<'_>,
) -> Eval {
    match e {
        Expr::Empty => Eval::yes(0., vec![]),
        Expr::Literal(l) => {
            let mut score = 0.;
            for (field, input) in fields(doc) {
                score += text::literal(&input, l, sensitive).len() as f64 * weight(field)
            }
            if score > 0. {
                Eval::yes(score, vec![])
            } else {
                Eval::default()
            }
        }
        Expr::And(items) | Expr::Or(items) => combine(
            items
                .iter()
                .map(|e| document(e, doc, props, sensitive, locale))
                .collect(),
            matches!(e, Expr::And(_)),
        ),
        Expr::Not(e) => {
            if document(e, doc, props, sensitive, locale).matched {
                Eval::default()
            } else {
                Eval::yes(1., vec![])
            }
        }
        Expr::Field(name, value) => {
            let name = name.to_lowercase();
            if name == "match-case" || name == "ignore-case" {
                return document(value, doc, props, name == "match-case", locale);
            }
            if let Some(mapped) = mapped(&name) {
                let mut e = evaluate_text(value, &field(doc, mapped), Some(mapped), sensitive);
                if e.matched {
                    e.score = e.score.max(1.) + weight(mapped)
                }
                return e;
            }
            let mut e = super::text::evaluate_scopes(&name, &doc.content, value, sensitive);
            if e.matched {
                e.score += 12.
            }
            e
        }
        Expr::Property(name, value) => {
            let Some(raw) = props.get(name) else {
                return Eval::default();
            };
            let Some(value) = value else {
                return Eval::yes(30., vec![]);
            };
            if let Expr::Compare(op, target) = value.as_ref() {
                return if properties::compare(&raw, op, target, locale) {
                    Eval::yes(35., vec![])
                } else {
                    Eval::default()
                };
            }
            if matches!(value.as_ref(),Expr::Literal(l) if matches!(l.kind,Kind::Null)) {
                return if properties::nullish(&raw) {
                    Eval::yes(35., vec![])
                } else {
                    Eval::default()
                };
            }
            let mut evaluation = combine(
                properties::texts(&raw)
                    .into_iter()
                    .map(|s| evaluate_text(value, &s, None, sensitive))
                    .collect(),
                false,
            );
            if evaluation.matched {
                evaluation.score += 30.
            }
            evaluation
        }
        Expr::Compare(_, _) => Eval::default(),
    }
}
fn fields(doc: &SearchDocumentRecord) -> Vec<(SearchField, String)> {
    let mut content = doc
        .chunks
        .iter()
        .flatten()
        .map(|c| c.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if content.is_empty() {
        content = doc.content.clone()
    }
    vec![
        (SearchField::Name, doc.name.clone()),
        (SearchField::Path, doc.path.clone()),
        (SearchField::Content, content),
        (SearchField::Tags, field(doc, SearchField::Tags)),
        (SearchField::Metadata, field(doc, SearchField::Metadata)),
    ]
}
pub(in crate::database::search::retrieval) fn score(
    query: &Query,
    doc: &SearchDocumentRecord,
    props: &Properties,
    sensitive: bool,
    locale: &CollatorBorrowed<'_>,
) -> f64 {
    if query.structured() || sensitive {
        if let Some(e) = &query.expr {
            return document(e, doc, props, sensitive, locale).score;
        }
        let mut score = 0.;
        let fields = fields(doc);
        for term in super::super::query::legacy(&query.raw) {
            let mut matched = false;
            for (field, input) in &fields {
                let n = text::ranges(input, &term, sensitive).len();
                matched |= n > 0;
                score += n as f64 * weight(*field)
            }
            if !matched {
                return 0.;
            }
        }
        return score;
    }
    let names = query.properties();
    if !props.has_required(&names) {
        return 0.;
    }
    let mut score = names.len() as f64 * 25.;
    let content = fields(doc)
        .into_iter()
        .find(|(f, _)| *f == SearchField::Content)
        .unwrap()
        .1;
    let stacks = [
        doc.path.clone(),
        doc.name.clone(),
        content,
        doc.tags.join(" "),
        doc.tag_parts.join(" "),
        doc.tag_hierarchy.join(" "),
        doc.metadata_text.clone().unwrap_or_default(),
    ]
    .map(|s| s.to_lowercase());
    for term in query
        .terms()
        .into_iter()
        .filter(|term| !names.contains(term))
    {
        let Some(index) = stacks.iter().position(|s| s.contains(&term)) else {
            return 0.;
        };
        score += if index <= 1 { 25. } else { 10. };
    }
    score
}
