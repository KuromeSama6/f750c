//! This module defines the semantic structures and representations used in the parsing step of the F750 compiler.

use std::fmt::{write, Debug, Display, Formatter};
use bitfield_struct::bitfield;
use crate::bytecode::{BytecodeSerialize, BytecodeStream};
use crate::opcode::{CompilerConstruct, OpcodeMnemonic, Register};
use crate::util;
use crate::value::{DataType, DataTypeLiteral, RegisterSpec, SymbolName};

/// Semantic representation of a parsed line in the F750 source code.
#[derive(Debug, Clone)]
pub enum SemanticRepr {
    /// Represents the start of a special section, such as `.data`.
    SpecialSection(String),
    /// Represents a binding definition.
    BindingDef(SemanticBindingDef),
    /// Represents a label.
    Label(SemanticSymbol),
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
            SemanticRepr::Label(label) => {
                if label.external {
                    write!(f, "(extern) _{}:", label.name)

                } else {
                    write!(f, "_{}:", label.name)
                }
            },
            SemanticRepr::Instruction(instr) => write!(f, "{}", instr),
            SemanticRepr::CompilerConstruct(construct) => write!(f, "{}", construct),
        }
    }
}

pub type SemanticSource = [SemanticRepr];

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

impl SemanticBindingDef {
    pub fn total_size(&self) -> usize {
        self.values.iter()
            .map(|c| c.size())
            .sum()
    }
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

impl BytecodeSerialize for SemanticBindingDef {
    fn serialize(&self, stream: &mut BytecodeStream) {
        for value in &self.values {
            value.serialize(stream);
        }
    }
}

/// Represents a semantic literal value.
///
/// Note that semantic literals are different from [`DataType`]s in that they represent actual literal values written in the source code, which allows string literals to be present.
#[derive(Debug, Clone)]
pub enum
SemanticLiteral {
    UntypedInteger(i64),
    UntypedFloating(f64),
    Typed(DataTypeLiteral),
    String(String),
}
impl SemanticLiteral {
    pub fn size(&self) -> usize {
        match self {
            SemanticLiteral::UntypedInteger(_) => DataType::Qword.size(),
            SemanticLiteral::UntypedFloating(_) => DataType::Double.size(),
            SemanticLiteral::Typed(t) => t.size(),
            SemanticLiteral::String(s) => s.len(),
        }
    }

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
            SemanticLiteral::UntypedInteger(i) => write!(f, "(qword) {}", i),
            SemanticLiteral::UntypedFloating(fl) => write!(f, "(double) {}", fl),
            SemanticLiteral::Typed(t) => write!(f, "{} {}", t.data_type(), t.value_string()),
            SemanticLiteral::String(s) => write!(f, "\"{}\"", s),
        }
    }
}

impl BytecodeSerialize for SemanticLiteral {
    fn serialize(&self, stream: &mut BytecodeStream) {
        match self {
            SemanticLiteral::UntypedInteger(i) => {
                let bytes = (*i as u64).to_be_bytes();
                stream.write_bytes(&bytes);
            }
            SemanticLiteral::UntypedFloating(f) => {
                let bytes = (*f).to_be_bytes();
                stream.write_bytes(&bytes);
            }
            SemanticLiteral::Typed(t) => {
                t.serialize(stream);
            }
            SemanticLiteral::String(s) => {
                stream.write_bytes(s.as_bytes());
            }
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
    
    pub fn is_immediate_or_register(&self) -> bool {
        self.is_immediate() || self.is_register()
    }

    pub fn get_external_symbol(&self) -> Option<ExternalSymbolUsage> {
        let (symbol, is_deref): (SemanticSymbol, bool);
        match self {
            SemanticOperand::Immediate(SemanticImmediateType::Label(s, _)) => {
                symbol = s.clone();
                is_deref = false;
            },
            SemanticOperand::Immediate(SemanticImmediateType::Binding(s, _)) => {
                symbol = s.clone();
                is_deref = false;
            },
            SemanticOperand::Deref(SemanticDeref { kind: SemanticDerefKind::Binding(s), .. }) => {
                symbol = s.clone();
                is_deref = true;
            },
            _ => return None,
        };

        if !symbol.external {
            return None;
        }

        Some(ExternalSymbolUsage::new(symbol.name.clone(), is_deref))
    }
}

impl Display for SemanticOperand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticOperand::Immediate(imm) => write!(f, "{}", imm),
            SemanticOperand::Register(reg) => write!(f, "{}", reg),
            SemanticOperand::Deref(deref) => write!(f, "{}", deref),
            SemanticOperand::EngineParam(param) => write!(f, "#{}", param),
            SemanticOperand::Mnemonic(mnemonic) => write!(f, "{}", mnemonic.as_ref()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExternalSymbolUsage {
    pub name: SymbolName,
    pub is_deref: bool,
}

impl ExternalSymbolUsage {
    pub fn new(name: SymbolName, is_deref: bool) -> Self {
        Self {
            name,
            is_deref,
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

impl SemanticImmediateType {
    pub fn from_u64_untyped(value: u64) -> Self {
        SemanticImmediateType::Literal(SemanticLiteral::UntypedInteger(value as i64))
    }
}

impl Display for SemanticImmediateType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticImmediateType::Literal(lit) => write!(f, "{}", lit),
            SemanticImmediateType::Label(label, offset) => {
                if label.external {
                    write!(f, "(extern) _{}", label.name.format_offset(*offset))

                } else {
                    write!(f, "_{}", label.name.format_offset(*offset))
                }
            },
            SemanticImmediateType::Binding(binding, offset) => write!(f, "{}", binding.name.format_offset(*offset)),
            SemanticImmediateType::ConstDerefBinding(binding) => {
                if binding.external {
                    write!(f, "(extern) ")?
                }

                write!(f, "&const {}", binding.name)
            },
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
            SemanticDerefKind::Binding(binding) => {
                if binding.external {
                    write!(f, "(extern) &{}", binding.name)
                } else {
                    write!(f, "&{}", binding.name)
                }
            },
            SemanticDerefKind::Register(reg) => write!(f, "&{}", reg),
            SemanticDerefKind::Address(addr) => write!(f, "&0x{:x}", addr),
        }
    }
}

/// Represents a semantic symbol, which consists of a name and an optional namespace. Semantic symbols are used to represent the name of labels and bindings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SemanticSymbol {
    pub name: SymbolName,
    pub external: bool,
}

impl SemanticSymbol {
    pub fn new(name: SymbolName, external: bool) -> Self {
        Self {
            name,
            external,
        }
    }
}

impl From<String> for SemanticSymbol {
    fn from(name: String) -> Self {
        Self {
            name: SymbolName::new(&name, None),
            external: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SemanticReprStream {
    lines: Vec<SemanticRepr>,
    cur: usize,
}

impl SemanticReprStream {
    pub fn from(lines: &SemanticSource) -> Self {
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