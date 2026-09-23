//! This module defines the semantic structures and representations used in the parsing step of the F750 compiler.

use bitfield_struct::bitfield;
use crate::opcode::{CompilerConstruct, OpcodeMnemonic};
use crate::parser::{ParseError, ParseResult};
use crate::value::{DataType, DataTypeLiteral, RegisterSpec};

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

/// Represents a semantic instruction, which consists of an opcode mnemonic and a list of operands.
#[derive(Debug, Clone)]
pub struct SemanticInstruction {
    pub opcode: OpcodeMnemonic,
    pub operands: Vec<SemanticOperand>,
}

/// Represents a semantic compiler construct, which consists of a compiler construct and a list of operands.
#[derive(Debug, Clone)]
pub struct SemanticCompilerConstruct {
    pub construct: CompilerConstruct,
    pub operands: Vec<SemanticOperand>,
}

/// Represents a semantic operand, which consists of an argument body, as well as additional properties.
#[derive(Debug, Clone)]
pub struct SemanticOperand {
    /// The body of the operand.
    pub body: SemanticArgBody,
    /// The kind of dereference applied to this operand, if any.
    pub deref: Option<SemanticDerefKind>,
    /// The memory offset applied to this operand, or zero if no offset is specified.
    pub offset: i64,
}

/// Represents the body of a semantic argument.
#[derive(Debug, Clone)]
pub enum SemanticArgBody {
    /// Represents a literal value.
    Literal(DataTypeLiteral),
    /// Represents a specific register.
    Register(RegisterSpec),
    /// Represents a label.
    Label(SemanticSymbol),
    /// Represents a binding.
    Binding(SemanticSymbol),
    /// Represents an opcode mnemonic. This is only allowed in operands of a compiler construct.
    Mnemonic(OpcodeMnemonic),
}

/// Represents the possible kinds of dereference that can be applied to an operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticDerefKind {
    /// Represents dereferencing of a memory address (i.e. `&register`, `&binding`, etc.).
    Deref,
    /// Represents dereferencing of a constant value (i.e. `#const_binding`, `#engine_enum`, etc.). Constant dereferences may be optimized and inlined by the compiler at compile time.
    Const,
}

/// Represents a semantic symbol, which consists of a name and an optional namespace. Semantic symbols are used to represent the name of labels and bindings.
#[derive(Debug, Clone)]
pub struct SemanticSymbol {
    pub name: String,
    pub namespace: Option<String>,
}