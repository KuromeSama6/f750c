//! This module contains logic for the compiler construct expansion phase.

use std::fmt::{Display, Formatter};
use thiserror::Error;
use crate::opcode::CompilerConstruct;
use crate::semantic::{SemanticRepr, SemanticReprStream};
use crate::tokenizer;

#[derive(Debug, Clone)]
pub struct ConstructExpansionErrorDetails {
    pub error: ConstructExpansionError,
    pub source: SemanticRepr,
    pub line: usize,
}

impl Display for ConstructExpansionErrorDetails {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Compiler construct expansion error: Line {} in source {:?}: {}", self.line, self.source, self.error)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Error)]
pub enum ConstructExpansionError {
    #[error("Construct expansion not implemented for {0:?}")]
    ConstructNotImplemented(CompilerConstruct),
    #[error("Compiler constructs are not allowed in the data section.")]
    ConstructInDataSection,
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

        match construct {
            _ => return Err(ConstructExpansionError::ConstructNotImplemented(construct.opcode).into_details(line, i)),
        }
    }

    Ok(ret)
}