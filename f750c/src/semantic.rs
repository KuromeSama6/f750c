use bitfield_struct::bitfield;
use crate::opcode::{CompilerConstruct, OpcodeMnemonic};
use crate::parser::{ParseErrorKind, ParseResult, ParserToken, TokenStream};
use crate::value::{DataType, DataTypeLiteral, RegisterSpec};

#[derive(Debug, Clone)]
pub struct SemanticBindingDef {
    pub name: String,
    pub constant: bool,
    pub values: Vec<SemanticLiteral>,
}

impl SemanticBindingDef {
    pub fn parse_values(stream: &mut TokenStream) -> ParseResult<Vec<SemanticLiteral>> {
        let mut ret = Vec::new();
        stream.skip_whitespace();
        let mut byte_mode = false;

        while stream.has_more() {
            let next = stream.next_non_whitespace_or_err()?;
            match next {
                ParserToken::Separator => {
                    // next value
                    stream.skip_whitespace();
                }
                ParserToken::IntLiteral(n) => {
                    if byte_mode {
                        ret.push(SemanticLiteral::Byte(n as u8));
                    } else {
                        ret.push(SemanticLiteral::QWord(n as u64));
                    }
                }
                ParserToken::FloatLiteral(n) => {
                    ret.push(SemanticLiteral::Double(n));
                }
                ParserToken::QuotedString(s) => {
                    byte_mode = true;
                    ret.push(SemanticLiteral::String(s));
                }
                ParserToken::Token(s) => {
                    // try parse data type
                    if let Ok(dt) = s.parse::<DataType>() {
                        let next = stream.next_non_whitespace_or_err()?;
                        match next {
                            ParserToken::IntLiteral(n) => {
                                if dt.is_floating_point() {
                                    ret.push(dt.as_float_literal(n as f64)?);
                                } else {
                                    ret.push(dt.as_int_literal(n)?);
                                }
                            }
                            ParserToken::FloatLiteral(n) => {
                                ret.push(dt.as_float_literal(n)?);
                            }
                            _ => {
                                return Err(ParseErrorKind::UnexpectedToken(format!("Expected a literal value for data type {dt}, found {next:?}")).to_error(stream.cur));
                            }
                        }

                    } else {
                        return Err(ParseErrorKind::UnexpectedToken(format!("Expected a data type preceding literal value, found {s}")).to_error(stream.cur));
                    }
                }
                _ => {
                    return Err(ParseErrorKind::UnexpectedToken(format!("Expected a data type, literal, or comma separator in literal value, found {next:?}")).to_error(stream.cur))
                }
            }
        }
        
        Ok(ret)
    }
}

#[derive(Debug, Clone)]
pub enum SemanticLiteral {
    Byte(u8),
    Word(u16),
    DWord(u32),
    QWord(u64),
    Float(f32),
    Double(f64),
    String(String),
}

#[derive(Debug, Clone)]
pub struct SemanticInstruction {
    pub opcode: OpcodeMnemonic,
    pub operands: Vec<SemanticOperand>,
}

#[derive(Debug, Clone)]
pub struct SemanticCompilerConstruct {
    pub construct: CompilerConstruct,
    pub operands: Vec<SemanticOperand>,
}

#[derive(Debug, Clone)]
pub struct SemanticOperand {
    pub body: SemanticArgBody,
    pub deref: Option<SemanticDerefKind>,
    pub offset: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticDerefKind {
    Deref,
    Const,
}

#[derive(Debug, Clone)]
pub struct SemanticSymbol {
    pub name: String,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SemanticArgBody {
    Literal(DataTypeLiteral),
    Register(RegisterSpec),
    Label(SemanticSymbol),
    Binding(SemanticSymbol),
    Mnemonic(OpcodeMnemonic),
}