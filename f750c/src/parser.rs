use std::collections::VecDeque;
use std::fmt::Display;
use std::str::FromStr;
use thiserror::Error;
use crate::semantic::{SemanticOperand, SemanticArgBody, SemanticBindingDef, SemanticInstruction, SemanticDerefKind, SemanticSymbol, SemanticCompilerConstruct};
use crate::{constants, semantic, token, util};
use crate::opcode::{CompilerConstruct, OpcodeMnemonic};
use crate::value::{DataType, RegisterSpec, RegisterSpecError};

#[derive(Debug, Error, Clone)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub cur: Option<usize>,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parse error at position {:?}: {}", self.cur, self.kind)
    }
}

impl From<ParseErrorKind> for ParseError {
    fn from(kind: ParseErrorKind) -> Self {
        ParseError {
            kind,
            cur: None,
        }
    }
}

#[derive(Debug, Error, Clone)]
pub enum ParseErrorKind {
    #[error("Underminated quote in string literal")]
    UnterminatedQuote,

    #[error("Unexpected token: {0}")]
    UnexpectedToken(String),
    #[error("Unexpected end of line")]
    UnexpectedEOL,
    #[error("Invalid label name: {0}")]
    InvalidLabel(String),
    #[error("Invalid data type for literal '{0}': {1}")]
    InvalidDataTypeForLiteral(String, DataType),

    #[error("Invalid register spec: {0}")]
    InvalidRegisterSpec(#[from] RegisterSpecError),

    #[error("Constant register dereference (#register) is not allowed")]
    ConstantRegisterDerefNotAllowed,
    #[error("Constant literal dereference (#literal) is not allowed")]
    ConstantLiteralDerefNotAllowed,
    #[error("Literal offset is not allowed")]
    LiteralOffsetNotAllowed,
    #[error("Memory offset on a non-dereferenced register or binding is not allowed")]
    NonDerefOffsetNotAllowed,
}

impl ParseErrorKind {
    pub fn to_error(self, cur: usize) -> ParseError {
        ParseError {
            kind: self,
            cur: Some(cur),
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
    CompilerConstruct(SemanticCompilerConstruct),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParserToken {
    Whitespace,
    Separator,
    Colon,
    Offset,
    Deref,
    Hashtag,
    Annotation,
    NamespaceSeparator,
    Token(String),
    IntLiteral(i64),
    FloatLiteral(f64),
    QuotedString(String),
}

#[derive(Debug)]
pub struct TokenStream {
    tokens: VecDeque<ParserToken>,
    pub cur: usize,
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

    pub fn peek_n(&self, n: usize) -> Option<&ParserToken> {
        self.tokens.get(n)
    }

    pub fn peek_non_whitespace(&self) -> Option<&ParserToken> {
        for token in &self.tokens {
            if token != &ParserToken::Whitespace {
                return Some(token);
            }
        }
        None
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
            return Err(ParseErrorKind::UnexpectedToken(format!("Expected {:?}, found {:?}", expected, token)).to_error(cur));
        }
        Ok(())
    }

    pub fn expect_non_whitespace(&mut self, expected: ParserToken) -> ParseResult<()> {
        let cur = self.cur;
        let token = self.next_non_whitespace_or_err()?;
        if token != expected {
            return Err(ParseErrorKind::UnexpectedToken(format!("Expected {:?}, found {:?}", expected, token)).to_error(cur));
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

#[derive(Debug, Default)]
struct SemanticArgBuilder {
    body: Option<SemanticArgBody>,
    deref: Option<SemanticDerefKind>,
    offset: i64,
}

impl SemanticArgBuilder {
    pub fn build(self) -> Result<SemanticOperand, ParseErrorKind> {
        let Some(body) = self.body else {
            return Err(ParseErrorKind::UnexpectedToken("Expected an operand, found none".to_string()));
        };

        // disallow const deref of constant register
        if matches!(body, SemanticArgBody::Register(_)) && self.deref == Some(SemanticDerefKind::Const) {
            return Err(ParseErrorKind::ConstantRegisterDerefNotAllowed);
        }

        // disallow const deref of constant literal
        if matches!(body, SemanticArgBody::Literal(_)) && self.deref == Some(SemanticDerefKind::Const) {
            return Err(ParseErrorKind::ConstantLiteralDerefNotAllowed);
        }

        // disallow literal offset
        if matches!(body, SemanticArgBody::Literal(_)) && self.offset != 0 {
            return Err(ParseErrorKind::LiteralOffsetNotAllowed);
        }

        // disallow offset on non-dereferenced register or binding
        if matches!(self.deref, None) && self.offset != 0 {
            return Err(ParseErrorKind::NonDerefOffsetNotAllowed);
        }

        Ok(SemanticOperand {
            body,
            deref: self.deref,
            offset: self.offset,
        })
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

        let tokens = tokenize_line(line)?;
        // println!("Tokens: {:?}", tokens);
        if tokens.is_empty() {
            continue;
        }

        let res = parse_line(&tokens, ctx);
        // if let Err(e) = res {
        //     let e = e.clone();
        //     let token = tokens[e.cur - 1].clone();
        //     eprintln!("Error parsing line: {:?}, at token: {:?}", e, token);
        //     return Err(e);
        // }

        let parsed_line = res?;
        println!("{:?}", parsed_line);

        if ctx == ParseLineContext::BindingDef && !matches!(parsed_line, ParsedLine::BindingDef(_)) {
            ctx = ParseLineContext::None;
        }

        if ctx == ParseLineContext::None && let ParsedLine::SpecialSection(s) = &parsed_line && s == token::SPECIAL_SECTION_LABEL_DATA {
            ctx = ParseLineContext::BindingDef;
        }
        ret.push(parsed_line);
    }

    Ok(ret)
}

fn tokenize_line(line: &str) -> ParseResult<Vec<ParserToken>> {
    let mut ret = Vec::new();
    let mut cur = 0usize;

    while cur < line.len() {
        let c = line[cur..].chars().next().unwrap();
        match c {
            token::ATOM_COMMENT => {
                // Ignore the rest of the line after a comment
                break;
            }
            token::ATOM_WHITESPACE | '\t' => {
                ret.push(ParserToken::Whitespace);
                cur += 1;
            }
            token::ATOM_SEPARATOR => {
                ret.push(ParserToken::Separator);
                cur += 1;
            }
            token::ATOM_COLON => {
                // Handle namespace separator or label colon
                if cur + 1 < line.len() && line[cur + 1..].starts_with(token::ATOM_COLON) {
                    ret.push(ParserToken::NamespaceSeparator);
                    cur += 2; // Skip both colons

                } else {
                    ret.push(ParserToken::Colon);
                    cur += 1;
                }
            }
            token::ATOM_PLUS => {
                ret.push(ParserToken::Offset);
                cur += 1;
            }
            token::ATOM_DEREF => {
                ret.push(ParserToken::Deref);
                cur += 1;
            }
            token::ATOM_HASHTAG => {
                // Handle offset with hashtag
                ret.push(ParserToken::Hashtag);
                cur += 1;
            }
            token::ATOM_AT => {
                ret.push(ParserToken::Annotation);
                cur += 1;
            }
            _ => {
                if c == token::ATOM_QUOTE {
                    // Handle quoted string
                    let end_quote = line[cur + 1..].find(token::ATOM_QUOTE);
                    if let Some(end) = end_quote {
                        let token = &line[cur + 1..cur + 1 + end]; // Exclude quotes
                        ret.push(ParserToken::QuotedString(token.to_string()));
                        cur += 1 + end + 1; // Move past the quoted string

                    } else {
                        // Unterminated quote
                        return Err(ParseErrorKind::UnterminatedQuote.to_error(cur));
                    }

                } else {
                    // Handle regular token
                    let Some(first) = line.chars().nth(cur) else {
                        return Err(ParseErrorKind::UnexpectedEOL.to_error(cur));
                    };

                    let is_digit = first.is_ascii_digit() || first == '-' || first == '+';

                    let next_special = line[cur..]
                        .find(|c| token::is_special_atom(c, is_digit))
                        .unwrap_or(line.len() - cur);
                    let token = &line[cur..cur + next_special];

                    if let Ok(int_val) = token.parse::<i64>() {
                        ret.push(ParserToken::IntLiteral(int_val));
                    } else if let Ok(float_val) = token.parse::<f64>() {
                        ret.push(ParserToken::FloatLiteral(float_val));
                    } else {
                        ret.push(ParserToken::Token(token.to_string()));
                    }
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

fn parse_line(tokens: &[ParserToken], ctx: ParseLineContext) -> ParseResult<ParsedLine> {
    let mut stream = TokenStream::new(tokens);

    // special section
    if let Some(ParserToken::Token(s)) = stream.peek() && s.starts_with(token::CHAR_DOT) {
        let next = stream.next_or_err()?;
        let ParserToken::Token(s) = next else {
            return Err(ParseErrorKind::UnexpectedToken(format!("Expected an identifier after '.' for a special section, found {:?}", next)).to_error(stream.cur));
        };

        stream.expect(ParserToken::Colon)?;
        return Ok(ParsedLine::SpecialSection(s));
    }

    let is_label = tokens.len() == 2 && matches!(tokens[1], ParserToken::Colon);

    if ctx == ParseLineContext::BindingDef && !is_label {
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
                        return Err(ParseErrorKind::UnexpectedToken(format!("Expected an identifier for a binding definition, found {:?}", name)).to_error(stream.cur));
                    }

                } else {
                    binding_name = token;
                }
            }
            _ => {
                return Err(ParseErrorKind::UnexpectedToken(format!("Expected an identifier or a modifier at the start of a binding definition, found {:?}", first)).to_error(stream.cur));
            }
        }

        stream.expect_non_whitespace(ParserToken::Colon)?;

        let values = SemanticBindingDef::parse_values(&mut stream)?;
        let ret = SemanticBindingDef {
            name: binding_name,
            constant: is_constant,
            values,
        };

        return Ok(ParsedLine::BindingDef(ret));
    }

    // compiler construct
    if let Some(ParserToken::Annotation) = stream.peek() {
        stream.next_or_err()?; // consume the annotation token (@)
        let next = stream.next_non_whitespace_or_err()?;
        let ParserToken::Token(construct_name) = next else {
            return Err(ParseErrorKind::UnexpectedToken(format!("Expected a compiler construct name after '@', found {:?}", next)).to_error(stream.cur));
        };

        let construct = CompilerConstruct::from_str(&construct_name)
            .map_err(|_| ParseErrorKind::UnexpectedToken(format!("Unknown compiler construct: {}", construct_name)).to_error(stream.cur))?;

        if !stream.has_more() {
            return Ok(ParsedLine::CompilerConstruct(SemanticCompilerConstruct {
                construct,
                operands: Vec::new(),
            }));
        }

        let operands = parse_arguments(&mut stream, true)?;

        return Ok(ParsedLine::CompilerConstruct(SemanticCompilerConstruct {
            construct,
            operands,
        }));
    }

    // labels and instructions
    let first = stream.next_non_whitespace_or_err()?;
    let ParserToken::Token(first) = first else {
        return Err(ParseErrorKind::UnexpectedToken(format!("Expected an label (starts with underscore(_)) or an instruction at the start of a new line, found {first:?}")).to_error(stream.cur));
    };

    // label
    if let Some(ParserToken::Colon) = stream.peek() {
        if first.len() < 2 || !first.starts_with(token::ATOM_UNDERLINE) {
            return Err(ParseErrorKind::InvalidLabel(first).to_error(stream.cur));
        }

        return Ok(ParsedLine::Label(first[1..].to_string()));
    }

    // instruction
    let instruction = first.parse::<OpcodeMnemonic>();
    let Ok(instruction) = instruction else {
        return Err(ParseErrorKind::UnexpectedToken(format!("Expected an instruction mnemonic, found {first:?} (is '{first}' a valid instruction?)")).to_error(stream.cur));
    };

    if !stream.has_more() {
        return Ok(ParsedLine::Instruction(SemanticInstruction {
            opcode: instruction,
            operands: Vec::new(),
        }));
    }

    stream.expect(ParserToken::Whitespace)?;
    let operands = parse_arguments(&mut stream, false)?;

    Ok(ParsedLine::Instruction(SemanticInstruction {
        opcode: instruction,
        operands,
    }))
}

fn parse_arguments(stream: &mut TokenStream, allow_mnemonic_as_operand: bool) -> ParseResult<Vec<SemanticOperand>> {
    let mut operands = Vec::new();

    // semantic arg builder properties
    {
        let mut builder = SemanticArgBuilder::default();

        while stream.has_more() {
            let next = stream.next_non_whitespace_or_err()?;
            match next {
                ParserToken::Separator => {
                    let operand = builder.build()?;
                    operands.push(operand);
                    builder = SemanticArgBuilder::default();
                }
                ParserToken::Deref => {
                    if builder.deref.is_some() {
                        return Err(ParseErrorKind::UnexpectedToken("Unexpected dereference after another dereference".to_string()).to_error(stream.cur));
                    }

                    builder.deref = Some(SemanticDerefKind::Deref);
                }
                ParserToken::Hashtag => {
                    if builder.deref.is_some() {
                        return Err(ParseErrorKind::UnexpectedToken("Unexpected dereference after another dereference".to_string()).to_error(stream.cur));
                    }

                    builder.deref = Some(SemanticDerefKind::Const);
                }
                ParserToken::Token(token) => {
                    // mnenonic as operand
                    if allow_mnemonic_as_operand {
                        if let Ok(mnemonic) = OpcodeMnemonic::from_str(&token) {
                            builder.body = Some(SemanticArgBody::Mnemonic(mnemonic));
                            continue;
                        }
                    }

                    // 1. Check if is a register
                    if let Ok(register) = RegisterSpec::parse(&token) {
                        builder.body = Some(SemanticArgBody::Register(register));

                        // check offset
                        builder.offset = parse_token_offset(stream)?;
                        continue;
                    }

                    // 2. check if it is a data type
                    if let Ok(dt) = DataType::from_str(&token) {
                        let next = stream.next_non_whitespace_or_err()?;
                        match next {
                            ParserToken::IntLiteral(n) => {
                                if dt.is_floating_point() {
                                    let sem_literal = dt.as_float_literal(n as f64)?;
                                    builder.body = Some(SemanticArgBody::Literal(sem_literal.into()));
                                } else {
                                    let sem_literal = dt.as_int_literal(n)?;
                                    builder.body = Some(SemanticArgBody::Literal(sem_literal.into()));
                                }
                            }
                            ParserToken::FloatLiteral(n) => {
                                builder.body = Some(SemanticArgBody::Literal(dt.as_float_literal(n)?.into()));
                            }
                            _ => {
                                return Err(ParseErrorKind::UnexpectedToken(format!("Expected a literal value for data type {dt}, found {next:?}")).to_error(stream.cur));
                            }
                        }

                        // check offset
                        builder.offset = parse_token_offset(stream)?;
                        continue;
                    }

                    // label
                    let symbol = parse_semantic_symbol(&token, stream)?;
                    builder.body = Some(SemanticArgBody::Binding(symbol));
                    builder.offset = parse_token_offset(stream)?;
                }
                ParserToken::IntLiteral(n) => {
                    let sem_literal = DataType::Dword.as_int_literal(n)?;
                    builder.body = Some(SemanticArgBody::Literal(sem_literal.into()));
                }
                ParserToken::FloatLiteral(n) => {
                    let sem_literal = DataType::Float.as_float_literal(n)?;
                    builder.body = Some(SemanticArgBody::Literal(sem_literal.into()));
                }
                _ => {
                    return Err(ParseErrorKind::UnexpectedToken(format!("Expected a register, label, binding, or literal as operand, found {next:?}")).to_error(stream.cur));
                }
            }
        }

        operands.push(builder.build()?);
    }

    Ok(operands)
}

fn parse_token_offset(stream: &mut TokenStream) -> ParseResult<i64> {
    return if let Some(ParserToken::Offset) = stream.peek_non_whitespace() {
        stream.next_non_whitespace_or_err()?;
        let offset_token = stream.next_non_whitespace_or_err()?;
        match offset_token {
            ParserToken::IntLiteral(n) => {
                Ok(n)
            }
            _ => {
                Err(ParseErrorKind::UnexpectedToken(format!("Expected an integer literal for offset, found {offset_token:?}")).to_error(stream.cur))
            }
        }

    } else {
        Ok(0)
    }
}

fn parse_semantic_symbol(first: &str, stream: &mut TokenStream) -> ParseResult<SemanticSymbol> {
    if let Some(ParserToken::NamespaceSeparator) = stream.peek_non_whitespace() {
        stream.next_non_whitespace_or_err()?;
        let second = stream.next_non_whitespace_or_err()?;
        let ParserToken::Token(second) = second else {
            return Err(ParseErrorKind::UnexpectedToken(format!("Expected a label or binding name after namespace separator, found {second:?}")).to_error(stream.cur));
        };

        Ok(SemanticSymbol {
            name: second,
            namespace: Some(first.to_string()),
        })

    } else {
        Ok(SemanticSymbol {
            name: first.to_string(),
            namespace: None,
        })
    }
}

mod test {
    use crate::parser;
    use crate::parser::{ParseLineContext, ParserToken};
    const TEST_SCRIPT: &str = include_str!("test/test_script.f750");

    #[test]
    fn test_parse() {
        let source: Vec<&str> = TEST_SCRIPT.lines().collect();
        let res = parser::parse_source(&source);
        if let Err(e) = res {
            panic!("Parsing failed: {e}");
        }
    }

    #[test]
    fn test_tokenize() {
        let res = parser::tokenize_line("const String: \"Hello, World!\", 0, float 69.420, dword 5, qword -999 ; This is a comment");
        if let Err(e) = res {
            panic!("Tokenization failed: {:?}", e);
        }

        println!("Tokens: {:?}", res.unwrap());
    }

    #[test]
    fn test_parse_binding_def() {
        let tokens = parser::tokenize_line("const SomeVariable: 67676767, dword 721, float 69.420, \"Hello, World!\", 0").unwrap();
        let res = crate::parser::parse_line(&tokens, ParseLineContext::BindingDef);
        if let Err(e) = res {
            panic!("Parsing failed: {:?}", e);
        }

        println!("Parsed line: {:?}", res.unwrap());
    }
}