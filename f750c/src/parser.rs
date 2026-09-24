//! This module defines functions and structures for the parsing step of the F750 compiler.
//! 
//! In the F750 compiler flow, each line in a source file is first tokenized into a sequence of `Token`s, which are then parsed into semantic representations of the source code.

use std::fmt::Display;
use std::str::FromStr;
use log::{debug, trace, warn};
use thiserror::Error;
use crate::semantic::{SemanticRepr, SemanticOperand, SemanticBindingDef, SemanticInstruction, SemanticSymbol, SemanticCompilerConstruct, SemanticLiteral, SemanticDeref, SemanticDerefKind, SemanticImmediateType};
use crate::{semantic, tokenizer, util};
use crate::opcode::{CompilerConstruct, OpcodeMnemonic};
use crate::tokenizer::{Token, TokenStream, TokenizedLine};
use crate::value::{DataType, DataTypeLiteral, RegisterSpec, RegisterSpecError};

/// Represents a parsing error with additional details to locate the error in the source code.
#[derive(Debug, Error, Clone)]
pub struct ParseErrorDetails {
    pub kind: ParseError,
    pub line_count: usize,
    pub line: TokenizedLine,
}

impl Display for ParseErrorDetails {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let line_content = self.line.iter().map(|t| t.to_string()).collect::<Vec<_>>().join("");
        
        write!(f, "Parse error at line {} '{}': {}", self.line_count, line_content, self.kind)
    }
}

pub type ParseResult<T> = Result<T, ParseError>;

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
    #[error("Label dereference (&_label) is not allowed")]
    LabelDerefNotAllowed,
    #[error("Dereference (& or &const) is not valid here")]
    DerefNotAllowed,
    #[error("Offset not valid here")]
    OffsetNotAllowed,
    #[error("Constant dereference (&const) not valid here")]
    ConstantDerefNotAllowed,
    #[error("Memory offset on a non-dereferenced register or binding is not allowed")]
    NonDerefOffsetNotAllowed,
    #[error("Constant binding dereference (&const binding) with an offset is not allowed")]
    ConstantBindingDerefOffsetNotAllowed,
    #[error("Memory address dereference with a zero address is not allowed")]
    ZeroAddressNotAllowed,
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

/// Parses a source file into a vector of `SemanticRepr`s, which represent the semantic structure of the source code. 
/// 
/// This function first tokenizes each line of the source file, and then parses the tokens into semantic representations.
pub fn parse_source(source: &[TokenizedLine]) -> Result<Vec<SemanticRepr>, ParseErrorDetails> {
    let mut ctx = ParseLineContext::None;
    let mut ret = Vec::new();

    debug!("Beginning of parse step.");
    for (i, line) in source.iter().enumerate() {
        let res = parse_line(line, ctx);

        let parsed_line = match res {
            Ok(parsed_line) => parsed_line,
            Err(e) => {
                return Err(ParseErrorDetails {
                    kind: e,
                    line_count: i + 1,
                    line: line.clone(),
                });
            }
        };
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
            return Err(ParseError::UnexpectedToken(format!("Expected an identifier after '.' for a special section, found {:?}", next)));
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
                        return Err(ParseError::UnexpectedToken(format!("Expected an identifier for a binding definition, found {:?}", name)));
                    }

                } else {
                    binding_name = token;
                }
            }
            _ => {
                return Err(ParseError::UnexpectedToken(format!("Expected an identifier or a modifier at the start of a binding definition, found {:?}", first)));
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
            return Err(ParseError::UnexpectedToken(format!("Expected a compiler construct name after '@', found {:?}", next)));
        };

        let construct = CompilerConstruct::from_str(&construct_name)
            .map_err(|_| ParseError::UnexpectedToken(format!("Unknown compiler construct: {}", construct_name)))?;

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
        return Err(ParseError::UnexpectedToken(format!("Expected an label (starts with underscore(_)) or an instruction at the start of a new line, found {first:?}")));
    };

    // label
    if let Some(Token::Colon) = stream.peek() {
        if first.len() < 2 || !first.starts_with(tokenizer::ATOM_UNDERLINE) {
            return Err(ParseError::InvalidLabel(first));
        }

        return Ok(SemanticRepr::Label(first[1..].to_string()));
    }

    // instruction
    let instruction = first.parse::<OpcodeMnemonic>();
    let Ok(instruction) = instruction else {
        return Err(ParseError::UnexpectedToken(format!("Expected an instruction mnemonic, found {first:?} (is '{first}' a valid instruction?)")));
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
                    ret.push(SemanticLiteral::Typed(DataTypeLiteral::Byte(n as u8)));
                } else {
                    ret.push(SemanticLiteral::UntypedInteger(n));
                }
            }
            Token::FloatLiteral(n) => {
                ret.push(SemanticLiteral::UntypedFloating(n));
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
                                ret.push(dt.as_float_literal_typed(n as f64)?);
                            } else {
                                ret.push(dt.as_int_literal_typed(n)?);
                            }
                        }
                        Token::FloatLiteral(n) => {
                            ret.push(dt.as_float_literal_typed(n)?);
                        }
                        _ => {
                            return Err(ParseError::UnexpectedToken(format!("Expected a literal value for data type {dt}, found {next:?}")));
                        }
                    }

                } else {
                    return Err(ParseError::UnexpectedToken(format!("Expected a data type preceding literal value, found {s}")));
                }
            }
            _ => {
                return Err(ParseError::UnexpectedToken(format!("Expected a data type, literal, or comma separator in literal value, found {next:?}")))
            }
        }
    }

    Ok(ret)
}


/// Stores properties of a semantic argument during parsing, which are used to construct a [`SemanticOperand`]. This structure is used to accumulate properties of an argument as it is being parsed, and then build the final `SemanticOperand` once all properties have been collected.
#[derive(Debug, Default)]
struct OperandsBuilder {
    operands: Vec<SemanticOperand>,
    deref: bool,
    const_deref: bool,
    offset: i64,
}

impl OperandsBuilder {
    pub fn reset(&mut self) {
        self.deref = false;
        self.const_deref = false;
        self.offset = 0;
    }

    pub fn push_next_flat(&mut self, operand: SemanticOperand) -> ParseResult<()> {
        self.ensure_flat()?;
        self.push_next(operand);
        Ok(())
    }

    fn push_next(&mut self, operand: SemanticOperand) {
        self.operands.push(operand);
        self.reset();
    }

    pub fn finalize_register(&mut self, reg: RegisterSpec) -> ParseResult<()> {
        if self.deref {
            if self.const_deref {
                return Err(ParseError::ConstantRegisterDerefNotAllowed);
            }

            self.push_next(SemanticOperand::Deref(SemanticDeref {
                kind: SemanticDerefKind::Register(reg),
                offset: self.offset,
            }));

        } else {
            if self.offset != 0 {
                return Err(ParseError::NonDerefOffsetNotAllowed);
            }

            self.push_next(SemanticOperand::Register(reg));
        }

        Ok(())
    }

    pub fn finalize_binding(&mut self, symbol: SemanticSymbol) -> ParseResult<()> {
        if self.deref {
            if self.const_deref {
                if self.offset != 0 {
                    return Err(ParseError::ConstantBindingDerefOffsetNotAllowed);
                }

                self.push_next(SemanticOperand::Immediate(SemanticImmediateType::ConstDerefBinding(symbol.clone())));

            } else {
                self.push_next(SemanticOperand::Deref(SemanticDeref {
                    kind: SemanticDerefKind::Binding(symbol.clone()),
                    offset: self.offset,
                }));
            }

        } else {
            self.push_next(SemanticOperand::Immediate(SemanticImmediateType::Binding(symbol.clone(), self.offset)));

        }

        Ok(())
    }

    pub fn finalize_label(&mut self, symbol: SemanticSymbol) -> ParseResult<()> {
        if self.deref || self.const_deref {
            return Err(ParseError::LabelDerefNotAllowed);
        }

        self.push_next(SemanticOperand::Immediate(SemanticImmediateType::Label(symbol.clone(), self.offset)));

        Ok(())
    }

    pub fn finalize_int_literal(&mut self, value: i64) -> ParseResult<()> {
        if self.deref {
            if self.const_deref {
                return Err(ParseError::ConstantDerefNotAllowed);
            }

            if self.offset != 0 {
                return Err(ParseError::NonDerefOffsetNotAllowed);
            }

            if value == 0 {
                return Err(ParseError::ZeroAddressNotAllowed);
            }

            if value < 0 {
                warn!("Negative address '{value}' in dereference used, resolving to unsigned address '0x{:X}'", value as u64);
            }

            self.push_next(SemanticOperand::Deref(SemanticDeref {
                kind: SemanticDerefKind::Address(value as u64),
                offset: 0,
            }));

        } else {
            self.push_next(SemanticOperand::Immediate(SemanticImmediateType::Literal(SemanticLiteral::UntypedInteger(value))));
        }

        Ok(())
    }

    pub fn into_operands(self) -> Vec<SemanticOperand> {
        self.operands
    }

    fn ensure_flat(&mut self) -> ParseResult<()> {
        if self.deref || self.const_deref{
            return Err(ParseError::DerefNotAllowed);
        }
        if self.offset != 0 {
            return Err(ParseError::OffsetNotAllowed);
        }
        Ok(())
    }
}

/// Parses a list of operands for an instruction or compiler construct from a [`TokenStream`]. 
/// 
/// The `allow_mnemonic_as_operand` parameter determines whether mnemonics can be used as operands.
fn parse_arguments(stream: &mut TokenStream, allow_mnemonic_as_operand: bool) -> ParseResult<Vec<SemanticOperand>> {
    let mut builder = OperandsBuilder::default();

    // semantic arg builder properties
    {
        while stream.has_more() {
            let next = stream.next_non_whitespace_or_err()?;
            match next {
                Token::Separator => {
                    builder.reset();
                }
                Token::Deref => {
                    if builder.deref {
                        return Err(ParseError::UnexpectedToken("Unexpected dereference after another dereference".to_string()));
                    }

                    builder.deref = true;

                    // &const
                    if let Some(Token::Token(s)) = stream.peek_non_whitespace() && s == tokenizer::TOKEN_CONST_BINDING {
                        stream.next_non_whitespace_or_err()?;
                        builder.const_deref = true;

                        // whitespace after &const
                        stream.expect(Token::Whitespace)?;
                    }
                }
                Token::Hashtag => {
                    // engine parameter
                    let next = stream.next_non_whitespace_or_err()?;
                    let Token::Token(param_name) = next else {
                        return Err(ParseError::UnexpectedToken(format!("Expected an engine parameter name after '#', found {next:?}")));
                    };

                    // finalize
                    builder.push_next_flat(SemanticOperand::EngineParam(param_name))?;
                }
                Token::Token(token) => {
                    // mnenonic as operand
                    if allow_mnemonic_as_operand {
                        if let Ok(mnemonic) = OpcodeMnemonic::from_str(&token) {
                            builder.push_next_flat(SemanticOperand::Mnemonic(mnemonic))?;
                            continue;
                        }
                    }

                    // 1. Check if is a register
                    if let Ok(register) = RegisterSpec::parse(&token) {
                        builder.offset = parse_token_offset(stream)?;
                        builder.finalize_register(register)?;
                        continue;
                    }

                    // 2. check if it is a data type
                    // If it has a data type, it has to be a literal, and cannot be an address dereference.
                    if let Ok(dt) = DataType::from_str(&token) {
                        let next = stream.next_non_whitespace_or_err()?;
                        match next {
                            Token::IntLiteral(n) => {
                                if dt.is_floating_point() {
                                    builder.push_next_flat(SemanticOperand::Immediate(SemanticImmediateType::Literal(dt.as_float_literal_typed(n as f64)?.into())))?;

                                } else {
                                    let sem_literal = dt.as_int_literal_typed(n)?;
                                    builder.push_next_flat(SemanticOperand::Immediate(SemanticImmediateType::Literal(sem_literal.into())))?;
                                }
                            }
                            Token::FloatLiteral(n) => {
                                builder.push_next_flat(SemanticOperand::Immediate(SemanticImmediateType::Literal(dt.as_float_literal_typed(n as f64)?.into())))?;
                            }
                            _ => {
                                return Err(ParseError::UnexpectedToken(format!("Expected a literal value for data type {dt}, found {next:?}")));
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
                            return Err(ParseError::InvalidLabel(token));
                        }

                        let symbol = parse_semantic_symbol(label_name, stream)?;

                        builder.offset = parse_token_offset(stream)?;
                        builder.finalize_label(symbol)?;
                        continue;
                    }

                    // binding
                    let symbol = parse_semantic_symbol(&token, stream)?;
                    builder.offset = parse_token_offset(stream)?;
                    builder.finalize_binding(symbol)?;
                }
                Token::IntLiteral(n) => {
                    builder.finalize_int_literal(n)?;
                }
                Token::FloatLiteral(n) => {
                    let sem_literal = SemanticLiteral::UntypedFloating(n);
                    builder.push_next_flat(SemanticOperand::Immediate(SemanticImmediateType::Literal(sem_literal)))?;
                }
                _ => {
                    return Err(ParseError::UnexpectedToken(format!("Expected a register, label, binding, or literal as operand, found {next:?}")));
                }
            }
        }
    }

    Ok(builder.into_operands())
}

/// Parses a memory offset value from a [`TokenStream`]. If the end of line is reached, or no offset is specified, it returns 0.
/// 
/// This function assumes that the binding or register prior to the offset has already been parsed, and that the next token in the stream is either an `Offset` token or the end of line.
fn parse_token_offset(stream: &mut TokenStream) -> ParseResult<i64> {
    let Some(token) = stream.peek_non_whitespace() else {
        return Ok(0);
    };
    let token = token.clone();

    match token {
        Token::Offset => {
            stream.next_non_whitespace_or_err()?;
            let offset_token = stream.next_non_whitespace_or_err()?;
            match offset_token {
                Token::IntLiteral(n) => {
                    Ok(n)
                }
                _ => {
                    Err(ParseError::UnexpectedToken(format!("Expected an integer literal for offset, found {offset_token:?}")))
                }
            }
        }
        Token::IntLiteral(n) if n < 0 => {
            stream.next_non_whitespace_or_err()?;
            Ok(n)
        }
        _ => Ok(0),
    }
}

/// Parses a semantic symbol in the form of `namespace::name` or just `name` from a [`TokenStream`]. If no namespace is specified, the `namespace` field of the returned `SemanticSymbol` will be `None`.
fn parse_semantic_symbol(first: &str, stream: &mut TokenStream) -> ParseResult<SemanticSymbol> {
    if let Some(Token::NamespaceSeparator) = stream.peek_non_whitespace() {
        stream.next_non_whitespace_or_err()?;
        let second = stream.next_non_whitespace_or_err()?;
        let Token::Token(second) = second else {
            return Err(ParseError::UnexpectedToken(format!("Expected a label or binding name after namespace separator, found {second:?}")));
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