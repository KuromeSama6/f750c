//! This module defines functions and structures for the parsing step of the F750 compiler.
//! 
//! In the F750 compiler flow, each line in a source file is first tokenized into a sequence of `Token`s, which are then parsed into semantic representations of the source code.

use std::collections::VecDeque;
use std::fmt::Display;
use std::str::FromStr;
use log::{debug, trace};
use thiserror::Error;
use crate::semantic::{SemanticRepr, SemanticOperand, SemanticOperandKind, SemanticBindingDef, SemanticInstruction, SemanticSymbol, SemanticCompilerConstruct, SemanticLiteral};
use crate::{semantic, tokenizer, util};
use crate::opcode::{CompilerConstruct, OpcodeMnemonic};
use crate::tokenizer::{Token, TokenStream, TokenizedLine};
use crate::value::{BindingDerefType, DataType, RegisterSpec, RegisterSpecError};

/// Represents a parsing error with additional details to locate the error in the source code.
#[derive(Debug, Error, Clone)]
pub struct ParseErrorDetails {
    /// The error.
    pub kind: ParseError,
    /// The current position in the token stream where the error occurred. This is an optional value, as some errors may not have a specific position associated with them.
    pub cur: Option<usize>,
}
pub type ParseResult<T> = Result<T, ParseErrorDetails>;

impl Display for ParseErrorDetails {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parse error at position {:?}: {}", self.cur, self.kind)
    }
}

impl From<ParseError> for ParseErrorDetails {
    fn from(kind: ParseError) -> Self {
        ParseErrorDetails {
            kind,
            cur: None,
        }
    }
}

/// Represents the different kinds of parsing errors that can occur during the parsing step of the F750 compiler.
#[derive(Debug, Error, Clone)]
pub enum ParseError {
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

    #[error("Constant register dereference (&const register) is not allowed")]
    ConstantRegisterDerefNotAllowed,
    #[error("Constant literal dereference (&const literal) is not allowed")]
    ConstantLiteralDerefNotAllowed,
    #[error("Literal offset is not allowed")]
    LiteralOffsetNotAllowed,
    #[error("Memory offset on a non-dereferenced register or binding is not allowed")]
    NonDerefOffsetNotAllowed,
    #[error("Constant binding dereference (&const binding) with an offset is not allowed")]
    ConstantBindingDerefOffsetNotAllowed,
}

impl ParseError {
    pub fn to_error(self, cur: usize) -> ParseErrorDetails {
        ParseErrorDetails {
            kind: self,
            cur: Some(cur),
        }
    }
}

/// Represents the context in which a line is being parsed.
/// 
/// In F750, binding definitions are only allowed in the `.data` section, which is denoted by the `ParseLineContext::BindingDef` context. This context is used to enforce this rule during parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseLineContext {
    /// The default context, where any line can be parsed.
    None,
    /// The context for parsing a binding definition, which is only allowed in the `.data` section.
    BindingDef,
}

/// Stores properties of a semantic argument during parsing, which are used to construct a [`SemanticOperand`]. This structure is used to accumulate properties of an argument as it is being parsed, and then build the final `SemanticOperand` once all properties have been collected.
#[derive(Debug, Default)]
struct SemanticArgBuilder {
    body: Option<SemanticOperandKind>,
    deref: Option<BindingDerefType>,
    offset: i64,
}

impl SemanticArgBuilder {
    pub fn build(self) -> Result<SemanticOperand, ParseError> {
        let Some(body) = self.body else {
            return Err(ParseError::UnexpectedToken("Expected an operand, found none".to_string()));
        };
        // disallow const deref of constant register
        if matches!(body, SemanticOperandKind::Register(_)) && self.deref == Some(BindingDerefType::Const) {
            return Err(ParseError::ConstantRegisterDerefNotAllowed);
        }

        // disallow const deref of constant literal
        if matches!(body, SemanticOperandKind::Literal(_)) && self.deref == Some(BindingDerefType::Const) {
            return Err(ParseError::ConstantLiteralDerefNotAllowed);
        }

        // disallow literal offset
        if matches!(body, SemanticOperandKind::Literal(_)) && self.offset != 0 {
            return Err(ParseError::LiteralOffsetNotAllowed);
        }

        // disallow offset on non-dereferenced register or binding
        if self.deref.is_none() && self.offset != 0 {
            return Err(ParseError::NonDerefOffsetNotAllowed);
        }

        // disallow const deref of binding with offset
        if matches!(body, SemanticOperandKind::Binding(_)) && self.deref == Some(BindingDerefType::Const) && self.offset != 0 {
            return Err(ParseError::ConstantBindingDerefOffsetNotAllowed);
        }

        Ok(SemanticOperand {
            kind: body,
            deref: self.deref,
            offset: self.offset,
        })
    }
}

/// Parses a source file into a vector of `SemanticRepr`s, which represent the semantic structure of the source code. 
/// 
/// This function first tokenizes each line of the source file, and then parses the tokens into semantic representations.
pub fn parse_source(source: &[TokenizedLine]) -> ParseResult<Vec<SemanticRepr>> {
    let mut ctx = ParseLineContext::None;
    let mut ret = Vec::new();

    debug!("Beginning of parse step.");
    for line in source {
        let res = parse_line(line, ctx);

        let parsed_line = res?;
        debug!("{:?}", parsed_line);

        if ctx == ParseLineContext::BindingDef && !matches!(parsed_line, SemanticRepr::BindingDef(_)) {
            ctx = ParseLineContext::None;
        }

        if ctx == ParseLineContext::None && let SemanticRepr::SpecialSection(s) = &parsed_line && s == tokenizer::SPECIAL_SECTION_LABEL_DATA {
            ctx = ParseLineContext::BindingDef;
        }
        ret.push(parsed_line);
    }

    Ok(ret)
}

/// Parses a line of tokens into a `SemanticRepr`, which represents the semantic structure of the line. The parsing behavior may vary depending on the context in which the line is being parsed (e.g., whether it is in a binding definition context).
fn parse_line(tokens: &[Token], ctx: ParseLineContext) -> ParseResult<SemanticRepr> {
    let mut stream = TokenStream::new(tokens);

    // special section
    if let Some(Token::Token(s)) = stream.peek() && s.starts_with(tokenizer::CHAR_DOT) {
        let next = stream.next_or_err()?;
        let Token::Token(s) = next else {
            return Err(ParseError::UnexpectedToken(format!("Expected an identifier after '.' for a special section, found {:?}", next)).to_error(stream.cur));
        };

        stream.expect(Token::Colon)?;
        return Ok(SemanticRepr::SpecialSection(s));
    }

    let is_label = tokens.len() == 2 && matches!(tokens[1], Token::Colon);

    if ctx == ParseLineContext::BindingDef && !is_label {
        let binding_name: String;
        let mut is_constant = false;

        let first = stream.next_non_whitespace_or_err()?;

        match first {
            Token::Token(token) => {
                if token == tokenizer::TOKEN_CONST_BINDING {
                    let name = stream.next_non_whitespace_or_err()?;
                    if let Token::Token(s) = name {
                        binding_name = s;
                        is_constant = true;

                    } else {
                        return Err(ParseError::UnexpectedToken(format!("Expected an identifier for a binding definition, found {:?}", name)).to_error(stream.cur));
                    }

                } else {
                    binding_name = token;
                }
            }
            _ => {
                return Err(ParseError::UnexpectedToken(format!("Expected an identifier or a modifier at the start of a binding definition, found {:?}", first)).to_error(stream.cur));
            }
        }

        stream.expect_non_whitespace(Token::Colon)?;

        let values = parse_binding_values(&mut stream)?;
        let ret = SemanticBindingDef {
            name: binding_name,
            constant: is_constant,
            values,
        };

        return Ok(SemanticRepr::BindingDef(ret));
    }

    // compiler construct
    if let Some(Token::Annotation) = stream.peek() {
        stream.next_or_err()?; // consume the annotation token (@)
        let next = stream.next_non_whitespace_or_err()?;
        let Token::Token(construct_name) = next else {
            return Err(ParseError::UnexpectedToken(format!("Expected a compiler construct name after '@', found {:?}", next)).to_error(stream.cur));
        };

        let construct = CompilerConstruct::from_str(&construct_name)
            .map_err(|_| ParseError::UnexpectedToken(format!("Unknown compiler construct: {}", construct_name)).to_error(stream.cur))?;

        if !stream.has_more() {
            return Ok(SemanticRepr::CompilerConstruct(SemanticCompilerConstruct {
                opcode: construct,
                operands: Vec::new(),
            }));
        }

        let operands = parse_arguments(&mut stream, true)?;

        return Ok(SemanticRepr::CompilerConstruct(SemanticCompilerConstruct {
            opcode: construct,
            operands,
        }));
    }

    // labels and instructions
    let first = stream.next_non_whitespace_or_err()?;
    let Token::Token(first) = first else {
        return Err(ParseError::UnexpectedToken(format!("Expected an label (starts with underscore(_)) or an instruction at the start of a new line, found {first:?}")).to_error(stream.cur));
    };

    // label
    if let Some(Token::Colon) = stream.peek() {
        if first.len() < 2 || !first.starts_with(tokenizer::ATOM_UNDERLINE) {
            return Err(ParseError::InvalidLabel(first).to_error(stream.cur));
        }

        return Ok(SemanticRepr::Label(first[1..].to_string()));
    }

    // instruction
    let instruction = first.parse::<OpcodeMnemonic>();
    let Ok(instruction) = instruction else {
        return Err(ParseError::UnexpectedToken(format!("Expected an instruction mnemonic, found {first:?} (is '{first}' a valid instruction?)")).to_error(stream.cur));
    };

    if !stream.has_more() {
        return Ok(SemanticRepr::Instruction(SemanticInstruction {
            opcode: instruction,
            operands: Vec::new(),
        }));
    }

    stream.expect(Token::Whitespace)?;
    let operands = parse_arguments(&mut stream, false)?;

    Ok(SemanticRepr::Instruction(SemanticInstruction {
        opcode: instruction,
        operands,
    }))
}

/// Parses a list of binding values from a [`TokenStream`]. This is used in parsing binding definitions.
fn parse_binding_values(stream: &mut TokenStream) -> ParseResult<Vec<SemanticLiteral>> {
    let mut ret = Vec::new();
    stream.skip_whitespace();
    let mut byte_mode = false;

    while stream.has_more() {
        let next = stream.next_non_whitespace_or_err()?;
        match next {
            Token::Separator => {
                // next value
                stream.skip_whitespace();
            }
            Token::IntLiteral(n) => {
                if byte_mode {
                    ret.push(SemanticLiteral::Byte(n as u8));
                } else {
                    ret.push(SemanticLiteral::QWord(n as u64));
                }
            }
            Token::FloatLiteral(n) => {
                ret.push(SemanticLiteral::Double(n));
            }
            Token::QuotedString(s) => {
                byte_mode = true;
                ret.push(SemanticLiteral::String(s));
            }
            Token::Token(s) => {
                // try parse data type
                if let Ok(dt) = s.parse::<DataType>() {
                    let next = stream.next_non_whitespace_or_err()?;
                    match next {
                        Token::IntLiteral(n) => {
                            if dt.is_floating_point() {
                                ret.push(dt.as_float_literal(n as f64)?);
                            } else {
                                ret.push(dt.as_int_literal(n)?);
                            }
                        }
                        Token::FloatLiteral(n) => {
                            ret.push(dt.as_float_literal(n)?);
                        }
                        _ => {
                            return Err(ParseError::UnexpectedToken(format!("Expected a literal value for data type {dt}, found {next:?}")).to_error(stream.cur));
                        }
                    }

                } else {
                    return Err(ParseError::UnexpectedToken(format!("Expected a data type preceding literal value, found {s}")).to_error(stream.cur));
                }
            }
            _ => {
                return Err(ParseError::UnexpectedToken(format!("Expected a data type, literal, or comma separator in literal value, found {next:?}")).to_error(stream.cur))
            }
        }
    }

    Ok(ret)
}

/// Parses a list of operands for an instruction or compiler construct from a [`TokenStream`]. 
/// 
/// The `allow_mnemonic_as_operand` parameter determines whether mnemonics can be used as operands.
fn parse_arguments(stream: &mut TokenStream, allow_mnemonic_as_operand: bool) -> ParseResult<Vec<SemanticOperand>> {
    let mut operands = Vec::new();

    // semantic arg builder properties
    {
        let mut builder = SemanticArgBuilder::default();

        while stream.has_more() {
            let next = stream.next_non_whitespace_or_err()?;
            match next {
                Token::Separator => {
                    let operand = builder.build()?;
                    operands.push(operand);
                    builder = SemanticArgBuilder::default();
                }
                Token::Deref => {
                    if builder.deref.is_some() {
                        return Err(ParseError::UnexpectedToken("Unexpected dereference after another dereference".to_string()).to_error(stream.cur));
                    }

                    if let Some(Token::Token(s)) = stream.peek_non_whitespace() && s == tokenizer::TOKEN_CONST_BINDING {
                        stream.next_non_whitespace_or_err()?;
                        builder.deref = Some(BindingDerefType::Const);

                        // whitespace after &const
                        stream.expect(Token::Whitespace)?;

                    } else {
                        builder.deref = Some(BindingDerefType::Dynamic);
                    }
                }
                Token::Hashtag => {
                    // engine parameter
                    let next = stream.next_non_whitespace_or_err()?;
                    let Token::Token(param_name) = next else {
                        return Err(ParseError::UnexpectedToken(format!("Expected an engine parameter name after '#', found {next:?}")).to_error(stream.cur));
                    };

                    builder.body = Some(SemanticOperandKind::EngineParam(param_name));
                }
                Token::Token(token) => {
                    // mnenonic as operand
                    if allow_mnemonic_as_operand {
                        if let Ok(mnemonic) = OpcodeMnemonic::from_str(&token) {
                            builder.body = Some(SemanticOperandKind::Mnemonic(mnemonic));
                            continue;
                        }
                    }

                    // 1. Check if is a register
                    if let Ok(register) = RegisterSpec::parse(&token) {
                        builder.body = Some(SemanticOperandKind::Register(register));

                        // check offset
                        builder.offset = parse_token_offset(stream)?;
                        continue;
                    }

                    // 2. check if it is a data type
                    if let Ok(dt) = DataType::from_str(&token) {
                        let next = stream.next_non_whitespace_or_err()?;
                        match next {
                            Token::IntLiteral(n) => {
                                if dt.is_floating_point() {
                                    let sem_literal = dt.as_float_literal(n as f64)?;
                                    builder.body = Some(SemanticOperandKind::Literal(sem_literal.into()));
                                } else {
                                    let sem_literal = dt.as_int_literal(n)?;
                                    builder.body = Some(SemanticOperandKind::Literal(sem_literal.into()));
                                }
                            }
                            Token::FloatLiteral(n) => {
                                builder.body = Some(SemanticOperandKind::Literal(dt.as_float_literal(n)?.into()));
                            }
                            _ => {
                                return Err(ParseError::UnexpectedToken(format!("Expected a literal value for data type {dt}, found {next:?}")).to_error(stream.cur));
                            }
                        }

                        // check offset
                        builder.offset = parse_token_offset(stream)?;
                        continue;
                    }

                    // 3. Check if is a label
                    if token.starts_with(tokenizer::ATOM_UNDERLINE) {
                        let label_name = &token[1..];
                        if label_name.is_empty() {
                            return Err(ParseError::InvalidLabel(token).to_error(stream.cur));
                        }

                        let symbol = parse_semantic_symbol(label_name, stream)?;
                        builder.body = Some(SemanticOperandKind::Label(symbol));
                        builder.offset = parse_token_offset(stream)?;
                        continue;
                    }

                    // label
                    let symbol = parse_semantic_symbol(&token, stream)?;
                    builder.body = Some(SemanticOperandKind::Binding(symbol));
                    builder.offset = parse_token_offset(stream)?;
                }
                Token::IntLiteral(n) => {
                    let sem_literal = DataType::Dword.as_int_literal(n)?;
                    builder.body = Some(SemanticOperandKind::Literal(sem_literal.into()));
                }
                Token::FloatLiteral(n) => {
                    let sem_literal = DataType::Float.as_float_literal(n)?;
                    builder.body = Some(SemanticOperandKind::Literal(sem_literal.into()));
                }
                _ => {
                    return Err(ParseError::UnexpectedToken(format!("Expected a register, label, binding, or literal as operand, found {next:?}")).to_error(stream.cur));
                }
            }
        }

        operands.push(builder.build()?);
    }

    Ok(operands)
}

/// Parses a memory offset value from a [`TokenStream`]. If the end of line is reached, or no offset is specified, it returns 0.
/// 
/// This function assumes that the binding or register prior to the offset has already been parsed, and that the next token in the stream is either an `Offset` token or the end of line.
fn parse_token_offset(stream: &mut TokenStream) -> ParseResult<i64> {
    return if let Some(Token::Offset) = stream.peek_non_whitespace() {
        stream.next_non_whitespace_or_err()?;
        let offset_token = stream.next_non_whitespace_or_err()?;
        match offset_token {
            Token::IntLiteral(n) => {
                Ok(n)
            }
            _ => {
                Err(ParseError::UnexpectedToken(format!("Expected an integer literal for offset, found {offset_token:?}")).to_error(stream.cur))
            }
        }

    } else {
        Ok(0)
    }
}

/// Parses a semantic symbol in the form of `namespace::name` or just `name` from a [`TokenStream`]. If no namespace is specified, the `namespace` field of the returned `SemanticSymbol` will be `None`.
fn parse_semantic_symbol(first: &str, stream: &mut TokenStream) -> ParseResult<SemanticSymbol> {
    if let Some(Token::NamespaceSeparator) = stream.peek_non_whitespace() {
        stream.next_non_whitespace_or_err()?;
        let second = stream.next_non_whitespace_or_err()?;
        let Token::Token(second) = second else {
            return Err(ParseError::UnexpectedToken(format!("Expected a label or binding name after namespace separator, found {second:?}")).to_error(stream.cur));
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