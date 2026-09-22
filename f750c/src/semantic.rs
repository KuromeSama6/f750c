use bitfield_struct::bitfield;
use crate::opcode::OpcodeMnemonic;
use crate::value::{DataType, RegisterSpec};

#[derive(Debug, Clone)]
pub struct SemanticInstruction {
    pub opcode: OpcodeMnemonic,
    pub args: [SemanticArg; 2],
}

#[derive(Debug, Clone)]
pub struct SemanticArg {
    pub body: SemanticArgBody,
    pub deref: bool,
    pub offset: Option<i32>,
}

#[derive(Debug, Clone)]
pub enum SemanticArgBody {
    Literal(DataType),
    Register(RegisterSpec),
    Label(String),
    Binding(String),
}