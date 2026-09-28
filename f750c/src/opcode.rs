//! This module defines opcodes, mnemonics, registers, and other compile-time identifiers for F750. 
//! These identifiers are not semantic - they are used beyond the parsing stage.

use strum::{AsRefStr, Display, EnumString};
use crate::bytecode::{BytecodeSerialize, BytecodeSerializeError, BytecodeSerializeResult, BytecodeStream};
use crate::compiler::{BindingTable, EngCallTable, LabelTable};
use crate::semantic::SemanticSymbol;
use crate::value::{DataTypeLiteral, DerefType, RegisterSpec, SymbolName, SymbolRef};

/// Represents the opcodes for the F750 virtual machine. Each opcode is represented by a unique u8 value.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    EngCall = 0x01,
    Nop = 0x02,
    CallReg = 0x03,
    CallImm = 0x04,
    Ret = 0x05,
    StackAllocImm = 0x06,
    StackAllocReg = 0x07,
    StackFree = 0x08,
    Exit = 0x09,
    RetFree = 0x0A,
    HAllocImm = 0x0B,
    HAllocReg = 0x0C,
    HFreeImm = 0x0D,
    HFreeReg = 0x0E,
    CallLabel = 0x0F,

    // Move
    MovRegImm = 0x20,
    MovRegMem = 0x21,
    MovMemImm = 0x22,
    MovMemReg = 0x23,
    MovRegReg = 0x24,
    MovRegEngineParam = 0x25,
    MovEngineParamReg = 0x26,
    MovRegLabel = 0x27,

    // Comparison
    CmpRegReg = 0x30,
    CmpRegImm = 0x31,

    // Arithmetic
    AddRegReg = 0x40,
    AddRegImm = 0x41,
    SubRegReg = 0x42,
    SubRegImm = 0x43,
    MulRegReg = 0x44,
    MulRegImm = 0x45,
    DivRegReg = 0x46,
    DivRegImm = 0x47,
    ModRegReg = 0x48,
    ModRegImm = 0x49,

    // Stack
    PushReg = 0x50,
    PushImm = 0x51,
    PopReg = 0x52,
    PopMem = 0x53,

    // Jump
    JmpReg = 0xB0,
    JmpImm = 0xB1,
    JmpLabel = 0xB2,
}

#[derive(Debug, Clone)]
pub enum OpcodePayload {
    EngCall,
    Nop,
    CallReg(RegisterSpec),
    CallImm(DataTypeLiteral),
    Ret,
    StackAllocImm(DataTypeLiteral),
    StackAllocReg(RegisterSpec),
    StackFree,
    Exit,
    RetFree,
    HAllocImm(DataTypeLiteral),
    HAllocReg(RegisterSpec),
    HFreeImm(DataTypeLiteral),
    HFreeReg(RegisterSpec),
    CallLabel(SymbolRef),

    // Move
    MovRegImm(RegisterSpec, DataTypeLiteral),
    MovRegMem(RegisterSpec, DerefType),
    MovMemImm(DerefType, DataTypeLiteral),
    MovMemReg(DerefType, RegisterSpec),
    MovRegReg(RegisterSpec, RegisterSpec),
    MovRegEngineParam(RegisterSpec, String),
    MovEngineParamReg(String, RegisterSpec),
    MovRegLabel(RegisterSpec, SymbolRef),

    // Comparison
    CmpRegReg(RegisterSpec, RegisterSpec),
    CmpRegImm(RegisterSpec, DataTypeLiteral),

    // Arithmetic
    AddRegReg(RegisterSpec, RegisterSpec),
    AddRegImm(RegisterSpec, DataTypeLiteral),
    SubRegReg(RegisterSpec, RegisterSpec),
    SubRegImm(RegisterSpec, DataTypeLiteral),
    MulRegReg(RegisterSpec, RegisterSpec),
    MulRegImm(RegisterSpec, DataTypeLiteral),
    DivRegReg(RegisterSpec, RegisterSpec),
    DivRegImm(RegisterSpec, DataTypeLiteral),
    ModRegReg(RegisterSpec, RegisterSpec),
    ModRegImm(RegisterSpec, DataTypeLiteral),

    // Stack
    PushReg(RegisterSpec),
    PushImm(DataTypeLiteral),
    PopReg(RegisterSpec),
    PopMem(DerefType),

    // Jump
    JmpReg(JumpCondition, RegisterSpec),
    JmpImm(JumpCondition, DataTypeLiteral),
    JmpLabel(JumpCondition, SymbolRef),
}

impl OpcodePayload {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::EngCall => Opcode::EngCall,
            Self::Nop => Opcode::Nop,
            Self::CallReg(_) => Opcode::CallReg,
            Self::CallImm(_) => Opcode::CallImm,
            Self::Ret => Opcode::Ret,
            Self::StackAllocImm(_) => Opcode::StackAllocImm,
            Self::StackAllocReg(_) => Opcode::StackAllocReg,
            Self::StackFree => Opcode::StackFree,
            Self::Exit => Opcode::Exit,
            Self::RetFree => Opcode::RetFree,
            Self::HAllocImm(_) => Opcode::HAllocImm,
            Self::HAllocReg(_) => Opcode::HAllocReg,
            Self::HFreeImm(_) => Opcode::HFreeImm,
            Self::HFreeReg(_) => Opcode::HFreeReg,
            Self::CallLabel(_) => Opcode::CallLabel,

            Self::MovRegImm(_, _) => Opcode::MovRegImm,
            Self::MovRegMem(_, _) => Opcode::MovRegMem,
            Self::MovMemImm(_, _) => Opcode::MovMemImm,
            Self::MovMemReg(_, _) => Opcode::MovMemReg,
            Self::MovRegReg(_, _) => Opcode::MovRegReg,
            Self::MovRegEngineParam(_, _) => Opcode::MovRegEngineParam,
            Self::MovEngineParamReg(_, _) => Opcode::MovEngineParamReg,
            Self::MovRegLabel(_, _) => Opcode::MovRegLabel,
            Self::CmpRegReg(_, _) => Opcode::CmpRegReg,
            Self::CmpRegImm(_, _) => Opcode::CmpRegImm,

            Self::AddRegReg(_, _) => Opcode::AddRegReg,
            Self::AddRegImm(_, _) => Opcode::AddRegImm,
            Self::SubRegReg(_, _) => Opcode::SubRegReg,
            Self::SubRegImm(_, _) => Opcode::SubRegImm,
            Self::MulRegReg(_, _) => Opcode::MulRegReg,
            Self::MulRegImm(_, _) => Opcode::MulRegImm,
            Self::DivRegReg(_, _) => Opcode::DivRegReg,
            Self::DivRegImm(_, _) => Opcode::DivRegImm,
            Self::ModRegReg(_, _) => Opcode::ModRegReg,
            Self::ModRegImm(_, _) => Opcode::ModRegImm,

            Self::PushReg(_) => Opcode::PushReg,
            Self::PushImm(_) => Opcode::PushImm,
            Self::PopReg(_) => Opcode::PopReg,
            Self::PopMem(_) => Opcode::PopMem,

            Self::JmpReg(_, _) => Opcode::JmpReg,
            Self::JmpImm(_, _) => Opcode::JmpImm,
            Self::JmpLabel(_, _) => Opcode::JmpLabel,
        }
    }

    pub fn bytecode_serialize(&self, stream: &mut BytecodeStream, bindings: &BindingTable, labels: &LabelTable, eng_calls: &EngCallTable) -> BytecodeSerializeResult<()> {
        let opcode = self.opcode();
        stream.write_u8(opcode as u8);

        match self {
            Self::CallReg(reg) => {
                reg.serialize(stream);
            },
            Self::CallImm(imm) => {
                imm.serialize(stream);
            },
            Self::StackAllocImm(imm) => {
                imm.serialize(stream);
            },
            Self::StackAllocReg(reg) => {
                reg.serialize(stream);
            },
            Self::HAllocImm(imm) => {
                imm.serialize(stream);
            },
            Self::HAllocReg(reg) => {
                reg.serialize(stream);
            },
            Self::HFreeImm(imm) => {
                imm.serialize(stream);
            },
            Self::HFreeReg(reg) => {
                reg.serialize(stream);
            },
            Self::CallLabel(label_ref) => {
                label_ref.bytecode_serialize(stream, labels)?;
            },
            Self::MovRegImm(reg, imm) => {
                reg.serialize(stream);
                imm.serialize(stream);
            },
            Self::MovRegMem(reg, deref) => {
                reg.serialize(stream);
                deref.bytecode_serialize(stream, bindings)?;
            },
            Self::MovMemImm(deref, imm) => {
                deref.bytecode_serialize(stream, bindings)?;
                imm.serialize(stream);
            },
            Self::MovMemReg(deref, reg) => {
                deref.bytecode_serialize(stream, bindings)?;
                reg.serialize(stream);
            },
            Self::MovRegReg(reg1, reg2) => {
                reg1.serialize(stream);
                reg2.serialize(stream);
            },
            Self::MovRegEngineParam(reg, param_name) => {
                reg.serialize(stream);
                let param_index = eng_calls.get(param_name).ok_or_else(|| BytecodeSerializeError::UnknownEngineParameter(param_name.clone()))?;
                stream.write_varint64(param_index);
            },
            Self::MovEngineParamReg(param_name, reg) => {
                let param_index = eng_calls.get(param_name).ok_or_else(|| BytecodeSerializeError::UnknownEngineParameter(param_name.clone()))?;
                stream.write_varint64(param_index);
                reg.serialize(stream);
            },
            Self::MovRegLabel(reg, label_ref) => {
                reg.serialize(stream);
                label_ref.bytecode_serialize(stream, labels)?;
            },
            Self::CmpRegReg(reg1, reg2) => {
                reg1.serialize(stream);
                reg2.serialize(stream);
            },
            Self::CmpRegImm(reg, imm) => {
                reg.serialize(stream);
                imm.serialize(stream);
            },
            Self::AddRegReg(reg1, reg2) => {
                reg1.serialize(stream);
                reg2.serialize(stream);
            },
            Self::AddRegImm(reg, imm) => {
                reg.serialize(stream);
                imm.serialize(stream);
            },
            Self::SubRegReg(reg1, reg2) => {
                reg1.serialize(stream);
                reg2.serialize(stream);
            },
            Self::SubRegImm(reg, imm) => {
                reg.serialize(stream);
                imm.serialize(stream);
            },
            Self::MulRegReg(reg1, reg2) => {
                reg1.serialize(stream);
                reg2.serialize(stream);
            },
            Self::MulRegImm(reg, imm) => {
                reg.serialize(stream);
                imm.serialize(stream);
            },
            Self::DivRegReg(reg1, reg2) => {
                reg1.serialize(stream);
                reg2.serialize(stream);
            },
            Self::DivRegImm(reg, imm) => {
                reg.serialize(stream);
                imm.serialize(stream);
            },
            Self::ModRegReg(reg1, reg2) => {
                reg1.serialize(stream);
                reg2.serialize(stream);
            },
            Self::ModRegImm(reg, imm) => {
                reg.serialize(stream);
                imm.serialize(stream);
            },
            Self::PushReg(reg) => {
                reg.serialize(stream);
            },
            Self::PushImm(imm) => {
                imm.serialize(stream);
            },
            Self::PopReg(reg) => {
                reg.serialize(stream);
            },
            Self::PopMem(deref) => {
                deref.bytecode_serialize(stream, bindings)?;
            },
            Self::JmpReg(cond, reg) => {
                stream.write_u8(*cond as u8);
                reg.serialize(stream);
            },
            Self::JmpImm(cond, imm) => {
                stream.write_u8(*cond as u8);
                imm.serialize(stream);
            },
            Self::JmpLabel(cond, label_ref) => {
                stream.write_u8(*cond as u8);
                label_ref.bytecode_serialize(stream, labels)?;
            },
            _ => {}
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum JumpCondition {
    Unconditional = 0x00,
    Zero = 0x01,
    NotZero = 0x02,
    Sign = 0x03,
    NotSign = 0x04,
    Overflow = 0x05,
    NotOverflow = 0x06,
    Carry = 0x07,
    NotCarry = 0x08,
    Parity = 0x09,
    NotParity = 0x0a,
    BelowEqual = 0x0b,
    Above = 0x0c,
    Less = 0x0d,
    GreaterEqual = 0x0e,
    LessEqual = 0x0f,
    Greater = 0x10,
    Err = 0x21,
    NotErr = 0x22,
}

/// Represents the mnemonics for the F750 virtual machine opcodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr, Display, EnumString)]
pub enum OpcodeMnemonic {
    #[strum(serialize = "engcall")]
    EngCall,
    #[strum(serialize = "nop")]
    Nop,
    #[strum(serialize = "call")]
    Call,
    #[strum(serialize = "ret")]
    Ret,
    #[strum(serialize = "stackalloc")]
    StackAlloc,
    #[strum(serialize = "stackfree")]
    StackFree,
    #[strum(serialize = "exit")]
    Exit,
    #[strum(serialize = "retf")]
    RetFree,
    #[strum(serialize = "malloc")]
    HAlloc,
    #[strum(serialize = "free")]
    HFree,
    #[strum(serialize = "mov")]
    Mov,
    #[strum(serialize = "jmp")]
    Jmp,
    #[strum(serialize = "jz")]
    JmpIfZero,
    #[strum(serialize = "jnz")]
    JmpIfNotZero,
    #[strum(serialize = "je")]
    JmpIfEqual,
    #[strum(serialize = "jne")]
    JmpIfNotEqual,
    #[strum(serialize = "js")]
    JmpIfSign,
    #[strum(serialize = "jns")]
    JmpIfNotSign,
    #[strum(serialize = "jo")]
    JmpIfOverflow,
    #[strum(serialize = "jno")]
    JmpIfNotOverflow,
    #[strum(serialize = "jc")]
    JmpIfCarry,
    #[strum(serialize = "jb")]
    JmpIfBelow,
    #[strum(serialize = "jnae")]
    JmpIfNotAboveEqual,
    #[strum(serialize = "jnc")]
    JmpIfNotCarry,
    #[strum(serialize = "jae")]
    JmpIfAboveEqual,
    #[strum(serialize = "jnb")]
    JmpIfNotBelow,
    #[strum(serialize = "jp")]
    JmpIfParity,
    #[strum(serialize = "jnp")]
    JmpIfNotParity,
    #[strum(serialize = "jbe")]
    JmpIfBelowEqual,
    #[strum(serialize = "jna")]
    JmpIfNotAbove,
    #[strum(serialize = "ja")]
    JmpIfAbove,
    #[strum(serialize = "jnbe")]
    JmpIfNotBelowEqual,
    #[strum(serialize = "jl")]
    JmpIfLess,
    #[strum(serialize = "jnle")]
    JmpIfNotGreaterEqual,
    #[strum(serialize = "jge")]
    JmpIfGreaterEqual,
    #[strum(serialize = "jnl")]
    JmpIfNotLess,
    #[strum(serialize = "jle")]
    JmpIfLessEqual,
    #[strum(serialize = "jng")]
    JmpIfNotGreater,
    #[strum(serialize = "jg")]
    JmpIfGreater,
    #[strum(serialize = "jnge")]
    JmpIfNotLessEqual,
    #[strum(serialize = "jer")]
    JmpIfErr,
    #[strum(serialize = "jnok")]
    JmpIfNotOk,
    #[strum(serialize = "jner")]
    JmpIfNotErr,
    #[strum(serialize = "jok")]
    JmpIfOk,
    #[strum(serialize = "cmp")]
    Cmp,
    #[strum(serialize = "add")]
    Add,
    #[strum(serialize = "sub")]
    Sub,
    #[strum(serialize = "mul")]
    Mul,
    #[strum(serialize = "div")]
    Div,
    #[strum(serialize = "mod")]
    Mod,
    #[strum(serialize = "push")]
    Push,
    #[strum(serialize = "pop")]
    Pop,
}

impl OpcodeMnemonic {
    pub fn is_jump(&self) -> bool {
        self.to_jump_condition().is_some()
    }

    pub fn to_jump_condition(&self) -> Option<JumpCondition> {
        match self {
            OpcodeMnemonic::Jmp => Some(JumpCondition::Unconditional),
            OpcodeMnemonic::JmpIfZero => Some(JumpCondition::Zero),
            OpcodeMnemonic::JmpIfNotZero => Some(JumpCondition::NotZero),
            OpcodeMnemonic::JmpIfEqual => Some(JumpCondition::Zero),
            OpcodeMnemonic::JmpIfNotEqual => Some(JumpCondition::NotZero),
            OpcodeMnemonic::JmpIfSign => Some(JumpCondition::Sign),
            OpcodeMnemonic::JmpIfNotSign => Some(JumpCondition::NotSign),
            OpcodeMnemonic::JmpIfOverflow => Some(JumpCondition::Overflow),
            OpcodeMnemonic::JmpIfNotOverflow => Some(JumpCondition::NotOverflow),
            OpcodeMnemonic::JmpIfCarry => Some(JumpCondition::Carry),
            OpcodeMnemonic::JmpIfBelow => Some(JumpCondition::Carry),
            OpcodeMnemonic::JmpIfNotAboveEqual => Some(JumpCondition::Carry),
            OpcodeMnemonic::JmpIfNotCarry => Some(JumpCondition::NotCarry),
            OpcodeMnemonic::JmpIfAboveEqual => Some(JumpCondition::NotCarry),
            OpcodeMnemonic::JmpIfNotBelow => Some(JumpCondition::NotCarry),
            OpcodeMnemonic::JmpIfParity => Some(JumpCondition::Parity),
            OpcodeMnemonic::JmpIfNotParity => Some(JumpCondition::NotParity),
            OpcodeMnemonic::JmpIfBelowEqual => Some(JumpCondition::BelowEqual),
            OpcodeMnemonic::JmpIfNotAbove => Some(JumpCondition::BelowEqual),
            OpcodeMnemonic::JmpIfAbove => Some(JumpCondition::Above),
            OpcodeMnemonic::JmpIfNotBelowEqual => Some(JumpCondition::Above),
            OpcodeMnemonic::JmpIfLess => Some(JumpCondition::Less),
            OpcodeMnemonic::JmpIfNotGreaterEqual => Some(JumpCondition::Less),
            OpcodeMnemonic::JmpIfGreaterEqual => Some(JumpCondition::GreaterEqual),
            OpcodeMnemonic::JmpIfNotLess => Some(JumpCondition::GreaterEqual),
            OpcodeMnemonic::JmpIfLessEqual => Some(JumpCondition::LessEqual),
            OpcodeMnemonic::JmpIfNotGreater => Some(JumpCondition::LessEqual),
            OpcodeMnemonic::JmpIfGreater => Some(JumpCondition::Greater),
            OpcodeMnemonic::JmpIfNotLessEqual => Some(JumpCondition::Greater),
            OpcodeMnemonic::JmpIfErr => Some(JumpCondition::Err),
            OpcodeMnemonic::JmpIfNotOk => Some(JumpCondition::Err),
            OpcodeMnemonic::JmpIfNotErr => Some(JumpCondition::NotErr),
            OpcodeMnemonic::JmpIfOk => Some(JumpCondition::NotErr),
            _ => None,
        }
    }
}

/// Represents all register families of the F750 virtual machine.
///
/// Note: The maximum allowed register number for the current bytecode specification is `0x3F`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr, Display, EnumString)]
pub enum Register {
    #[strum(serialize = "a")]
    A = 0x01,
    #[strum(serialize = "b")]
    B = 0x02,
    #[strum(serialize = "c")]
    C = 0x03,
    #[strum(serialize = "d")]
    D = 0x04,
    #[strum(serialize = "sp")]
    StackPointer = 0x05,
    #[strum(serialize = "bp")]
    BasePointer = 0x06,
    #[strum(serialize = "si")]
    SourceIndex = 0x07,
    #[strum(serialize = "di")]
    DestinationIndex = 0x08,
    #[strum(serialize = "ip")]
    InstructionPointer = 0x09,
    #[strum(serialize = "lca")]
    LoopCounterA = 0x0A,
    #[strum(serialize = "lcb")]
    LoopCounterB = 0x0B,
    #[strum(serialize = "la")]
    LocalA = 0x0C,
    #[strum(serialize = "lb")]
    LocalB = 0x0D,
    #[strum(serialize = "lc")]
    LocalC = 0x0E,
    #[strum(serialize = "ld")]
    LocalD = 0x0F,
    #[strum(serialize = "le")]
    LocalE = 0x10,
    #[strum(serialize = "lf")]
    LocalF = 0x11,
    #[strum(serialize = "lg")]
    LocalG = 0x12,
    #[strum(serialize = "lh")]
    LocalH = 0x13,
}

impl Register {
    pub fn local_arg(index: u8) -> Option<Self> {
        match index {
            0 => Some(Register::LocalA),
            1 => Some(Register::LocalB),
            2 => Some(Register::LocalC),
            3 => Some(Register::LocalD),
            4 => Some(Register::LocalE),
            5 => Some(Register::LocalF),
            6 => Some(Register::LocalG),
            7 => Some(Register::LocalH),
            _ => None,
        }
    }
}

/// Represents all compiler constructs of the F750 virtual machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr, Display, EnumString)]
pub enum CompilerConstruct {
    #[strum(serialize = "end")]
    BlockEnd,
    #[strum(serialize = "if")]
    If,
    #[strum(serialize = "else")]
    Else,
    #[strum(serialize = "elseif")]
    ElseIf,
    #[strum(serialize = "inline_loop")]
    InlineLoop,
    #[strum(serialize = "loop")]
    Loop,
    #[strum(serialize = "while")]
    While,
    #[strum(serialize = "for")]
    For,
    #[strum(serialize = "break")]
    LoopBreak,
    #[strum(serialize = "continue")]
    LoopContinue,
    #[strum(serialize = "pry")]
    Pry,
    #[strum(serialize = "getarg")]
    GetArg,
    #[strum(serialize = "engcall")]
    EngineCall,
    #[strum(serialize = "loadargs")]
    LoadArgs,
    #[strum(serialize = "call")]
    ProcCall,
}

impl CompilerConstruct {
    pub fn is_block_start(&self) -> bool {
        matches!(
            self,
            CompilerConstruct::If
                | CompilerConstruct::InlineLoop
                | CompilerConstruct::Loop
                | CompilerConstruct::While
                | CompilerConstruct::For
        )
    }
}

/// Represents all compiler directives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr, Display, EnumString)]
pub enum CompilerDirective {
    #[strum(serialize = "use")]
    Use,
    #[strum(serialize = "namespace")]
    Namespace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr, Display, EnumString)]
pub enum ReservedWord {
    #[strum(serialize = "const")]
    Const,
    #[strum(serialize = "extern")]
    Extern,
    #[strum(serialize = "struct")]
    Alias,
    #[strum(serialize = "pub")]
    Public,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonFlag {
    Zero = 1 << 0,
    Sign = 1 << 1,
    Overflow = 1 << 2,
    Carry = 1 << 3,
    Parity = 1 << 4,
    Error = 1 << 5,
}