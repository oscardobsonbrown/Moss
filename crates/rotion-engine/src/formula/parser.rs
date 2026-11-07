use crate::{EngineError, error};
use serde_json::{Value, json};
#[derive(Debug)]
pub enum Expr {
    Value(Value),
    List(Vec<Expr>),
    Unary(String, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}
#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    String(String),
    Ident(String),
    Symbol(String),
    End,
}

pub struct Parser {
    tokens: Vec<Token>,
    at: usize,
}
impl Parser {
    pub fn parse(s: &str) -> Result<Expr, EngineError> {
        let mut p = Self {
            tokens: lex(s)?,
            at: 0,
        };
        let expr = p.expression(0)?;
        if p.peek() != &Token::End {
            return Err(error("Unexpected token after expression"));
        }
        Ok(expr)
    }
    fn peek(&self) -> &Token {
        &self.tokens[self.at]
    }
    fn next(&mut self) -> Token {
        let t = self.peek().clone();
        self.at += 1;
        t
    }
    fn symbol(&mut self, value: &str) -> bool {
        if self.peek() == &Token::Symbol(value.into()) {
            self.next();
            true
        } else {
            false
        }
    }
    fn expression(&mut self, min: u8) -> Result<Expr, EngineError> {
        let mut left = match self.next() {
            Token::Number(n) => Expr::Value(json!(n)),
            Token::String(s) => Expr::Value(json!(s)),
            Token::Ident(s) if s == "true" || s == "false" => Expr::Value(json!(s == "true")),
            Token::Ident(s) if s == "null" => Expr::Value(Value::Null),
            Token::Ident(s) if s == "not" => Expr::Unary(s, Box::new(self.expression(7)?)),
            Token::Ident(s) => {
                if !self.symbol("(") {
                    return Err(error(format!("Expected '(' after {s}")));
                }
                Expr::Call(s, self.arguments(")")?)
            }
            Token::Symbol(s) if s == "-" || s == "!" => {
                Expr::Unary(s, Box::new(self.expression(7)?))
            }
            Token::Symbol(s) if s == "(" => {
                let x = self.expression(0)?;
                if !self.symbol(")") {
                    return Err(error("Expected ')'"));
                }
                x
            }
            Token::Symbol(s) if s == "[" => Expr::List(self.arguments("]")?),
            _ => return Err(error("Expression expected")),
        };
        loop {
            let op = match self.peek() {
                Token::Symbol(s) | Token::Ident(s) => s.clone(),
                _ => break,
            };
            let prec = match op.as_str() {
                "or" | "||" => 1,
                "and" | "&&" => 2,
                "==" | "!=" => 3,
                ">" | ">=" | "<" | "<=" => 4,
                "+" | "-" => 5,
                "*" | "/" | "%" => 6,
                _ => break,
            };
            if prec < min {
                break;
            }
            self.next();
            let right = self.expression(prec + 1)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn arguments(&mut self, end: &str) -> Result<Vec<Expr>, EngineError> {
        let mut args = Vec::new();
        if self.symbol(end) {
            return Ok(args);
        }
        loop {
            args.push(self.expression(0)?);
            if self.symbol(end) {
                return Ok(args);
            }
            if !self.symbol(",") {
                return Err(error(format!("Expected ',' or '{end}'")));
            }
        }
    }
}
fn lex(s: &str) -> Result<Vec<Token>, EngineError> {
    let mut chars = s.chars().peekable();
    let mut out = Vec::new();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        if c.is_ascii_digit() {
            let mut n = c.to_string();
            while chars
                .peek()
                .is_some_and(|x| x.is_ascii_digit() || *x == '.')
            {
                n.push(chars.next().unwrap());
            }
            out.push(Token::Number(
                n.parse().map_err(|_| error("Invalid number"))?,
            ));
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let mut id = c.to_string();
            while chars
                .peek()
                .is_some_and(|x| x.is_alphanumeric() || *x == '_')
            {
                id.push(chars.next().unwrap());
            }
            out.push(Token::Ident(id));
            continue;
        }
        if c == '"' || c == '\'' {
            let mut text = String::new();
            let mut closed = false;
            while let Some(ch) = chars.next() {
                if ch == c {
                    closed = true;
                    break;
                }
                if ch == '\\' {
                    let escaped = chars.next().ok_or_else(|| error("Incomplete escape"))?;
                    text.push(match escaped {
                        'n' => '\n',
                        't' => '\t',
                        '\\' => '\\',
                        '\'' => '\'',
                        '"' => '"',
                        _ => return Err(error("Unknown escape")),
                    });
                } else {
                    text.push(ch);
                }
            }
            if !closed {
                return Err(error("Unclosed string"));
            }
            out.push(Token::String(text));
            continue;
        }
        if "()[],+-*/%<>=!&|".contains(c) {
            let mut op = c.to_string();
            if let Some(&next) = chars.peek() {
                if ("<>=!".contains(c) && next == '=')
                    || (c == '&' && next == '&')
                    || (c == '|' && next == '|')
                {
                    op.push(chars.next().unwrap());
                }
            }
            if op == "=" || op == "&" || op == "|" {
                return Err(error("Invalid operator"));
            }
            out.push(Token::Symbol(op));
            continue;
        }
        return Err(error(format!("Unexpected character: {c}")));
    }
    out.push(Token::End);
    Ok(out)
}
