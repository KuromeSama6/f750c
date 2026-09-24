//! This module defines the semantic structures and representations used in the parsing step of the F750 compiler.

use std::fmt::{Display, Formatter};
use bitfield_struct::bitfield;
use crate::opcode::{CompilerConstruct, OpcodeMnemonic, Register};
use crate::util;
use crate::value::{DataType, DataTypeLiteral, RegisterSpec};

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
    UntypedInteger(i64),
    UntypedFloating(f64),
    Typed(DataTypeLiteral),
    String(String),
}

impl SemanticLiteral {
    pub fn to_data_type(&self) -> DataTypeLiteral {
        match self {
            SemanticLiteral::UntypedInteger(i) => DataTypeLiteral::Qword(*i as u64),
            SemanticLiteral::UntypedFloating(f) => DataTypeLiteral::Double(*f),
            SemanticLiteral::Typed(t) => *t,
            SemanticLiteral::String(_) => panic!("Cannot convert string literal to DataType"),
        }
    }
}

impl Display for SemanticLiteral {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticLiteral::UntypedInteger(i) => write!(f, "(qword){}", i),
            SemanticLiteral::UntypedFloating(fl) => write!(f, "(double){}", fl),
            SemanticLiteral::Typed(t) => write!(f, "{} {}", t.data_type(), t.value_string()),
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


/// Represents a semantic operand.
#[derive(Debug, Clone)]
pub enum SemanticOperand {
    /// Represents a value that is able to be inlined by the compiler, such as a literal, a label (which will be resolved to an address), or a constant binding (which will be resolved to its value).
    Immediate(SemanticImmediateType),
    /// Represents a register family and its width, without dereferencing.
    Register(RegisterSpec),
    /// Represents a dereference (access) of memory.
    Deref(SemanticDeref),
    /// Represents an engine parameter.
    EngineParam(String),
    /// Represents an opcode mnemonic. Only allowed in compiler constructs.
    Mnemonic(OpcodeMnemonic),
}

impl SemanticOperand {
    pub fn is_immediate(&self) -> bool {
        matches!(self, SemanticOperand::Immediate(_))
    }

    pub fn is_register(&self) -> bool {
        matches!(self, SemanticOperand::Register(_))
    }

    pub fn is_deref(&self) -> bool {
        matches!(self, SemanticOperand::Deref(_))
    }

    pub fn is_engine_param(&self) -> bool {
        matches!(self, SemanticOperand::EngineParam(_))
    }

    pub fn is_mnemonic(&self) -> bool {
        matches!(self, SemanticOperand::Mnemonic(_))
    }
}

impl Display for SemanticOperand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticOperand::Immediate(imm) => write!(f, "{}", imm),
            SemanticOperand::Register(reg) => write!(f, "{}", reg),
            SemanticOperand::Deref(deref) => write!(f, "{}", deref),
            SemanticOperand::EngineParam(param) => write!(f, "@{}", param),
            SemanticOperand::Mnemonic(mnemonic) => write!(f, "{}", mnemonic.as_ref()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SemanticImmediateType {
    /// Represents a flat literal value (i.e. '42', '0.5', etc.).
    Literal(SemanticLiteral),
    /// Represents a label (i.e. '_start', '_proc', etc.) that will be resolved to an address (that may contain an offset) at compile time.
    Label(SemanticSymbol, i64),
    /// Represents a variable binding (i.e. '&myBinding') that will be resolved to an address (that may contain an offset) at compile time.
    Binding(SemanticSymbol, i64),
    /// Represents a constant binding (i.e. '&const MyBinding') that will be inlined to its value at compile time.
    ConstDerefBinding(SemanticSymbol),
}

impl Display for SemanticImmediateType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticImmediateType::Literal(lit) => write!(f, "{}", lit),
            SemanticImmediateType::Label(label, offset) => write!(f, "_{}", label.format_offset(*offset)),
            SemanticImmediateType::Binding(binding, offset) => write!(f, "{}", binding.format_offset(*offset)),
            SemanticImmediateType::ConstDerefBinding(binding) => write!(f, "&const {}", binding),
        }
    }
}

/// Represents a dereference (access) of memory.
#[derive(Debug, Clone)]
pub struct SemanticDeref {
    pub kind: SemanticDerefKind,
    pub offset: i64,
}

impl Display for SemanticDeref {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.offset == 0 {
            write!(f, "{}", self.kind)
        } else if self.offset > 0 {
            write!(f, "{}+{}", self.kind, self.offset)
        } else {
            write!(f, "{}-{}", self.kind, -self.offset)
        }
    }
}

/// Represents the kind of dereference (access) of memory.
#[derive(Debug, Clone)]
pub enum SemanticDerefKind {
    /// The access of memory located at the address represented by a variable binding (i.e. `&myBinding`).
    Binding(SemanticSymbol),
    /// The access of memory located at the address represented by the value of a register (i.e. `&sp64`).
    Register(RegisterSpec),
    /// The access of memory located at a flat address (i.e. `&0x12345678`).
    Address(u64),
}

impl Display for SemanticDerefKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticDerefKind::Binding(binding) => write!(f, "&{}", binding),
            SemanticDerefKind::Register(reg) => write!(f, "&{}", reg),
            SemanticDerefKind::Address(addr) => write!(f, "&0x{:X}", addr),
        }
    }
}

/// Represents a semantic symbol, which consists of a name and an optional namespace. Semantic symbols are used to represent the name of labels and bindings.
#[derive(Debug, Clone)]
pub struct SemanticSymbol {
    pub name: String,
    pub namespace: Option<String>,
}

impl SemanticSymbol {
    pub fn format_offset(&self, offset: i64) -> String {
        if offset == 0 {
            format!("{}", self)
        } else if offset > 0 {
            format!("{}+{}", self, offset)
        } else {
            format!("{}-{}", self, -offset)
        }
    }
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