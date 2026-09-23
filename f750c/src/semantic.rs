//! This module defines the semantic structures and representations used in the parsing step of the F750 compiler.

use std::fmt::{Display, Formatter};
use bitfield_struct::bitfield;
use crate::opcode::{CompilerConstruct, OpcodeMnemonic};
use crate::parser::{ParseError, ParseResult};
use crate::value::{BindingDerefType, DataType, DataTypeLiteral, RegisterSpec};

/// Semantic representation of a parsed line in the F750 source code.
#[derive(Debug, Clone)]
pub enum SemanticRepr {
    /// Represents the start of a special section, such as `.data`.
    SpecialSection(String),
    /// Represents a binding definition.
    BindingDef(SemanticBindingDef),
    /// Represents a label.
    Label(String),
    /// Represents an instruction.
    Instruction(SemanticInstruction),
    /// Represents a compiler construct.
    CompilerConstruct(SemanticCompilerConstruct),
}

impl Display for SemanticRepr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticRepr::SpecialSection(section) => write!(f, "{}", section),
            SemanticRepr::BindingDef(binding) => write!(f, "{binding}"),
            SemanticRepr::Label(label) => write!(f, "_{}:", label),
            SemanticRepr::Instruction(instr) => write!(f, "{}", instr),
            SemanticRepr::CompilerConstruct(construct) => write!(f, "{}", construct),
        }
    }
}

/// Represents the semantic definition of a binding.
#[derive(Debug, Clone)]
pub struct SemanticBindingDef {
    /// The name of the binding.
    pub name: String,
    /// Whether this binding is a **constant binding**. The F750 compiler disallows writing to constant bindings at compile time.
    pub constant: bool,
    /// The values associated with this binding. When compiled to bytecode, all values are packed side-by-side in the order they are defined.
    /// 
    /// For instance, the binding values `"Hello, World!", 0` would be represented as a sequence of bytes (representing "Hello, World!"), followed by a single byte `0x00`.
    pub values: Vec<SemanticLiteral>,
}

impl Display for SemanticBindingDef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.constant {
            write!(f, "const ")?;
        }

        write!(f, "{}: ", self.name)?;

        for (i, value) in self.values.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", value)?;
        }

        Ok(())
    }
}

/// Represents a semantic literal value.
///
/// Note that semantic literals are different from [`DataType`]s in that they represent actual literal values written in the source code, which allows string literals to be present.
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

impl Display for SemanticLiteral {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticLiteral::Byte(b) => write!(f, "{}", b),
            SemanticLiteral::Word(w) => write!(f, "{}", w),
            SemanticLiteral::DWord(d) => write!(f, "{}", d),
            SemanticLiteral::QWord(q) => write!(f, "{}", q),
            SemanticLiteral::Float(fl) => write!(f, "{}", fl),
            SemanticLiteral::Double(dbl) => write!(f, "{}", dbl),
            SemanticLiteral::String(s) => write!(f, "\"{}\"", s),
        }
    }
}

/// Represents a semantic instruction, which consists of an opcode mnemonic and a list of operands.
#[derive(Debug, Clone)]
pub struct SemanticInstruction {
    pub opcode: OpcodeMnemonic,
    pub operands: Vec<SemanticOperand>,
}

impl Display for SemanticInstruction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ", self.opcode.as_ref())?;

        for (i, operand) in self.operands.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", operand)?;
        }

        Ok(())
    }
}


/// Represents a semantic compiler construct, which consists of a compiler construct and a list of operands.
#[derive(Debug, Clone)]
pub struct SemanticCompilerConstruct {
    pub opcode: CompilerConstruct,
    pub operands: Vec<SemanticOperand>,
}

impl Display for SemanticCompilerConstruct {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{} ", self.opcode.as_ref())?;

        for (i, operand) in self.operands.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", operand)?;
        }

        Ok(())
    }
}

/// Represents a semantic operand, which consists of an argument body, as well as additional properties.
#[derive(Debug, Clone)]
pub struct SemanticOperand {
    /// The body of the operand.
    pub kind: SemanticOperandKind,
    /// Whether this operand is dereferenced.
    pub deref: Option<BindingDerefType>,
    /// The memory offset applied to this operand, or zero if no offset is specified.
    pub offset: i64,
}

impl SemanticOperand {
    pub fn with_body(body: SemanticOperandKind) -> Self {
        Self {
            kind: body,
            deref: None,
            offset: 0,
        }
    }

    pub fn is_flat(&self) -> bool {
        self.deref.is_none() && self.offset == 0
    }

    pub fn has_offset(&self) -> bool {
        self.offset != 0
    }

    pub fn is_deref(&self) -> bool {
        self.deref.is_some()
    }

    pub fn is_dynamic_deref(&self) -> bool {
        matches!(self.deref, Some(BindingDerefType::Dynamic))
    }

    pub fn is_const_deref(&self) -> bool {
        matches!(self.deref, Some(BindingDerefType::Const))
    }

    pub fn is_register(&self) -> bool {
        matches!(self.kind, SemanticOperandKind::Register(_))
    }

    pub fn is_immediate(&self) -> bool {
        match self.kind {
            SemanticOperandKind::Literal(_) if self.is_flat() => true,
            SemanticOperandKind::Binding(_) if self.is_const_deref() => true,
            _ => false,
        }
    }
}

impl Display for SemanticOperand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.deref {
            Some(BindingDerefType::Dynamic) => write!(f, "&")?,
            Some(BindingDerefType::Const) => write!(f, "&const ")?,
            None => {}
        }

        write!(f, "{}", self.kind)?;

        if self.offset > 0 {
            write!(f, "+{}", self.offset)?;
        } else if self.offset < 0 {
            write!(f, "+ {}", self.offset)?;
        }

        Ok(())
    }
}

/// Represents the body of a semantic argument.
#[derive(Debug, Clone)]
pub enum SemanticOperandKind {
    /// Represents a literal value.
    Literal(DataTypeLiteral),
    /// Represents a specific register.
    Register(RegisterSpec),
    /// Represents a label.
    Label(SemanticSymbol),
    /// Represents a binding.
    Binding(SemanticSymbol),
    /// Represents an engine parameter.
    EngineParam(String),
    /// Represents an opcode mnemonic. This is only allowed in operands of a compiler construct.
    Mnemonic(OpcodeMnemonic),
}

impl Display for SemanticOperandKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticOperandKind::Literal(lit) => write!(f, "{}", lit.value_string()),
            SemanticOperandKind::Register(reg) => write!(f, "{reg}"),
            SemanticOperandKind::Label(sym) => write!(f, "_{sym}"),
            SemanticOperandKind::Binding(sym) => write!(f, "{sym}"),
            SemanticOperandKind::EngineParam(param) => write!(f, "#{param}"),
            SemanticOperandKind::Mnemonic(mnemonic) => write!(f, "{}", mnemonic.as_ref()),
        }
    }
}

/// Represents a semantic symbol, which consists of a name and an optional namespace. Semantic symbols are used to represent the name of labels and bindings.
#[derive(Debug, Clone)]
pub struct SemanticSymbol {
    pub name: String,
    pub namespace: Option<String>,
}

impl Display for SemanticSymbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(ns) = &self.namespace {
            write!(f, "{}::{}", ns, self.name)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

#[derive(Debug, Clone)]
pub struct SemanticReprStream {
    lines: Vec<SemanticRepr>,
    cur: usize,
}

impl SemanticReprStream {
    pub fn from(lines: &[SemanticRepr]) -> Self {
        Self {
            lines: lines.to_vec(),
            cur: 0,
        }
    }

    pub fn into_inner(self) -> Vec<SemanticRepr> {
        self.lines
    }

    pub fn has_more(&self) -> bool {
        self.cur < self.lines.len()
    }

    pub fn peek(&self) -> Option<&SemanticRepr> {
        self.lines.get(self.cur)
    }
}