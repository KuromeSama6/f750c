//! This module defines functions and structures for the tokenization step of the F750 compiler.
//!
//! In the F750 compiler flow, each line in a source file is first tokenized into a sequence of `ParserToken`s, which are then parsed into semantic representations of the source code.

use std::collections::VecDeque;
use std::fmt::Display;
use std::str::FromStr;
use crate::opcode::ReservedWord;
use crate::parser::{ParseError, ParseResult};
use crate::tokenizer;

/// Denotes the beginning of a comment.
/// During tokenization of a line, all characters (including this character) beyond the comment delimiter are ignored.
pub const ATOM_COMMENT: char = ';';
/// Denotes whitespace.
pub const ATOM_WHITESPACE: char = ' ';
/// Denotes a separator. Separators are used to separate arguments in a list of arguments.
pub const ATOM_SEPARATOR: char = ',';
/// Denotes a colon character. Colons are used to separate binding names from their values, as well as marking end-of-line for label and section definitions.
pub const ATOM_COLON: char = ':';
/// Denotes a quote character. Quotes are used to denote string literals.
pub const ATOM_QUOTE: char = '"';
/// Denotes an underscore character. Underscores are used in binding names and labels.
pub const ATOM_UNDERLINE: char = '_';
/// Denotes an integer memory offset from an address.
pub const ATOM_PLUS: char = '+';
/// Denotes memory dereference.
pub const ATOM_DEREF: char = '&';
/// Denotes either an engine parameter reference, or the beginning of a compiler directive.
pub const ATOM_HASHTAG: char = '#';
/// Denotes the beginning of a compiler construct.
pub const ATOM_AT: char = '@';

/// The character dot(.).
pub const CHAR_DOT: char = '.';

/// The beginning of the special '.data' section.
pub const SPECIAL_SECTION_LABEL_DATA: &str = ".data";

/// Represents a token during tokenization.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Whitespace,
    Separator,
    Colon,
    Offset,
    Deref,
    Hashtag,
    Annotation,
    /// Represents the namespace separator `::` used in labels and bindings.
    NamespaceSeparator,
    IntLiteral(i64),
    FloatLiteral(f64),
    /// Represents a quoted string literal, which is enclosed in double quotes.
    QuotedString(String),
    Keyword(ReservedWord),
    /// Represents anything that does not match any of the other token types, such as identifiers, labels, and binding names.
    Token(String),
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Whitespace => write!(f, " "),
            Token::Separator => write!(f, ","),
            Token::Colon => write!(f, ":"),
            Token::Offset => write!(f, "+"),
            Token::Deref => write!(f, "&"),
            Token::Hashtag => write!(f, "#"),
            Token::Annotation => write!(f, "@"),
            Token::NamespaceSeparator => write!(f, "::"),
            Token::IntLiteral(val) => write!(f, "{}", val),
            Token::FloatLiteral(val) => write!(f, "{}", val),
            Token::QuotedString(val) => write!(f, "\"{}\"", val),
            Token::Keyword(val) => write!(f, "{}", val.as_ref()),
            Token::Token(val) => write!(f, "{}", val),
        }
    }
}

pub type TokenizedLine = Vec<Token>;

/// Utility for managing a stream of tokens during parsing. This structure allows for peeking at the next token, consuming tokens, and checking for the end of the stream.
#[derive(Debug)]
pub struct TokenStream {
    tokens: VecDeque<Token>,
    pub cur: usize,
}

impl TokenStream {
    pub fn new(tokens: &[Token]) -> Self {
        Self {
            tokens: VecDeque::from(tokens.to_vec()),
            cur: 0,
        }
    }

    /// Peeks at the next token in the stream without consuming it. If there are no more tokens, it returns `None`.
    pub fn peek(&self) -> Option<&Token> {
        self.tokens.get(0)
    }

    /// Peeks at the `n`th token in the stream without consuming it. If there are fewer than n tokens remaining, it returns `None`.
    pub fn peek_n(&self, n: usize) -> Option<&Token> {
        self.tokens.get(n)
    }

    /// Peeks at the next non-whitespace token in the stream without consuming it.
    pub fn peek_non_whitespace(&self) -> Option<&Token> {
        for token in &self.tokens {
            if token != &Token::Whitespace {
                return Some(token);
            }
        }
        None
    }

    /// Consumes and returns the next token in the stream, advancing the current position. If there are no more tokens, it returns `None`.
    pub fn next(&mut self) -> Option<Token> {
        let token = self.tokens.pop_front();
        if token.is_some() {
            self.cur += 1;
        }
        token
    }

    /// Consumes and returns the next token in the stream, advancing the current position. If there are no more tokens, it returns a `ParseError` indicating an unexpected end of line.
    pub fn next_or_err(&mut self) -> ParseResult<Token> {
        let cur = self.cur;
        self.next().ok_or_else(|| ParseError::UnexpectedEOL)
    }

    /// Consumes all `Whitespace` tokens in the stream until a non-`Whitespace` token is encountered or the end of the stream is reached. Returns the next non-whitespace token, or `None` if there are no more tokens.
    pub fn next_non_whitespace(&mut self) -> Option<Token> {
        while let Some(token) = self.next() {
            if token != Token::Whitespace {
                return Some(token);
            }
        }

        None
    }

    /// Consumes all `Whitespace` tokens in the stream until a non-`Whitespace` token is encountered or the end of the stream is reached. Returns the next non-whitespace token, or a `ParseError` if there are no more tokens.
    pub fn next_non_whitespace_or_err(&mut self) -> ParseResult<Token> {
        let cur = self.cur;
        self.next_non_whitespace().ok_or_else(|| ParseError::UnexpectedEOL)
    }

    /// Consumes the next token in the stream and checks if it matches the expected token. Returns `Ok(())` if it matches, or a `ParseError` if it does not.
    pub fn expect(&mut self, expected: Token) -> ParseResult<()> {
        let cur = self.cur;
        let token = self.next_or_err()?;
        if token != expected {
            return Err(ParseError::UnexpectedToken(format!("Expected {:?}, found {:?}", expected, token)));
        }
        Ok(())
    }

    /// Consumes all `Whitespace` tokens in the stream until a non-`Whitespace` token is encountered or the end of the stream is reached. Checks if the next non-whitespace token matches the expected token. Returns `Ok(())` if it matches, or a `ParseError` if it does not.
    pub fn expect_non_whitespace(&mut self, expected: Token) -> ParseResult<()> {
        let cur = self.cur;
        let token = self.next_non_whitespace_or_err()?;
        if token != expected {
            return Err(ParseError::UnexpectedToken(format!("Expected {:?}, found {:?}", expected, token)));
        }
        Ok(())
    }

    /// Consumes all `Whitespace` tokens in the stream until a non-`Whitespace` token is encountered or the end of the stream is reached.
    pub fn skip_whitespace(&mut self) {
        while let Some(Token::Whitespace) = self.peek() {
            self.next();
        }
    }

    /// Returns whether there are more tokens in the stream. This does not consume any tokens.
    pub fn has_more(&self) -> bool {
        !self.tokens.is_empty()
    }
}

/// Tokenizes a line of source code into a vector of `ParserToken`s, which represent the lexical structure of the line.
pub fn tokenize_line(line: &str) -> ParseResult<TokenizedLine> {
    let mut ret = Vec::new();
    let mut cur = 0usize;

    while cur < line.len() {
        let c = line[cur..].chars().next().unwrap();
        match c {
            ATOM_COMMENT => {
                // Ignore the rest of the line after a comment
                break;
            }
            ATOM_WHITESPACE | '\t' => {
                ret.push(Token::Whitespace);
                cur += 1;
            }
            ATOM_SEPARATOR => {
                ret.push(Token::Separator);
                cur += 1;
            }
            ATOM_COLON => {
                // Handle namespace separator or label colon
                if cur + 1 < line.len() && line[cur + 1..].starts_with(ATOM_COLON) {
                    ret.push(Token::NamespaceSeparator);
                    cur += 2; // Skip both colons

                } else {
                    ret.push(Token::Colon);
                    cur += 1;
                }
            }
            ATOM_PLUS => {
                ret.push(Token::Offset);
                cur += 1;
            }
            ATOM_DEREF => {
                ret.push(Token::Deref);
                cur += 1;
            }
            ATOM_HASHTAG => {
                // Handle offset with hashtag
                ret.push(Token::Hashtag);
                cur += 1;
            }
            ATOM_AT => {
                ret.push(Token::Annotation);
                cur += 1;
            }
            _ => {
                if c == ATOM_QUOTE {
                    // Handle quoted string
                    let end_quote = line[cur + 1..].find(ATOM_QUOTE);
                    if let Some(end) = end_quote {
                        let token = &line[cur + 1..cur + 1 + end]; // Exclude quotes
                        ret.push(Token::QuotedString(token.to_string()));
                        cur += 1 + end + 1; // Move past the quoted string

                    } else {
                        // Unterminated quote
                        return Err(ParseError::UnterminatedQuote);
                    }

                } else {
                    // Handle regular token
                    let Some(first) = line.chars().nth(cur) else {
                        return Err(ParseError::UnexpectedEOL);
                    };

                    let is_digit = first.is_ascii_digit() || first == '-' || first == '+';

                    let next_special = line[cur..]
                        .find(|c| is_special_atom(c, is_digit))
                        .unwrap_or(line.len() - cur);

                    let token = &line[cur..cur + next_special];

                    if let Ok(int_val) = parse_int::parse(token) {
                        ret.push(Token::IntLiteral(int_val));
                    } else if let Ok(float_val) = token.parse::<f64>() {
                        ret.push(Token::FloatLiteral(float_val));
                    } else if let Ok(keyword) = ReservedWord::from_str(token) {
                        ret.push(Token::Keyword(keyword));
                    } else {
                        ret.push(Token::Token(token.to_string()));
                    }
                    cur += next_special;
                }
            }
        }
    }

    // trim whitespace tokens from the end
    while let Some(Token::Whitespace) = ret.last() {
        ret.pop();
    }

    Ok(ret)
}

fn is_special_atom(ch: char, exclude_digit: bool) -> bool {
    ch == ATOM_WHITESPACE || ch == ATOM_SEPARATOR || ch == ATOM_COLON || ch == ATOM_QUOTE || ch == ATOM_PLUS || ch == ATOM_DEREF || ch == ATOM_HASHTAG || ch == ATOM_AT
}