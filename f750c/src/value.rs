use std::str::FromStr;
use strum::{AsRefStr, Display, EnumString, FromRepr};
use thiserror::Error;
use crate::opcode::Register;
use crate::parser::{ParseError, ParseErrorKind, ParseResult};
use crate::parser::ParseErrorKind::InvalidRegisterSpec;
use crate::semantic::SemanticLiteral;

#[derive(Debug, Clone, Copy, PartialEq, AsRefStr, EnumString, Display)]
pub enum DataType {
    #[strum(serialize = "byte")]
    Byte,
    #[strum(serialize = "word")]
    Word,
    #[strum(serialize = "dword")]
    Dword,
    #[strum(serialize = "qword")]
    Qword,
    #[strum(serialize = "float")]
    Float,
    #[strum(serialize = "double")]
    Double,
}

impl DataType {
    pub fn as_int_literal(&self, value: i64) -> ParseResult<SemanticLiteral> {
        match self {
            DataType::Byte => Ok(SemanticLiteral::Byte(value as u8)),
            DataType::Word => Ok(SemanticLiteral::Word(value as u16)),
            DataType::Dword => Ok(SemanticLiteral::DWord(value as u32)),
            DataType::Qword => Ok(SemanticLiteral::QWord(value as u64)),
            _ => Err(crate::parser::ParseErrorKind::InvalidDataTypeForLiteral(value.to_string(), *self).to_error(0)),
        }
    }
    
    pub fn as_float_literal(&self, value: f64) -> ParseResult<SemanticLiteral> {
        match self {
            DataType::Float => Ok(SemanticLiteral::Float(value as f32)),
            DataType::Double => Ok(SemanticLiteral::Double(value)),
            _ => Err(crate::parser::ParseErrorKind::InvalidDataTypeForLiteral(value.to_string(), *self).to_error(0)),
        }
    }

    pub fn is_floating_point(&self) -> bool {
        matches!(self, DataType::Float | DataType::Double)
    }
}

impl From<DataTypeLiteral> for DataType {
    fn from(value: DataTypeLiteral) -> Self {
        match value {
            DataTypeLiteral::Byte(_) => DataType::Byte,
            DataTypeLiteral::Word(_) => DataType::Word,
            DataTypeLiteral::Dword(_) => DataType::Dword,
            DataTypeLiteral::Qword(_) => DataType::Qword,
            DataTypeLiteral::Float(_) => DataType::Float,
            DataTypeLiteral::Double(_) => DataType::Double,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Display)]
pub enum DataTypeLiteral {
    Byte(u8),
    Word(u16),
    Dword(u32),
    Qword(u64),
    Float(f32),
    Double(f64),
}

impl From<SemanticLiteral> for DataTypeLiteral {
    fn from(value: SemanticLiteral) -> Self {
        match value {
            SemanticLiteral::Byte(v) => DataTypeLiteral::Byte(v),
            SemanticLiteral::Word(v) => DataTypeLiteral::Word(v),
            SemanticLiteral::DWord(v) => DataTypeLiteral::Dword(v),
            SemanticLiteral::QWord(v) => DataTypeLiteral::Qword(v),
            SemanticLiteral::Float(v) => DataTypeLiteral::Float(v),
            SemanticLiteral::Double(v) => DataTypeLiteral::Double(v),
            _ => panic!("Cannot convert SemanticLiteral to DataTypeLiteral: {:?}", value)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr, EnumString, Display)]
pub enum RegisterWidth {
    #[strum(serialize = "8")]
    Byte,
    #[strum(serialize = "16")]
    Word,
    #[strum(serialize = "32")]
    Dword,
    #[strum(serialize = "64")]
    Qword,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterSpec {
    pub register: Register,
    pub width: RegisterWidth,
}


#[derive(Debug, Clone, Error)]
pub enum RegisterSpecError {
    #[error("Width not specified for register spec '{0}'. Format is <register><width>, i.e. 'a8', 'b16', 'c32', 'd64'")]
    WidthNotSpecified(String),
    #[error("No such register '{0}'")]
    NoSuchRegister(String),
    #[error("No such register width '{0}'")]
    NoSuchWidth(String),
}

impl RegisterSpec {
    pub fn parse(register_str: &str) -> Result<Self, RegisterSpecError> {
        // find the index of the first digit in the string
        let Some(digit_index) = register_str.find(|c: char| c.is_ascii_digit()) else {
            return Err(RegisterSpecError::WidthNotSpecified(register_str.to_string()));
        };

        let reg_name = &register_str[..digit_index];
        let register = Register::from_str(reg_name).map_err(|_| RegisterSpecError::NoSuchRegister(reg_name.to_string()))?;

        let width_str = &register_str[digit_index..];
        let width = RegisterWidth::from_str(width_str).map_err(|_| RegisterSpecError::NoSuchRegister(width_str.to_string()))?;

        Ok(Self {
            register,
            width,
        })
    }
}