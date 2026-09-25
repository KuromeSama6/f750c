//! This module defines opcodes, mnemonics, registers, and other compile-time identifiers for F750. 
//! These identifiers are not semantic - they are used beyond the parsing stage.

use strum::{AsRefStr, Display, EnumString};

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

    // Move
    MovRegImm = 0x20,
    MovRegMem = 0x21,
    MovMemImm = 0x22,
    MovMemReg = 0x23,
    MovRegReg = 0x24,
    MovRegEngineParam = 0x25,
    MovEngineParamReg = 0x26,

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

    // Stack,
    PushReg = 0x50,
    PushImm = 0x51,
    PopReg = 0x52,
    PopMem = 0x53,

    // Jump
    Jmp = 0xb0,
    JmpIfZero = 0xb1,
    JmpIfNotZero = 0xb2,
    JmpIfSign = 0xb3,
    JmpIfNotSign = 0xb4,
    JmpIfOverflow = 0xb5,
    JmpIfNotOverflow = 0xb6,
    JmpIfCarry = 0xb7,
    JmpIfNotCarry = 0xb8,
    JmpIfParity = 0xb9,
    JmpIfNotParity = 0xba,
    JmpIfBelowEqual = 0xbb,
    JmpIfAbove = 0xbc,
    JmpIfLess = 0xbd,
    JmpIfGreaterEqual = 0xbe,
    JmpIfLessEqual = 0xbf,
    JmpIfGreater = 0xc0,
    JmpIfErr = 0xc1,
    JmpIfNotErr = 0xc2,
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
        matches!(
            self,
            OpcodeMnemonic::Jmp
                | OpcodeMnemonic::JmpIfZero
                | OpcodeMnemonic::JmpIfNotZero
                | OpcodeMnemonic::JmpIfEqual
                | OpcodeMnemonic::JmpIfNotEqual
                | OpcodeMnemonic::JmpIfSign
                | OpcodeMnemonic::JmpIfNotSign
                | OpcodeMnemonic::JmpIfOverflow
                | OpcodeMnemonic::JmpIfNotOverflow
                | OpcodeMnemonic::JmpIfCarry
                | OpcodeMnemonic::JmpIfNotCarry
                | OpcodeMnemonic::JmpIfParity
                | OpcodeMnemonic::JmpIfNotParity
                | OpcodeMnemonic::JmpIfBelowEqual
                | OpcodeMnemonic::JmpIfAbove
                | OpcodeMnemonic::JmpIfLess
                | OpcodeMnemonic::JmpIfGreaterEqual
                | OpcodeMnemonic::JmpIfLessEqual
                | OpcodeMnemonic::JmpIfGreater
        )
    }
}

/// Represents all register families of the F750 virtual machine.
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