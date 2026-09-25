//! This module defines functions and logic for the bytecode emission stage of the F750 compiler.

use std::collections::HashMap;
use thiserror::Error;
use crate::semantic::{SemanticBindingDef, SemanticDeref, SemanticDerefKind, SemanticImmediateType, SemanticOperand, SemanticRepr, SemanticSource, SemanticSymbol};
use crate::tokenizer;

#[derive(Debug, Error)]
pub enum CompilerError {
    #[error("Data section start not allowed here")]
    DuplicateDataSection,
    #[error("Binding definition found in non-data section")]
    BindingDefInNonDataSection,
    #[error("Duplicate binding definition: {0}")]
    DuplicateBindingDef(SemanticSymbol),
    #[error("Use of undefined binding: {0}")]
    UndefinedBinding(SemanticSymbol),
    #[error("Constant dereference of a binding with multiple values is not allowed: {0}")]
    ConstDerefWithMultipleValues(SemanticSymbol),
    #[error("Constant dereference of a dynamic binding is not allowed: '{0}'. Either use dynamic dereference (&binding) or mark the binding as constant (const binding: ...).")]
    ConstDerefOfNonConstantBinding(SemanticSymbol),
    #[error("Constant dereference (&const binding) of a constant binding '{0}' occured, but this binding was previously dynamically dereferenced (&binding) or directly accessed (binding).")]
    MixedConstDerefOfConstantBinding(SemanticSymbol),
    #[error("Dynamic dereference (&binding) or direct access (binding) of a constant binding '{0}' occured, but this binding was previously constantly dereferenced (&const binding).")]
    MixedDynamicDerefOfConstantBinding(SemanticSymbol),
}
pub type CompileResult<T> = Result<T, CompilerError>;

#[derive(Debug, Default)]
pub struct BindingTable {
    pub entries: HashMap<SemanticSymbol, BindingTableEntry>,
    tentative_entries: HashMap<SemanticSymbol, SemanticBindingDef>,
    counter: usize,
    const_deref_map: HashMap<SemanticSymbol, bool>,
}

#[derive(Debug, Clone)]
pub struct BindingTableEntry {
    pub def: SemanticBindingDef,
    pub offset: Option<usize>,
}

impl BindingTable {
    pub fn new() -> Self {
        BindingTable {
            entries: HashMap::new(),
            tentative_entries: HashMap::new(),
            counter: 0,
            const_deref_map: HashMap::new(),
        }
    }

    pub fn preprocess_bindings(&mut self, source: &SemanticSource, module_name: &str) -> CompileResult<()> {
        for line in source {
            match line {
                SemanticRepr::BindingDef(def) => {
                    let symbol = SemanticSymbol::new(&def.name, Some(module_name));
                    if self.entries.contains_key(&symbol) {
                        return Err(CompilerError::DuplicateBindingDef(symbol));
                    }

                    self.tentative_entries.insert(symbol, def.clone());
                }
                SemanticRepr::Instruction(instruction) => {
                    for operand in &instruction.operands {
                        match operand {
                            SemanticOperand::Immediate(SemanticImmediateType::Binding(symbol, offset)) => {
                                let symbol = symbol.with_current_module(module_name);
                                if let Some(is_const) = self.const_deref_map.get(&symbol) && *is_const {
                                    return Err(CompilerError::MixedDynamicDerefOfConstantBinding(symbol));
                                }

                                let entry = self.get_tentative_entry_or_err(&symbol)?;
                                if entry.constant {
                                    self.const_deref_map.insert(symbol, false);
                                }
                            }
                            SemanticOperand::Deref(deref) => {
                                if let SemanticDerefKind::Binding(symbol) = &deref.kind {
                                    let symbol = symbol.with_current_module(module_name);
                                    if let Some(is_const) = self.const_deref_map.get(&symbol) && *is_const {
                                        return Err(CompilerError::MixedDynamicDerefOfConstantBinding(symbol));
                                    }

                                    let entry = self.get_tentative_entry_or_err(&symbol)?;
                                    if entry.constant {
                                        self.const_deref_map.insert(symbol, false);
                                    }
                                }
                            }
                            SemanticOperand::Immediate(SemanticImmediateType::ConstDerefBinding(symbol)) => {
                                let symbol = symbol.with_current_module(module_name);
                                if let Some(is_const) = self.const_deref_map.get(&symbol) && !is_const {
                                    return Err(CompilerError::MixedConstDerefOfConstantBinding(symbol));
                                }

                                // inline the binding's value
                                let entry = self.get_tentative_entry_or_err(&symbol)?;
                                if entry.values.len() != 1 {
                                    return Err(CompilerError::ConstDerefWithMultipleValues(symbol));
                                }

                                if !entry.constant {
                                    return Err(CompilerError::ConstDerefOfNonConstantBinding(symbol));
                                }
                                self.const_deref_map.insert(symbol, true);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }

        }
        Ok(())
    }

    pub fn append_bindings(&mut self, source: &SemanticSource, module_name: &str) -> CompileResult<()> {
        let mut section_open = false;

        for (i, line) in source.iter().enumerate() {
            match line {
                SemanticRepr::SpecialSection(label) => {
                    if label == tokenizer::SPECIAL_SECTION_LABEL_DATA {
                        if section_open {
                            return Err(CompilerError::DuplicateDataSection);
                        }

                        section_open = true;

                    } else {
                        section_open = false;
                    }
                }
                SemanticRepr::BindingDef(def) => {
                    if !section_open {
                        return Err(CompilerError::BindingDefInNonDataSection);
                    }

                    let symbol = SemanticSymbol::new(&def.name, Some(module_name));

                    if self.entries.contains_key(&symbol) {
                        return Err(CompilerError::DuplicateBindingDef(symbol));
                    }

                    let offset = if !self.can_inline(&symbol) {
                        let value = self.counter;
                        self.counter += def.total_size();
                        Some(value)

                    } else {
                        None
                    };

                    let entry = BindingTableEntry {
                        def: def.clone(),
                        offset,
                    };
                    self.entries.insert(symbol, entry);
                }
                _ => {
                    section_open = false;
                }
            }
        }

        Ok(())
    }

    pub fn resolve_bindings(&mut self, source: &mut SemanticSource, module_name: &str) -> CompileResult<()> {
        for line in source.iter_mut() {
            let SemanticRepr::Instruction(instruction) = line else {
                continue;
            };

            for operand in instruction.operands.iter_mut() {
                match operand {
                    SemanticOperand::Immediate(SemanticImmediateType::Binding(symbol, offset)) => {
                        let symbol = symbol.with_current_module(module_name);

                        // inline the binding's address
                        let addr = self.get_offset_or_err(&symbol)?.unwrap();
                        let value = addr as u64 + *offset as u64;

                        *operand = SemanticOperand::Immediate(SemanticImmediateType::from_u64_untyped(value));
                    }
                    SemanticOperand::Deref(deref) => {
                        if let SemanticDerefKind::Binding(symbol) = &deref.kind {
                            let symbol = symbol.with_current_module(module_name);

                            // inline the binding's address
                            let addr = self.get_offset_or_err(&symbol)?.unwrap();
                            let value = addr as u64 + deref.offset as u64;

                            *operand = SemanticOperand::Deref(SemanticDeref {
                                kind: SemanticDerefKind::Address(value),
                                offset: 0,
                            });
                        }
                    }
                    SemanticOperand::Immediate(SemanticImmediateType::ConstDerefBinding(symbol)) => {
                        let symbol = symbol.with_current_module(module_name);
                        // inline the binding's value
                        let entry = self.get_entry_or_err(&symbol)?;
                        let value = &entry.values[0];

                        *operand = SemanticOperand::Immediate(SemanticImmediateType::Literal(value.clone()));
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    pub fn get_offset(&self, symbol: &SemanticSymbol) -> Option<Option<usize>> {
        self.entries.get(symbol)
            .map(|c| c.offset)
    }

    pub fn get_offset_or_err(&self, symbol: &SemanticSymbol) -> CompileResult<Option<usize>> {
        self.get_offset(symbol)
            .ok_or_else(|| CompilerError::UndefinedBinding(symbol.clone()))
    }

    pub fn get_entry(&self, symbol: &SemanticSymbol) -> Option<&SemanticBindingDef> {
        self.entries.get(symbol)
            .map(|c| &c.def)
    }

    pub fn get_entry_or_err(&self, symbol: &SemanticSymbol) -> CompileResult<&SemanticBindingDef> {
        self.get_entry(symbol)
            .ok_or_else(|| CompilerError::UndefinedBinding(symbol.clone()))
    }

    fn get_tentative_entry(&self, symbol: &SemanticSymbol) -> Option<&SemanticBindingDef> {
        self.tentative_entries.get(symbol)
    }

    fn get_tentative_entry_or_err(&self, symbol: &SemanticSymbol) -> CompileResult<&SemanticBindingDef> {
        self.get_tentative_entry(symbol)
            .ok_or_else(|| CompilerError::UndefinedBinding(symbol.clone()))
    }

    fn can_inline(&self, symbol: &SemanticSymbol) -> bool {
        if let Some(b) = self.const_deref_map.get(symbol) && *b {
            return true;
        }
        false
    }
}