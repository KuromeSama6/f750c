//! This module defines functions and logic for the bytecode emission stage of the F750 compiler.

use std::collections::HashMap;
use indexmap::IndexMap;
use thiserror::Error;
use crate::bytecode::{BytecodeSerialize, BytecodeStream};
use crate::semantic::{SemanticBindingDef, SemanticDeref, SemanticDerefKind, SemanticImmediateType, SemanticLiteral, SemanticOperand, SemanticRepr, SemanticSource, SemanticSymbol};
use crate::tokenizer;
use crate::value::SymbolName;

#[derive(Debug, Error)]
pub enum CompilerError {
    #[error("Data section start not allowed here")]
    DuplicateDataSection,
    #[error("Binding definition found in non-data section")]
    BindingDefInNonDataSection,
    #[error("Duplicate binding definition: {0}")]
    DuplicateBindingDef(SymbolName),
    #[error("Use of undefined binding: {0}")]
    UndefinedBinding(SymbolName),
    #[error("Constant dereference of a binding with multiple values is not allowed: {0}")]
    ConstDerefWithMultipleValues(SymbolName),
    #[error("Constant dereference of a dynamic binding is not allowed: '{0}'. Either use dynamic dereference (&binding) or mark the binding as constant (const binding: ...).")]
    ConstDerefOfNonConstantBinding(SymbolName),
    #[error("Constant dereference (&const binding) of a constant binding '{0}' occured, but this binding was previously dynamically dereferenced (&binding) or directly accessed (binding).")]
    MixedConstDerefOfConstantBinding(SymbolName),
    #[error("Dynamic dereference (&binding) or direct access (binding) of a constant binding '{0}' occured, but this binding was previously constantly dereferenced (&const binding).")]
    MixedDynamicDerefOfConstantBinding(SymbolName),

    #[error("Duplicate label definition: {0}")]
    DuplicateLabel(SymbolName),
    #[error("Use of undefined label: {0}")]
    UndefinedLabel(SymbolName),
}
pub type CompileResult<T> = Result<T, CompilerError>;

#[derive(Debug, Default)]
pub struct BindingTable {
    entries: IndexMap<SymbolName, BindingTableEntry>,
    tentative_entries: HashMap<SymbolName, SemanticBindingDef>,
    counter: usize,
    const_deref_map: HashMap<SymbolName, bool>,
}

#[derive(Debug, Clone)]
pub struct BindingTableEntry {
    pub def: SemanticBindingDef,
    pub alloc: BindingAllocationType,
}

#[derive(Debug, Clone)]
pub enum BindingAllocationType {
    Inline,
    Extern,
    /// The binding is allocated at a static offset in the data section.
    Static(usize),
}

impl BindingTable {
    pub fn new() -> Self {
        BindingTable {
            entries: IndexMap::new(),
            tentative_entries: HashMap::new(),
            counter: 0,
            const_deref_map: HashMap::new(),
        }
    }

    pub fn parse_and_lower(lines: &mut IndexMap<String, Vec<SemanticRepr>>) -> CompileResult<Self> {
        let mut ret = Self::new();

        for (module_name, source) in lines.iter() {
            ret.preprocess_bindings(source, module_name)?;
        }

        for (module_name, source) in lines.iter() {
            ret.append_bindings(source, module_name)?;
        }

        // binding inlining
        for (module_name, source) in lines.iter_mut() {
            ret.lower_bindings(source, module_name)?;
        }
        
        Ok(ret)
    }
    
    fn preprocess_bindings(&mut self, source: &SemanticSource, module_name: &str) -> CompileResult<()> {
        for line in source {
            match line {
                SemanticRepr::BindingDef(def) => {
                    let symbol = SymbolName::new(&def.name, Some(module_name));
                    if self.entries.contains_key(&symbol) {
                        return Err(CompilerError::DuplicateBindingDef(symbol));
                    }

                    self.tentative_entries.insert(symbol.clone(), def.clone());
                }
                SemanticRepr::Instruction(instruction) => {
                    for operand in &instruction.operands {
                        // external symbol discovery
                        if let Some(external_symbol) = operand.get_external_symbol() {
                            let def = SemanticBindingDef {
                                name: external_symbol.name.to_string(),
                                constant: false,
                                values: vec![],
                            };

                            if !self.entries.contains_key(&external_symbol.name) {
                                self.tentative_entries.insert(external_symbol.name.clone(), def.clone());

                                let entry = BindingTableEntry {
                                    def,
                                    alloc: BindingAllocationType::Extern,
                                };
                                self.entries.insert(external_symbol.name.clone(), entry);
                            }
                        }

                        match operand {
                            SemanticOperand::Immediate(SemanticImmediateType::Binding(symbol, offset)) => {
                                let symbol = symbol.name.with_current_module(module_name);
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
                                    let symbol = symbol.name.with_current_module(module_name);
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
                                let symbol = symbol.name.with_current_module(module_name);
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

    fn append_bindings(&mut self, source: &SemanticSource, module_name: &str) -> CompileResult<()> {
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

                    let symbol = SymbolName::new(&def.name, Some(module_name));

                    if self.entries.contains_key(&symbol) {
                        return Err(CompilerError::DuplicateBindingDef(symbol));
                    }

                    let alloc = if !self.can_inline(&symbol) {
                        let value = self.counter;
                        self.counter += def.total_size();
                        BindingAllocationType::Static(value)

                    } else {
                        BindingAllocationType::Inline
                    };

                    let entry = BindingTableEntry {
                        def: def.clone(),
                        alloc,
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

    fn lower_bindings(&mut self, source: &mut SemanticSource, module_name: &str) -> CompileResult<()> {
        for line in source.iter_mut() {
            let SemanticRepr::Instruction(instruction) = line else {
                continue;
            };

            for operand in instruction.operands.iter_mut() {
                if let SemanticOperand::Immediate(SemanticImmediateType::ConstDerefBinding(symbol)) = operand {
                    let symbol = symbol.name.with_current_module(module_name);
                    // inline the binding's value
                    let entry = self.get_entry_or_err(&symbol)?;
                    let value = &entry.values[0];

                    *operand = SemanticOperand::Immediate(SemanticImmediateType::Literal(value.clone()));
                }
            }
        }

        Ok(())
    }

    pub fn get_offset(&self, symbol: &SymbolName) -> Option<Option<usize>> {
        self.entries.get(symbol)
            .map(|c| match c.alloc {
                BindingAllocationType::Static(offset) => Some(offset),
                _ => None,
            })
    }

    pub fn get_offset_or_err(&self, symbol: &SymbolName) -> CompileResult<Option<usize>> {
        self.get_offset(symbol)
            .ok_or_else(|| CompilerError::UndefinedBinding(symbol.clone()))
    }

    pub fn get_entry(&self, symbol: &SymbolName) -> Option<&SemanticBindingDef> {
        self.entries.get(symbol)
            .map(|c| &c.def)
    }

    pub fn get_entry_or_err(&self, symbol: &SymbolName) -> CompileResult<&SemanticBindingDef> {
        self.get_entry(symbol)
            .ok_or_else(|| CompilerError::UndefinedBinding(symbol.clone()))
    }

    fn get_tentative_entry(&self, symbol: &SymbolName) -> Option<&SemanticBindingDef> {
        self.tentative_entries.get(symbol)
    }

    fn get_tentative_entry_or_err(&self, symbol: &SymbolName) -> CompileResult<&SemanticBindingDef> {
        self.get_tentative_entry(symbol)
            .ok_or_else(|| CompilerError::UndefinedBinding(symbol.clone()))
    }

    fn can_inline(&self, symbol: &SymbolName) -> bool {
        if let Some(b) = self.const_deref_map.get(symbol) && *b {
            return true;
        }
        false
    }
}

impl BytecodeSerialize for BindingTable {
    fn serialize(&self, stream: &mut BytecodeStream) {
        let mut entries = Vec::new();
        let mut current_namespace: Option<&str> = None;

        for (i, (symbol, entry)) in self.entries.iter().enumerate() {
            entries.push((symbol, entry));

            let mut flag = 0u8;
            // const
            if entry.def.constant {
                flag |= 1 << 0;
            }
            // extern
            if matches!(entry.alloc, BindingAllocationType::Extern) {
                flag |= 1 << 1;
            }
            // end
            if i == self.entries.len() - 1 {
                flag |= 1 << 2;
            }

            let namespace = symbol.namespace.as_ref().unwrap();
            let write_namespace = current_namespace != Some(namespace);
            if write_namespace {
                flag |= 1 << 3;
                current_namespace = Some(namespace);
            }

            stream.write_u8(flag);
            stream.write_varint64(i as u64);
            if write_namespace {
                stream.write_cstr(namespace);
            }

            stream.write_cstr(&symbol.name);

            if let BindingAllocationType::Static(offset) = entry.alloc {
                stream.write_varint64(offset as u64);
            }
        }

        // Data section
        let size: usize = self.entries.values()
            .filter(|c| matches!(c.alloc, BindingAllocationType::Static(_)))
            .map(|c| c.def.total_size())
            .sum();
        stream.write_varint64(size as u64);

        for entry in self.entries.values() {
            if let BindingAllocationType::Static(_) = entry.alloc {
                entry.def.serialize(stream);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LabelTable {
    entries: IndexMap<SymbolName, LabelTableEntry>,
}

#[derive(Debug, Clone)]
struct LabelTableEntry {
    symbol: SemanticSymbol,
    addr: u64,
}

impl LabelTable {
    pub fn parse(lines: &IndexMap<String, Vec<SemanticRepr>>) -> CompileResult<Self> {
        let mut entries = IndexMap::new();
        let mut addr_counter = 0u64;

        // parse
        for (module_name, source) in lines {
            for line in source {
                match line {
                    SemanticRepr::Label(symbol) => {
                        if entries.contains_key(&symbol.name) {
                            return Err(CompilerError::DuplicateLabel(symbol.name.clone()));
                        }

                        let name = symbol.name.with_current_module(module_name);
                        let entry = LabelTableEntry {
                            symbol: SemanticSymbol::new(name.clone(), false),
                            addr: addr_counter,
                        };
                        entries.insert(name, entry);
                    }
                    SemanticRepr::Instruction(_) => {
                        addr_counter += 1;
                    }
                    _ => {}
                }
            }
        }

        // lower
        for (module_name, source) in lines {
            for line in source {
                if let SemanticRepr::Instruction(instruction) = line {
                    for operand in &instruction.operands {
                        // external label discovery
                        if let Some(external) = operand.get_external_symbol() {
                            if !entries.contains_key(&external.name) {
                                let entry = LabelTableEntry {
                                    symbol: SemanticSymbol::new(external.name.clone(), true),
                                    addr: 0,
                                };
                                entries.insert(external.name.clone(), entry);
                            }
                        }

                        // process operand
                        if let SemanticOperand::Immediate(SemanticImmediateType::Label(symbol, offset)) = operand {
                            let name = symbol.name.with_current_module(module_name);
                            if !entries.contains_key(&name) {
                                return Err(CompilerError::UndefinedLabel(name));
                            }
                        }
                    }
                }
            }
        }

        Ok(Self {
            entries,
        })
    }
}

impl BytecodeSerialize for LabelTable {
    fn serialize(&self, stream: &mut BytecodeStream) {
        let mut entries = Vec::new();
        let mut current_namespace: Option<&str> = None;

        for (i, (symbol, entry)) in self.entries.iter().enumerate() {
            entries.push((symbol, entry));

            let mut flag = 0u8;
            // extern
            if entry.symbol.external {
                flag |= 1 << 0;
            }
            // end
            if i == self.entries.len() - 1 {
                flag |= 1 << 1;
            }

            let namespace = symbol.namespace.as_ref().unwrap();
            let write_namespace = current_namespace != Some(namespace);
            if write_namespace {
                flag |= 1 << 2;
                current_namespace = Some(namespace);
            }

            stream.write_u8(flag);
            stream.write_varint64(i as u64);
            if write_namespace {
                stream.write_cstr(namespace);
            }

            stream.write_cstr(&symbol.name);

            if !entry.symbol.external {
                stream.write_varint64(entry.addr);
            }
        }
    }
}