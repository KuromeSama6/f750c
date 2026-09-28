use thiserror::Error;
use crate::bytecode::BytecodeStream;
use crate::compiler::{BindingTable, LabelTable};
use crate::opcode::{Opcode, OpcodeMnemonic};
use crate::semantic::{SemanticInstruction, SemanticOperand};

macro_rules! no_matching_opcode {
    ($mnemonic: expr, $operands: expr) => {{
        let ins = SemanticInstruction {
            opcode: $mnemonic,
            operands: $operands.to_vec(),
        };
        Err(InstructionEncodeError::NoMatchingOpcode(ins.to_string()))
    }};
}

macro_rules! require_operants_exact {
    ($operands: expr, $n: expr) => {{
        if $operands.len() != $n {
            return Err(InstructionEncodeError::ExpectedOperandCount($n));
        }
    }};
}

#[derive(Debug, Error)]
pub enum InstructionEncodeError {
    #[error("Expected exactly {0} operands")]
    ExpectedOperandCount(usize),
    #[error("No matching opcode found for '{0}'")]
    NoMatchingOpcode(String),
}

pub type InstructionEncodeResult<T> = Result<T, InstructionEncodeError>;

#[derive(Debug)]
pub struct InstructionEncodeContext {
    pub binding_table: BindingTable,
    pub label_table: LabelTable,
}

pub fn encode_instructions(instructions: &[SemanticInstruction], ctx: &InstructionEncodeContext, stream: &mut BytecodeStream) -> InstructionEncodeResult<()>{
    stream.write_varint64(instructions.len() as u64);

    for instruction in instructions {
        encode_instruction_single(instruction, ctx, stream)?;
    }

    todo!()
}

fn encode_instruction_single(instruction: &SemanticInstruction, ctx: &InstructionEncodeContext, stream: &mut BytecodeStream) -> InstructionEncodeResult<()> {
    let opcode = select_opcode(instruction.opcode, &instruction.operands)?;
    todo!()
}

fn select_opcode(mnemonic: OpcodeMnemonic, operands: &[SemanticOperand]) -> InstructionEncodeResult<Opcode> {
    match mnemonic {
        OpcodeMnemonic::EngCall => {
            require_operants_exact!(operands, 1);
            Ok(Opcode::EngCall)
        }
        _ => no_matching_opcode!(mnemonic, operands)
    }
}