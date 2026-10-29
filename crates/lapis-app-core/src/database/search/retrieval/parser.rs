use super::{
    lexer::Token,
    query::{Expr, Kind, Literal},
};
use crate::{database::invalid, services::Result};
pub(super) fn parse(tokens: Vec<Token>) -> Result<Option<Expr>> {
    let mut parser = Parser {
        tokens,
        at: 0,
        depth: 0,
        limited: false,
    };
    let value = if parser.tokens.is_empty() {
        Some(Expr::Empty)
    } else {
        parser.expression()
    };
    if parser.limited {
        return Err(invalid("Search query exceeds nesting bounds"));
    }
    Ok(if parser.at == parser.tokens.len() {
        value
    } else {
        None
    })
}
struct Parser {
    tokens: Vec<Token>,
    at: usize,
    depth: usize,
    limited: bool,
}
impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }
    fn eat(&mut self, t: Token) -> bool {
        if self.peek() == Some(&t) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn expression(&mut self) -> Option<Expr> {
        let mut values = vec![self.and()?];
        while self.eat(Token::Or) {
            values.push(self.and()?)
        }
        Some(if values.len() == 1 {
            values.pop()?
        } else {
            Expr::Or(values)
        })
    }
    fn and(&mut self) -> Option<Expr> {
        let mut values = vec![];
        while self
            .peek()
            .is_some_and(|t| !matches!(t, Token::Or | Token::Close | Token::BracketClose))
        {
            let negate = self.eat(Token::Minus);
            let value = self.primary(true)?;
            values.push(if negate {
                Expr::Not(Box::new(value))
            } else {
                value
            });
        }
        if values.is_empty() {
            None
        } else {
            Some(if values.len() == 1 {
                values.pop()?
            } else {
                Expr::And(values)
            })
        }
    }
    fn primary(&mut self, fields: bool) -> Option<Expr> {
        if self.depth >= 128 {
            self.limited = true;
            return None;
        }
        if self.eat(Token::Open) {
            self.depth += 1;
            let value = if self.peek() == Some(&Token::Close) {
                Expr::Empty
            } else {
                self.expression()?
            };
            self.depth -= 1;
            return self.eat(Token::Close).then_some(value);
        }
        if self.eat(Token::BracketOpen) {
            let name = match self.peek()?.clone() {
                Token::Identifier(s) | Token::Phrase(s) => s,
                _ => return None,
            };
            self.at += 1;
            let value = if self.eat(Token::Colon) {
                Some(Box::new(self.property_value()?))
            } else {
                None
            };
            if !self.eat(Token::BracketClose) {
                return None;
            }
            let value = if value.is_none() && self.eat(Token::Colon) {
                Some(Box::new(self.property_value()?))
            } else {
                value
            };
            return Some(Expr::Property(name, value));
        }
        let token = self.peek()?.clone();
        self.at += 1;
        Some(match token {
            Token::Identifier(name) if fields && self.eat(Token::Colon) => {
                self.depth += 1;
                let value = self.primary(false)?;
                self.depth -= 1;
                Expr::Field(name, Box::new(value))
            }
            Token::Identifier(s) | Token::Word(s) => Expr::Literal(Literal::new(s, Kind::Word)),
            Token::Phrase(s) => Expr::Literal(Literal::new(s, Kind::Phrase)),
            Token::Regex(s) => Expr::Literal(Literal::new(s, Kind::Regex)),
            Token::Null => Expr::Literal(Literal::new(String::new(), Kind::Null)),
            _ => return None,
        })
    }
    fn property_value(&mut self) -> Option<Expr> {
        if let Some(Token::Comparison(op)) = self.peek().cloned() {
            self.at += 1;
            if !matches!(
                self.peek(),
                Some(Token::Identifier(_) | Token::Word(_) | Token::Phrase(_) | Token::Null)
            ) {
                return None;
            }
            let value = self.primary(false)?;
            if !matches!(&value,Expr::Literal(l) if !matches!(l.kind,Kind::Regex)) {
                return None;
            }
            Some(Expr::Compare(op, Box::new(value)))
        } else {
            if self.peek() == Some(&Token::BracketOpen) {
                return None;
            }
            self.primary(false)
        }
    }
}
