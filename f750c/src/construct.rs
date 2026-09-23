//! This module contains logic for the compiler construct expansion phase.

use std::fmt::{Display, Formatter};
use log::warn;
use thiserror::Error;
use crate::opcode::{CompilerConstruct, OpcodeMnemonic, Register};
use crate::semantic::{SemanticOperandKind, SemanticCompilerConstruct, SemanticInstruction, SemanticOperand, SemanticRepr, SemanticReprStream};
use crate::tokenizer;
use crate::value::{BindingDerefType, RegisterSpec};

#[derive(Debug, Clone)]
pub struct ConstructExpansionErrorDetails {
    pub error: ConstructExpansionError,
    pub source: SemanticRepr,
    pub line: usize,
}

impl Display for ConstructExpansionErrorDetails {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Compiler construct expansion error: '{}' (line {}): {}", self.source, self.line, self.error)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Error)]
pub enum ConstructExpansionError {
    #[error("Construct expansion not implemented for {0:?}")]
    ConstructNotImplemented(CompilerConstruct),
    #[error("Compiler constructs are not allowed in the data section.")]
    ConstructInDataSection,
    #[error("Compiler construct '{opcode}' expects exactly {expected} operand(s).")]
    ExpectedExactOperands {
        opcode: CompilerConstruct,
        expected: usize,
    },
    #[error("Compiler construct '{opcode}' expects at least {minimum} operand(s).")]
    MinimumOperands {
        opcode: CompilerConstruct,
        minimum: usize,
    },
    #[error("Invalid operand at index {index} '{operand}': {reason}")]
    InvalidOperand {
        index: usize,
        operand: SemanticOperand,
        reason: String,
    }
}

impl ConstructExpansionError {
    pub fn into_details(self, source: &SemanticRepr, line: usize) -> ConstructExpansionErrorDetails {
        ConstructExpansionErrorDetails {
            error: self,
            source: source.clone(),
            line,
        }
    }
}

impl SemanticCompilerConstruct {
    pub fn expect_exact_operands(&self, n: usize) -> Result<(), ConstructExpansionError> {
        if self.operands.len() != n {
            return Err(ConstructExpansionError::ExpectedExactOperands {
                opcode: self.opcode,
                expected: n,
            });
        }
        Ok(())
    }

    pub fn invalid_operand_error(&self, index: usize, reason: &str) -> Result<(), ConstructExpansionError> {
        if index >= self.operands.len() {
            panic!("Invalid operand index {} for construct with {} operands", index, self.operands.len());
        }

        Err(ConstructExpansionError::InvalidOperand {
            index,
            operand: self.operands[index].clone(),
            reason: reason.to_string(),
        })
    }
}

pub fn expand_construct_source(source: &[SemanticRepr]) -> Result<Vec<SemanticRepr>, ConstructExpansionErrorDetails> {
    let mut stream = SemanticReprStream::from(source);
    let mut ret = Vec::with_capacity(source.len());
    let mut data_section = false;

    for (i, line) in source.iter().enumerate() {
        // toggle data section
        if let SemanticRepr::SpecialSection(name) = line && name == tokenizer::SPECIAL_SECTION_LABEL_DATA {
            data_section = true;
        }

        if matches!(line, SemanticRepr::Label(_)) || matches!(line, SemanticRepr::Instruction(_)) {
            data_section = false;
        }

        let SemanticRepr::CompilerConstruct(construct) = line else {
            ret.push(line.clone());
            continue;
        };

        if data_section {
            return Err(ConstructExpansionError::ConstructInDataSection.into_details(line, i));
        }

        let result = match construct.opcode {
            CompilerConstruct::Pry => expand_construct_pry(&construct, &mut ret),
            CompilerConstruct::GetArg => expand_construct_getarg(&construct, &mut ret),
            _ => Err(ConstructExpansionError::ConstructNotImplemented(construct.opcode))
        };

        if let Err(e) = result {
            return Err(e.into_details(line, i));
        }
    }

    Ok(ret)
}

// @pry <binding>, <value>
fn expand_construct_pry(construct: &SemanticCompilerConstruct, out: &mut Vec<SemanticRepr>) -> Result<(), ConstructExpansionError> {
    warn!("Use of unsafe compiler construct '@pry': '{construct}'");

    construct.expect_exact_operands(2)?;

    let arg0 = &construct.operands[0];
    let SemanticOperandKind::Binding(binding) = &arg0.kind else {
        return construct.invalid_operand_error(0, "Expected a binding as the first operand.");
    };

    if arg0.is_deref() {
        return construct.invalid_operand_error(0, "This binding must not be dereferenced.");
    }

    if arg0.has_offset() {
        return construct.invalid_operand_error(0, "This binding must not have an offset.");
    }

    let arg1 = &construct.operands[1];
    if !(arg1.is_immediate() || arg1.is_register()) {
        return construct.invalid_operand_error(1, "Expected an immediate value or a register.");
    }

    if arg1.is_register() && !arg1.is_flat() {
        return construct.invalid_operand_error(1, "This register must not be dereferenced or have an offset.");
    }

    // generate code

    // mov rsi, <binding>
    out.push(SemanticRepr::Instruction(SemanticInstruction {
        opcode: OpcodeMnemonic::Mov,
        operands: vec![
            SemanticOperand::with_body(SemanticOperandKind::Register(RegisterSpec::qword(Register::SourceIndex))),
            SemanticOperand {
                kind: SemanticOperandKind::Binding(binding.clone()),
                deref: Some(BindingDerefType::Dynamic),
                offset: 0,
            },
        ],
    }));

    // mov &rsi, <value>
    out.push(SemanticRepr::Instruction(SemanticInstruction {
        opcode: OpcodeMnemonic::Mov,
        operands: vec![
            SemanticOperand {
                kind: SemanticOperandKind::Register(RegisterSpec::qword(Register::SourceIndex)),
                deref: Some(BindingDerefType::Dynamic),
                offset: 0,
            },
            arg1.clone(),
        ],
    }));

    Ok(())
}

// @getarg <register>, <offset>
fn expand_construct_getarg(construct: &SemanticCompilerConstruct, out: &mut Vec<SemanticRepr>) -> Result<(), ConstructExpansionError> {
    construct.expect_exact_operands(2)?;

    let arg0 = &construct.operands[0];
    let SemanticOperandKind::Register(reg) = &arg0.kind else {
        return construct.invalid_operand_error(0, "Expected a register as the first operand.");
    };

    if !arg0.is_flat() {
        return construct.invalid_operand_error(0, "This register must not be dereferenced or have an offset.");
    }

    let arg1 = &construct.operands[1];
    let SemanticOperandKind::Literal(lit) = &arg1.kind else {
        return construct.invalid_operand_error(1, "Expected a literal as the second operand.");
    };

    if lit.data_type().is_floating_point() {
        return construct.invalid_operand_error(1, "This literal must not be a integer value.");
    }

    let offset_amount = lit.as_int().unwrap();

    out.push(SemanticRepr::Instruction(SemanticInstruction {
        opcode: OpcodeMnemonic::Mov,
        operands: vec![
            SemanticOperand::with_body(SemanticOperandKind::Register(reg.clone())),
            SemanticOperand {
                kind: SemanticOperandKind::Register(RegisterSpec::qword(Register::BasePointer)),
                deref: Some(BindingDerefType::Dynamic),
                offset: 16 + offset_amount,
            },
        ],
    }));

    Ok(())
}

fn expand_construt_engcall(construct: &SemanticCompilerConstruct, out: &mut Vec<SemanticRepr>) -> Result<(), ConstructExpansionError> {
    if construct.operands.len() < 1 {
        return Err(ConstructExpansionError::MinimumOperands {
            opcode: construct.opcode,
            minimum: 1,
        });
    }

    let arg0 = &construct.operands[0];
    let SemanticOperandKind::EngineParam(_) = &arg0.kind else {
        return construct.invalid_operand_error(0, "Expected an engine parameter as the first operand.");
    };

    let mut args: Vec<&SemanticOperand> = Vec::new();
    for (i, operand) in construct.operands.iter()
        .enumerate()
        .skip(1)
    {
        match operand.kind {
            _ => return construct.invalid_operand_error(i, format!("For parameter {i} '{operand}': ").as_str()),
        }
    }

    Ok(())
}