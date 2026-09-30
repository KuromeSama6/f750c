use thiserror::Error;
use crate::bytecode::{BytecodeSerializeError, BytecodeStream};
use crate::compiler::{BindingTable, EngCallTable, LabelTable};
use crate::opcode::{OpcodeMnemonic, OpcodePayload};
use crate::semantic::{SemanticDerefKind, SemanticImmediateType, SemanticInstruction, SemanticOperand};
use crate::value::{DerefType, SymbolRef};

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
    #[error(transparent)]
    BytecodeSerialize(#[from] BytecodeSerializeError),
}

pub type InstructionEncodeResult<T> = Result<T, InstructionEncodeError>;

#[derive(Debug)]
pub struct InstructionEncodeContext {
    pub binding_table: BindingTable,
    pub label_table: LabelTable,
    pub engcall_table: EngCallTable,
}

pub fn encode_instructions(instructions: &[SemanticInstruction], ctx: &InstructionEncodeContext, stream: &mut BytecodeStream) -> InstructionEncodeResult<()>{
    stream.write_varint64(instructions.len() as u64);

    for instruction in instructions {
        encode_instruction_single(instruction, ctx, stream)?;
    }

    Ok(())
}

fn encode_instruction_single(instruction: &SemanticInstruction, ctx: &InstructionEncodeContext, stream: &mut BytecodeStream) -> InstructionEncodeResult<()> {
    let payload = select_opcode(instruction.opcode, &instruction.operands)?;
    payload.bytecode_serialize(stream, &ctx.binding_table, &ctx.label_table, &ctx.engcall_table)?;

    Ok(())
}

fn select_opcode(mnemonic: OpcodeMnemonic, operands: &[SemanticOperand]) -> InstructionEncodeResult<OpcodePayload> {
    // jump (special case)
    if mnemonic.to_jump_condition().is_some() {
        return select_opcode_jmp(mnemonic, operands);
    }

    match mnemonic {
        OpcodeMnemonic::EngCall => {
            require_operants_exact!(operands, 0);
            Ok(OpcodePayload::EngCall)
        }
        OpcodeMnemonic::Nop => {
            require_operants_exact!(operands, 0);
            Ok(OpcodePayload::Nop)
        }
        OpcodeMnemonic::Call => {
            require_operants_exact!(operands, 1);
            match &operands[0] {
                SemanticOperand::Register(reg) => Ok(OpcodePayload::CallReg(*reg)),
                SemanticOperand::Immediate(SemanticImmediateType::Literal(lit)) => Ok(OpcodePayload::CallImm(lit.to_data_type())),
                SemanticOperand::Immediate(SemanticImmediateType::Label(label, offset)) => Ok(OpcodePayload::CallLabel(SymbolRef::new(label.name.clone(), *offset))),
                _ => no_matching_opcode!(mnemonic, operands)
            }
        }
        OpcodeMnemonic::Ret => {
            require_operants_exact!(operands, 0);
            Ok(OpcodePayload::Ret)
        }
        OpcodeMnemonic::StackAlloc => {
            require_operants_exact!(operands, 1);
            match &operands[0] {
                SemanticOperand::Immediate(SemanticImmediateType::Literal(l)) => Ok(OpcodePayload::StackAllocImm(l.to_data_type())),
                SemanticOperand::Register(reg) => Ok(OpcodePayload::StackAllocReg(*reg)),
                _ => no_matching_opcode!(mnemonic, operands)
            }
        }
        OpcodeMnemonic::StackFree => {
            require_operants_exact!(operands, 0);
            Ok(OpcodePayload::StackFree)
        }
        OpcodeMnemonic::Exit => {
            require_operants_exact!(operands, 0);
            Ok(OpcodePayload::Exit)
        }
        OpcodeMnemonic::RetFree => {
            require_operants_exact!(operands, 0);
            Ok(OpcodePayload::RetFree)
        }
        OpcodeMnemonic::HAlloc => {
            require_operants_exact!(operands, 2);
            // arg2 must be register
            if !matches!(operands[1], SemanticOperand::Register(_)) {
                return no_matching_opcode!(mnemonic, operands);
            }

            match &operands[0] {
                SemanticOperand::Immediate(SemanticImmediateType::Literal(l)) => Ok(OpcodePayload::HAllocImm(l.to_data_type())),
                SemanticOperand::Register(reg) => Ok(OpcodePayload::HAllocReg(*reg)),
                _ => no_matching_opcode!(mnemonic, operands)
            }
        }
        OpcodeMnemonic::HFree => {
            require_operants_exact!(operands, 1);
            match &operands[0] {
                SemanticOperand::Immediate(SemanticImmediateType::Literal(l)) => Ok(OpcodePayload::HFreeImm(l.to_data_type())),
                SemanticOperand::Register(reg) => Ok(OpcodePayload::HFreeReg(*reg)),
                _ => no_matching_opcode!(mnemonic, operands)
            }
        }
        OpcodeMnemonic::Mov => select_opcode_move(mnemonic, operands),
        OpcodeMnemonic::Cmp => {
            require_operants_exact!(operands, 2);
            let SemanticOperand::Register(reg1) = &operands[0] else {
                return no_matching_opcode!(mnemonic, operands);
            };
            
            match &operands[1] {
                SemanticOperand::Register(reg2) => Ok(OpcodePayload::CmpRegReg(*reg1, *reg2)),
                SemanticOperand::Immediate(SemanticImmediateType::Literal(l)) => Ok(OpcodePayload::CmpRegImm(*reg1, l.to_data_type())),
                _ => no_matching_opcode!(mnemonic, operands)
            }
        }
        OpcodeMnemonic::Add => {
            require_operants_exact!(operands, 2);
            let SemanticOperand::Register(reg1) = &operands[0] else {
                return no_matching_opcode!(mnemonic, operands);
            };
            
            match &operands[1] {
                SemanticOperand::Register(reg2) => Ok(OpcodePayload::AddRegReg(*reg1, *reg2)),
                SemanticOperand::Immediate(SemanticImmediateType::Literal(l)) => Ok(OpcodePayload::AddRegImm(*reg1, l.to_data_type())),
                _ => no_matching_opcode!(mnemonic, operands)
            }
        }
        OpcodeMnemonic::Sub => {
            require_operants_exact!(operands, 2);
            let SemanticOperand::Register(reg1) = &operands[0] else {
                return no_matching_opcode!(mnemonic, operands);
            };
            
            match &operands[1] {
                SemanticOperand::Register(reg2) => Ok(OpcodePayload::SubRegReg(*reg1, *reg2)),
                SemanticOperand::Immediate(SemanticImmediateType::Literal(l)) => Ok(OpcodePayload::SubRegImm(*reg1, l.to_data_type())),
                _ => no_matching_opcode!(mnemonic, operands)
            }
        }
        OpcodeMnemonic::Mul => {
            require_operants_exact!(operands, 2);
            let SemanticOperand::Register(reg1) = &operands[0] else {
                return no_matching_opcode!(mnemonic, operands);
            };
            
            match &operands[1] {
                SemanticOperand::Register(reg2) => Ok(OpcodePayload::MulRegReg(*reg1, *reg2)),
                SemanticOperand::Immediate(SemanticImmediateType::Literal(l)) => Ok(OpcodePayload::MulRegImm(*reg1, l.to_data_type())),
                _ => no_matching_opcode!(mnemonic, operands)
            }
        }
        OpcodeMnemonic::Div => {
            require_operants_exact!(operands, 2);
            let SemanticOperand::Register(reg1) = &operands[0] else {
                return no_matching_opcode!(mnemonic, operands);
            };
            
            match &operands[1] {
                SemanticOperand::Register(reg2) => Ok(OpcodePayload::DivRegReg(*reg1, *reg2)),
                SemanticOperand::Immediate(SemanticImmediateType::Literal(l)) => Ok(OpcodePayload::DivRegImm(*reg1, l.to_data_type())),
                _ => no_matching_opcode!(mnemonic, operands)
            }
        }
        OpcodeMnemonic::Mod => {
            require_operants_exact!(operands, 2);
            let SemanticOperand::Register(reg1) = &operands[0] else {
                return no_matching_opcode!(mnemonic, operands);
            };
            
            match &operands[1] {
                SemanticOperand::Register(reg2) => Ok(OpcodePayload::ModRegReg(*reg1, *reg2)),
                SemanticOperand::Immediate(SemanticImmediateType::Literal(l)) => Ok(OpcodePayload::ModRegImm(*reg1, l.to_data_type())),
                _ => no_matching_opcode!(mnemonic, operands)
            }
        }
        OpcodeMnemonic::Push => {
            require_operants_exact!(operands, 1);
            match &operands[0] {
                SemanticOperand::Register(reg) => Ok(OpcodePayload::PushReg(*reg)),
                SemanticOperand::Immediate(SemanticImmediateType::Literal(l)) => Ok(OpcodePayload::PushImm(l.to_data_type())),
                SemanticOperand::Immediate(SemanticImmediateType::Label(label, offset)) => Ok(OpcodePayload::PushLabel(SymbolRef::new(label.name.clone(), *offset))),
                SemanticOperand::Immediate(SemanticImmediateType::Binding(binding, offset)) => Ok(OpcodePayload::PushBinding(SymbolRef::new(binding.name.clone(), *offset))),
                _ => no_matching_opcode!(mnemonic, operands)
            }
        }
        OpcodeMnemonic::Pop => {
            require_operants_exact!(operands, 1);
            match &operands[0] {
                SemanticOperand::Register(reg) => Ok(OpcodePayload::PopReg(*reg)),
                SemanticOperand::Deref(deref) => match &deref.kind {
                    SemanticDerefKind::Binding(b) => Ok(OpcodePayload::PopMem(DerefType::Binding(SymbolRef::new(b.name.clone(), deref.offset)))),
                    SemanticDerefKind::Address(addr) => Ok(OpcodePayload::PopMem(DerefType::Addr(*addr))),
                    SemanticDerefKind::Register(reg) => Ok(OpcodePayload::PopMem(DerefType::Register(*reg, deref.offset))),
                    _ => no_matching_opcode!(mnemonic, operands)
                },
                _ => no_matching_opcode!(mnemonic, operands)
            }
        }
        _ => no_matching_opcode!(mnemonic, operands)
    }
}

fn select_opcode_move(mnemonic: OpcodeMnemonic, operands: &[SemanticOperand]) -> InstructionEncodeResult<OpcodePayload> {
    require_operants_exact!(operands, 2);

    match (&operands[0], &operands[1]) {
        (SemanticOperand::Register(reg), SemanticOperand::Immediate(SemanticImmediateType::Literal(l))) => {
            Ok(OpcodePayload::MovRegImm(*reg, l.to_data_type()))
        }
        (SemanticOperand::Register(reg), SemanticOperand::Deref(deref)) => {
            match &deref.kind {
                SemanticDerefKind::Binding(b) => Ok(OpcodePayload::MovRegMem(*reg, DerefType::Binding(SymbolRef::new(b.name.clone(), deref.offset)))),
                SemanticDerefKind::Address(addr) => Ok(OpcodePayload::MovRegMem(*reg, DerefType::Addr(*addr))),
                SemanticDerefKind::Register(reg) => Ok(OpcodePayload::MovRegMem(*reg, DerefType::Register(*reg, deref.offset))),
                _ => no_matching_opcode!(mnemonic, operands)
            }
        }
        (SemanticOperand::Deref(deref), SemanticOperand::Immediate(SemanticImmediateType::Literal(l))) => {
            match &deref.kind {
                SemanticDerefKind::Binding(b) => Ok(OpcodePayload::MovMemImm(DerefType::Binding(SymbolRef::new(b.name.clone(), deref.offset)), l.to_data_type())),
                SemanticDerefKind::Address(addr) => Ok(OpcodePayload::MovMemImm(DerefType::Addr(*addr), l.to_data_type())),
                SemanticDerefKind::Register(reg) => Ok(OpcodePayload::MovMemImm(DerefType::Register(*reg, deref.offset), l.to_data_type())),
                _ => no_matching_opcode!(mnemonic, operands)
            }
        }
        (SemanticOperand::Deref(deref), SemanticOperand::Register(reg)) => {
            match &deref.kind {
                SemanticDerefKind::Binding(b) => Ok(OpcodePayload::MovMemReg(DerefType::Binding(SymbolRef::new(b.name.clone(), deref.offset)), *reg)),
                SemanticDerefKind::Address(addr) => Ok(OpcodePayload::MovMemReg(DerefType::Addr(*addr), *reg)),
                SemanticDerefKind::Register(reg2) => Ok(OpcodePayload::MovMemReg(DerefType::Register(*reg2, deref.offset), *reg)),
                _ => no_matching_opcode!(mnemonic, operands)
            }
        }
        (SemanticOperand::Register(reg), SemanticOperand::Register(reg2)) => {
            Ok(OpcodePayload::MovRegReg(*reg, *reg2))
            }
            (SemanticOperand::Register(reg), SemanticOperand::EngineParam(param)) => {
                Ok(OpcodePayload::MovRegEngineParam(*reg, param.clone()))
            }
        (SemanticOperand::EngineParam(param), SemanticOperand::Register(reg)) => {
            Ok(OpcodePayload::MovEngineParamReg(param.clone(), *reg))
        }
        (SemanticOperand::Register(reg), SemanticOperand::Immediate(SemanticImmediateType::Label(label, offset))) => {
            Ok(OpcodePayload::MovRegLabel(*reg, SymbolRef::new(label.name.clone(), *offset)))
        }
        (SemanticOperand::Register(reg), SemanticOperand::Immediate(SemanticImmediateType::Binding(binding, offset))) => {
            Ok(OpcodePayload::MovRegBinding(*reg, SymbolRef::new(binding.name.clone(), *offset)))
        }
        _ => no_matching_opcode!(mnemonic, operands)
    }
}

fn select_opcode_jmp(mnemonic: OpcodeMnemonic, operands: &[SemanticOperand]) -> InstructionEncodeResult<OpcodePayload> {
    require_operants_exact!(operands, 1);
    let Some(condition) = mnemonic.to_jump_condition() else {
        return no_matching_opcode!(mnemonic, operands);
    };

    match &operands[0] {
        SemanticOperand::Immediate(SemanticImmediateType::Label(label, offset)) => {
            Ok(OpcodePayload::JmpLabel(condition, SymbolRef::new(label.name.clone(), *offset)))
        }
        SemanticOperand::Register(reg) => {
            Ok(OpcodePayload::JmpReg(condition, *reg))
        }
        SemanticOperand::Immediate(SemanticImmediateType::Literal(lit)) => {
            Ok(OpcodePayload::JmpImm(condition, lit.to_data_type()))
        }
        _ => no_matching_opcode!(mnemonic, operands)
    }
}