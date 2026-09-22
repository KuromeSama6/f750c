use strum::{AsRefStr, Display, EnumString};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    EngCall = 0x01,
    Nop = 0x02,
    CallReg = 0x03,
    CallImm = 0x04,
    Ret = 0x05,

    // Move
    MovRegImm = 0xa0,
    MovRegMem = 0xa1,
    MovMemImm = 0xa2,
    MovMemReg = 0xa3,
    MovRegReg = 0xa4,

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

    // Comparison
    CmpRegReg = 0xc0,
    CmpRegImm = 0xc1,

    // Arithmetic
    AddRegReg = 0xd0,
    AddRegImm = 0xd1,
    SubRegReg = 0xd2,
    SubRegImm = 0xd3,
    MulRegReg = 0xd4,
    MulRegImm = 0xd5,
    DivRegReg = 0xd6,
    DivRegImm = 0xd7,
    ModRegReg = 0xd8,
    ModRegImm = 0xd9,

    // Stack,
    PushReg = 0xe0,
    PushImm = 0xe1,
    PopReg = 0xe2,
    PopMem = 0xe3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr, Display, EnumString)]
pub enum OpcodeMnemonic {
    #[strum(serialize = "engcall")]
    EngCall,
    #[strum(serialize = "nop")]
    Nop,
    #[strum(serialize = "mov")]
    Mov,
    #[strum(serialize = "jmp")]
    Jmp,
    #[strum(serialize = "jz")]
    Jz,
    #[strum(serialize = "jnz")]
    Jnz,
    #[strum(serialize = "js")]
    Js,
    #[strum(serialize = "jns")]
    Jns,
    #[strum(serialize = "jo")]
    Jo,
    #[strum(serialize = "jno")]
    Jno,
    #[strum(serialize = "jc")]
    Jc,
    #[strum(serialize = "jnc")]
    Jnc,
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

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Register {
    Ax = 0x01,
    Bx = 0x02,
    Cx = 0x03,
    Dx = 0x04,
    Sp = 0x05,
    Bp = 0x06,
    Si = 0x07,
    Di = 0x08,
}