use crate::opcode::Register;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DataType {
    Byte(u8),
    Word(u16),
    Dword(u32),
    Qword(u64),
    Float(f32),
    Double(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterWidth {
    Byte,
    Word,
    Dword,
    Qword,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterSpec {
    pub register: Register,
    pub width: RegisterWidth,
}