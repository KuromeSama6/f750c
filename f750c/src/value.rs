//! This module defines the data types and commonly used compound types in the F750 language.

use std::fmt::{Display, Formatter};
use std::str::FromStr;
use strum::{AsRefStr, Display, EnumString, FromRepr};
use thiserror::Error;
use crate::opcode::Register;
use crate::parser::{ParseErrorDetails, ParseError, ParseResult};
use crate::parser::ParseError::InvalidRegisterSpec;
use crate::semantic::SemanticLiteral;

/// Represents the data types in the F750 language.
/// 
/// All integer data types are unsigned in their representation, but can be used to represent signed values in two's complement form.
#[derive(Debug, Clone, Copy, PartialEq, AsRefStr, EnumString, Display)]
pub enum DataType {
    /// Represents an 8-bit unsigned integer.
    #[strum(serialize = "byte")]
    Byte,
    /// Represents a 16-bit unsigned integer.
    #[strum(serialize = "word")]
    Word,
    /// Represents a 32-bit unsigned integer.
    #[strum(serialize = "dword")]
    Dword,
    /// Represents a 64-bit unsigned integer.
    #[strum(serialize = "qword")]
    Qword,
    /// Represents a 32-bit IEEE-754 floating point number.
    #[strum(serialize = "float")]
    Float,
    /// Represents a 64-bit IEEE-754 floating point number.
    #[strum(serialize = "double")]
    Double,
}

impl DataType {
    /// Converts an integer value to a [`SemanticLiteral`] of the appropriate type based on the [`DataType`].
    /// 
    /// This function will return an error if the data type is not an integer type (i.e., `Byte`, `Word`, `Dword`, or `Qword`).
    pub fn as_int_literal_typed(&self, value: i64) -> ParseResult<SemanticLiteral> {
        let lit = match self {
            DataType::Byte => DataTypeLiteral::Byte(value as u8),
            DataType::Word => DataTypeLiteral::Word(value as u16),
            DataType::Dword => DataTypeLiteral::Dword(value as u32),
            DataType::Qword => DataTypeLiteral::Qword(value as u64),
            _ => return Err(ParseError::InvalidDataTypeForLiteral(value.to_string(), *self)),
        };

        Ok(SemanticLiteral::Typed(lit))
    }

    /// Converts a floating point value to a [`SemanticLiteral`] of the appropriate type based on the [`DataType`].
    ///
    /// This function will return an error if the data type is not a floating point type (i.e., `Float` or `Double`).
    pub fn as_float_literal_typed(&self, value: f64) -> ParseResult<SemanticLiteral> {
        let lit = match self {
            DataType::Float => DataTypeLiteral::Float(value as f32),
            DataType::Double => DataTypeLiteral::Double(value),
            _ => return Err(ParseError::InvalidDataTypeForLiteral(value.to_string(), *self)),
        };

        Ok(SemanticLiteral::Typed(lit))
    }

    /// Returns whether this data type is a floating point type (i.e., `Float` or `Double`).
    pub fn is_floating_point(&self) -> bool {
        matches!(self, DataType::Float | DataType::Double)
    }

    pub fn size(&self) -> usize {
        match self {
            DataType::Byte => 1,
            DataType::Word => 2,
            DataType::Dword => 4,
            DataType::Qword => 8,
            DataType::Float => 4,
            DataType::Double => 8,
        }
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

/// Represents a literal value of a specific data type in the F750 language.
/// 
/// Refer to [`DataType`] for the supported data types.
#[derive(Debug, Clone, Copy, PartialEq, Display)]
pub enum DataTypeLiteral {
    Byte(u8),
    Word(u16),
    Dword(u32),
    Qword(u64),
    Float(f32),
    Double(f64),
}

impl DataTypeLiteral {
    pub fn size(&self) -> usize {
        self.data_type().size()
    }

    pub fn data_type(&self) -> DataType {
        (*self).into()
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            DataTypeLiteral::Byte(v) => Some(*v as i64),
            DataTypeLiteral::Word(v) => Some(*v as i64),
            DataTypeLiteral::Dword(v) => Some(*v as i64),
            DataTypeLiteral::Qword(v) => Some(*v as i64),
            _ => None,
        }
    }

    pub fn value_string(&self) -> String {
        match self {
            DataTypeLiteral::Byte(v) => v.to_string(),
            DataTypeLiteral::Word(v) => v.to_string(),
            DataTypeLiteral::Dword(v) => v.to_string(),
            DataTypeLiteral::Qword(v) => v.to_string(),
            DataTypeLiteral::Float(v) => v.to_string(),
            DataTypeLiteral::Double(v) => v.to_string(),
        }
    }
}

impl From<SemanticLiteral> for DataTypeLiteral {
    fn from(value: SemanticLiteral) -> Self {
        match value {
            SemanticLiteral::Typed(lit) => lit,
            SemanticLiteral::UntypedInteger(v) => DataTypeLiteral::Qword(v as u64),
            SemanticLiteral::UntypedFloating(v) => DataTypeLiteral::Double(v),
            _ => panic!("Cannot convert SemanticLiteral to DataTypeLiteral: {:?}", value)
        }
    }
}

/// Represents the width of a register (or a view into an underlying register) in the F750 language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr, EnumString, Display)]
pub enum RegisterWidth {
    /// Represents an 8-bit view into a register.
    #[strum(serialize = "8")]
    Byte,
    /// Represents a 16-bit view into a register.
    #[strum(serialize = "16")]
    Word,
    /// Represents a 32-bit view into a register.
    #[strum(serialize = "32")]
    Dword,
    /// Represents a 64-bit view into a register.
    #[strum(serialize = "64")]
    Qword,
}

/// Represents a register specification, which includes a register and its width (view).
/// 
/// For instance, the register mnemonic `a64` (equivalent to `rax` in x86-64) would be represented as a `RegisterSpec` with a register family of [`Register::A`] and a width of [`RegisterWidth::Qword`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterSpec {
    /// The register family (e.g., `a`, `b`, `c`, `d`, etc.).
    pub register: Register,
    /// The width (view) of the register (e.g., 8-bit, 16-bit, 32-bit, or 64-bit).
    pub width: RegisterWidth,
}

impl RegisterSpec {
    pub fn new(register: Register, width: RegisterWidth) -> Self {
        Self { register, width }
    }

    pub fn qword(register: Register) -> Self {
        Self::new(register, RegisterWidth::Qword)
    }

    pub fn dword(register: Register) -> Self {
        Self::new(register, RegisterWidth::Dword)
    }

    pub fn word(register: Register) -> Self {
        Self::new(register, RegisterWidth::Word)
    }

    pub fn byte(register: Register) -> Self {
        Self::new(register, RegisterWidth::Byte)
    }
}

impl Display for RegisterSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.register.as_ref(), self.width.as_ref())
    }
}

/// Represents errors that can occur when parsing a register specification string.
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
        let width = RegisterWidth::from_str(width_str).map_err(|_| RegisterSpecError::NoSuchWidth(width_str.to_string()))?;

        Ok(Self {
            register,
            width,
        })
    }
}