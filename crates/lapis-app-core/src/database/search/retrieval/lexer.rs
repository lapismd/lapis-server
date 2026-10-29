#[derive(Clone, PartialEq)]
pub(super) enum Token {
    Open,
    Close,
    BracketOpen,
    BracketClose,
    Colon,
    Minus,
    Or,
    Null,
    Identifier(String),
    Word(String),
    Phrase(String),
    Regex(String),
    Comparison(String),
}
fn id(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}
fn word(c: char) -> bool {
    id(c) || matches!(c, '.' | '#' | '*' | '?' | '/')
}
pub(super) fn tokenize(raw: &str) -> Option<Vec<Token>> {
    let mut result = vec![];
    let chars: Vec<_> = raw.chars().collect();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if super::text::whitespace(c) {
            at += 1;
            continue;
        }
        let simple = match c {
            '(' => Some(Token::Open),
            ')' => Some(Token::Close),
            '[' => Some(Token::BracketOpen),
            ']' => Some(Token::BracketClose),
            ':' => Some(Token::Colon),
            '-' => Some(Token::Minus),
            _ => None,
        };
        if let Some(t) = simple {
            result.push(t);
            at += 1;
            continue;
        }
        if c == '"' || c == '/' {
            let delimiter = c;
            at += 1;
            let mut value = String::new();
            let mut closed = false;
            while at < chars.len() {
                let c = chars[at];
                at += 1;
                if c == delimiter {
                    closed = true;
                    break;
                }
                if c == '\n' || c == '\r' {
                    return None;
                }
                if c == '\\' {
                    let escaped = *chars.get(at)?;
                    at += 1;
                    if delimiter == '"' && matches!(escaped, '"' | '\\' | 'n' | 'r' | 't') {
                        value.push(match escaped {
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            c => c,
                        });
                    } else {
                        value.push('\\');
                        value.push(escaped)
                    }
                } else {
                    value.push(c)
                }
            }
            if !closed {
                return None;
            }
            result.push(if delimiter == '"' {
                Token::Phrase(value)
            } else {
                Token::Regex(value)
            });
            continue;
        }
        if c == '<' || c == '>' {
            let mut op = c.to_string();
            at += 1;
            if chars.get(at) == Some(&'=') {
                op.push('=');
                at += 1
            }
            result.push(Token::Comparison(op));
            continue;
        }
        if !word(c) {
            return None;
        }
        let start = at;
        let mut word_end = at;
        while word_end < chars.len() && word(chars[word_end]) {
            word_end += 1
        }
        let mut id_end = at;
        while id_end < chars.len() && id(chars[id_end]) {
            id_end += 1
        }
        if id_end > start {
            while chars.get(id_end) == Some(&'-') && chars.get(id_end + 1).is_some_and(|c| id(*c)) {
                id_end += 1;
                while id_end < chars.len() && id(chars[id_end]) {
                    id_end += 1
                }
            }
        }
        at = word_end.max(id_end);
        let value: String = chars[start..at].iter().collect();
        result.push(match value.as_str() {
            "OR" => Token::Or,
            "null" => Token::Null,
            _ if id_end == at => Token::Identifier(value),
            _ => Token::Word(value),
        });
    }
    Some(result)
}
