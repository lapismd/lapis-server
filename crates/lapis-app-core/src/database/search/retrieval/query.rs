use super::lexer::tokenize;
use crate::database::invalid;
use crate::services::Result;

pub(super) enum Expr {
    Empty,
    Literal(Box<Literal>),
    And(Vec<Expr>),
    Or(Vec<Expr>),
    Not(Box<Expr>),
    Field(String, Box<Expr>),
    Property(String, Option<Box<Expr>>),
    Compare(String, Box<Expr>),
}
pub(super) enum Kind {
    Word,
    Phrase,
    Regex,
    Null,
}
pub(super) struct Literal {
    pub text: String,
    pub kind: Kind,
    pub regex: [Option<regress::Regex>; 2],
}
impl Literal {
    pub(super) fn new(text: String, kind: Kind) -> Box<Self> {
        let regex = if matches!(kind, Kind::Regex) {
            [
                regress::Regex::with_flags(&text, "iu").ok(),
                regress::Regex::with_flags(&text, "u").ok(),
            ]
        } else {
            [None, None]
        };
        Box::new(Self { text, kind, regex })
    }
}
pub(super) struct Query {
    pub raw: String,
    pub expr: Option<Expr>,
}
impl Query {
    pub fn new(raw: String) -> Result<Self> {
        if raw.len() > 65_536 || raw.encode_utf16().count() > 32_768 {
            return Err(invalid("Search query exceeds bounds"));
        }
        let tokens = tokenize(&raw);
        if tokens.as_ref().is_some_and(|t| t.len() > 4096) {
            return Err(invalid("Search query exceeds token bounds"));
        }
        let expr = if let Some(tokens) = tokens {
            super::parser::parse(tokens)?
        } else {
            None
        };
        Ok(Self { raw, expr })
    }
    pub fn structured(&self) -> bool {
        self.expr.as_ref().is_some_and(structured)
    }
    pub fn collected(&self) -> Option<Vec<String>> {
        self.expr.as_ref().filter(|e| can_collect(e)).map(|e| {
            let mut out = vec![];
            collect(e, &mut out, false);
            out
        })
    }
    pub fn properties(&self) -> Vec<String> {
        self.expr
            .as_ref()
            .filter(|e| can_collect(e))
            .map(|e| {
                let mut out = vec![];
                collect(e, &mut out, true);
                out
            })
            .unwrap_or_default()
    }
    pub fn terms(&self) -> Vec<String> {
        self.collected().unwrap_or_else(|| legacy(&self.raw))
    }
}
pub(super) fn legacy(raw: &str) -> Vec<String> {
    raw.to_lowercase()
        .split(super::text::whitespace)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
fn structured(e: &Expr) -> bool {
    match e {
        Expr::Empty => false,
        Expr::Literal(l) => matches!(l.kind, Kind::Regex | Kind::Null),
        Expr::And(v) => v.iter().any(structured),
        _ => true,
    }
}
fn can_collect(e: &Expr) -> bool {
    match e {
        Expr::Empty | Expr::Property(_, None) => true,
        Expr::Literal(l) => matches!(l.kind, Kind::Word | Kind::Phrase),
        Expr::And(v) => v.iter().all(can_collect),
        _ => false,
    }
}
fn collect(e: &Expr, out: &mut Vec<String>, properties: bool) {
    match e {
        Expr::Property(name, None) => out.push(name.to_lowercase()),
        Expr::Literal(l) if !properties && !l.text.is_empty() => out.push(l.text.to_lowercase()),
        Expr::And(v) => {
            for e in v {
                collect(e, out, properties)
            }
        }
        _ => (),
    }
}
