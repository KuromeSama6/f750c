use std::collections::VecDeque;
use std::fmt::Display;
use thiserror::Error;
use crate::semantic::{SemanticBindingDef, SemanticInstruction};
use crate::{constants, semantic, token, util};

#[derive(Debug, Error)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub cur: usize,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parse error at position {}: {:?}", self.cur, self.kind)
    }
}

#[derive(Debug, Error)]
pub enum ParseErrorKind {
    #[error("Underminated quote in string literal")]
    UnterminatedQuote,

    #[error("Unexpected token")]
    UnexpectedToken,
    #[error("Unexpected end of line")]
    UnexpectedEOL,
}

impl ParseErrorKind {
    pub fn to_error(self, cur: usize) -> ParseError {
        ParseError {
            kind: self,
            cur,
        }
    }
}

pub type ParseResult<T> = Result<T, ParseError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseLineContext {
    None,
    BindingDef,
}

#[derive(Debug)]
pub enum ParsedLine {
    SpecialSection(String),
    BindingDef(SemanticBindingDef),
    Label(String),
    Instruction(SemanticInstruction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParserToken {
    Dot,
    Whitespace,
    Separator,
    Colon,
    Token(String),
}

impl ParserToken {
    pub fn tokenize(line: &str) -> ParseResult<Vec<Self>> {
        let mut ret = Vec::new();
        let mut cur = 0usize;

        while cur < line.len() {
            let c = line[cur..].chars().next().unwrap();
            match c {
                token::ATOM_COMMENT => {
                    // Ignore the rest of the line after a comment
                    break;
                }
                token::ATOM_DOT => {
                    ret.push(ParserToken::Dot);
                    cur += 1;
                }
                token::ATOM_WHITESPACE | '\t' => {
                    ret.push(ParserToken::Whitespace);
                    cur += 1;
                }
                token::ATOM_SEPARATOR => {
                    ret.push(ParserToken::Separator);
                    cur += 1;
                }
                token::ATOM_COLOR => {
                    ret.push(ParserToken::Colon);
                    cur += 1;
                }
                _ => {
                    if c == token::ATOM_QUOTE {
                        // Handle quoted string
                        let end_quote = line[cur + 1..].find(token::ATOM_QUOTE);
                        if let Some(end) = end_quote {
                            let token = &line[cur + 1..cur + 1 + end]; // Exclude quotes
                            ret.push(ParserToken::Token(token.to_string()));
                            cur += 1 + end + 1; // Move past the quoted string

                        } else {
                            // Unterminated quote
                            return Err(ParseErrorKind::UnterminatedQuote.to_error(cur));
                        }

                    } else {
                        // Handle regular token
                        let next_special = line[cur..]
                            .find(|ch| ch == token::ATOM_DOT || ch == token::ATOM_WHITESPACE || ch == token::ATOM_SEPARATOR || ch == token::ATOM_COLOR || ch == token::ATOM_QUOTE)
                            .unwrap_or(line.len() - cur);
                        let token = &line[cur..cur + next_special];
                        ret.push(ParserToken::Token(token.to_string()));
                        cur += next_special;
                    }
                }
            }
        }

        // trim whitespace tokens from the end
        while let Some(ParserToken::Whitespace) = ret.last() {
            ret.pop();
        }

        Ok(ret)
    }
}

#[derive(Debug)]
pub struct TokenStream {
    tokens: VecDeque<ParserToken>,
    cur: usize,
}

impl TokenStream {
    pub fn new(tokens: &[ParserToken]) -> Self {
        Self {
            tokens: VecDeque::from(tokens.to_vec()),
            cur: 0,
        }
    }

    pub fn peek(&self) -> Option<&ParserToken> {
        self.tokens.get(0)
    }

    pub fn next(&mut self) -> Option<ParserToken> {
        let token = self.tokens.pop_front();
        if token.is_some() {
            self.cur += 1;
        }
        token
    }

    pub fn next_or_err(&mut self) -> ParseResult<ParserToken> {
        let cur = self.cur;
        self.next().ok_or_else(|| ParseErrorKind::UnexpectedEOL.to_error(cur))
    }

    pub fn next_non_whitespace(&mut self) -> Option<ParserToken> {
        while let Some(token) = self.next() {
            if token != ParserToken::Whitespace {
                return Some(token);
            }
        }

        None
    }

    pub fn next_non_whitespace_or_err(&mut self) -> ParseResult<ParserToken> {
        let cur = self.cur;
        self.next_non_whitespace().ok_or_else(|| ParseErrorKind::UnexpectedEOL.to_error(cur))
    }

    pub fn expect(&mut self, expected: ParserToken) -> ParseResult<()> {
        let cur = self.cur;
        let token = self.next_or_err()?;
        if token != expected {
            return Err(ParseErrorKind::UnexpectedToken.to_error(cur));
        }
        Ok(())
    }

    pub fn expect_non_whitespace(&mut self, expected: ParserToken) -> ParseResult<()> {
        let cur = self.cur;
        let token = self.next_non_whitespace_or_err()?;
        if token != expected {
            return Err(ParseErrorKind::UnexpectedToken.to_error(cur));
        }
        Ok(())
    }

    pub fn skip_whitespace(&mut self) {
        while let Some(ParserToken::Whitespace) = self.peek() {
            self.next();
        }
    }

    pub fn has_more(&self) -> bool {
        !self.tokens.is_empty()
    }
}

pub fn parse_source(source: &[&str]) -> ParseResult<Vec<ParsedLine>> {
    let mut ctx = ParseLineContext::None;
    let mut ret = Vec::new();

    for line in source {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let tokens = ParserToken::tokenize(line)?;
        if tokens.is_empty() {
            continue;
        }

        let parsed_line = parse_line(&tokens, &mut ctx)?;
        ret.push(parsed_line);
    }

    Ok(ret)
}

fn parse_line(tokens: &[ParserToken], ctx: &mut ParseLineContext) -> ParseResult<ParsedLine> {
    let mut stream = TokenStream::new(tokens);

    if *ctx == ParseLineContext::BindingDef {
        let binding_name: String;
        let mut is_constant = false;

        let first = stream.next_non_whitespace_or_err()?;

        match first {
            ParserToken::Token(token) => {
                if token == token::TOKEN_CONST_BINDING {
                    let name = stream.next_non_whitespace_or_err()?;
                    if let ParserToken::Token(s) = name {
                        binding_name = s;
                        is_constant = true;

                    } else {
                        return Err(ParseErrorKind::UnexpectedToken.to_error(stream.cur));
                    }

                } else {
                    binding_name = token;
                }
            }
            _ => {
                return Err(ParseErrorKind::UnexpectedToken.to_error(stream.cur));
            }
        }

        stream.expect_non_whitespace(ParserToken::Colon)?;

        let mut values = SemanticBindingDef::parse_values(&mut stream)?;
        let ret = SemanticBindingDef {
            name: binding_name,
            constant: is_constant,
            values,
        };

        return Ok(ParsedLine::BindingDef(ret));
    }

    todo!()
}

mod test {
    use crate::parser::ParserToken;

    const TEST_TOKENIZE_LINE: &str = "const String: \"Hello, World!, 0 ; This is a comment";

    #[test]
    fn test_tokenize() {
        let res = ParserToken::tokenize(TEST_TOKENIZE_LINE);
        if let Err(e) = res {
            panic!("Tokenization failed: {:?}", e);
        }

        println!("Tokens: {:?}", res.unwrap());
    }
}