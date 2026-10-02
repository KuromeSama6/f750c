//! This module defines functions and logic for the bytecode emission stage of the F750 compiler.

use std::collections::HashMap;
use std::fmt::Debug;
use indexmap::IndexMap;
use log::info;
use thiserror::Error;
use crate::bytecode::{BytecodeSerialize, BytecodeStream};
use crate::semantic::{SemanticBindingDef, SemanticDeref, SemanticDerefKind, SemanticImmediateType, SemanticInstruction, SemanticLiteral, SemanticOperand, SemanticRepr, SemanticSource, SemanticSymbol};
use crate::tokenizer;
use crate::value::SymbolName;

pub trait SymbolTable: Debug {
    fn get_symbol_ident(&self, symbol: &SymbolName) -> Option<u64>;
}

#[derive(Debug, Error)]
pub enum CompilerError {
    #[error("Binding table error: {0}")]
    BindingTable(#[from] BindingTableError),

    #[error("Duplicate label definition: {0}")]
    DuplicateLabel(SymbolName),
    #[error("Use of undefined label: {0}")]
    UndefinedLabel(SymbolName),
}
pub type CompileResult<T> = Result<T, CompilerError>;

#[derive(Debug, Default)]
pub struct BindingTable {
    entries: IndexMap<SymbolName, BindingTableEntry>,
    tentative_entries: IndexMap<SymbolName, SemanticBindingDef>,
    counter: usize,
    const_deref_map: HashMap<SymbolName, bool>,
}

impl SymbolTable for BindingTable {
    fn get_symbol_ident(&self, symbol: &SymbolName) -> Option<u64> {
        self.entries.get_index_of(symbol)
            .map(|i| i as u64)
    }
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

#[derive(Debug, Error)]
pub enum BindingTableError {
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
    #[error("Invalid external access of binding: {0}. External bindings must be accessed with 'extern', and local bindings must be accessed without 'extern'.")]
    InvalidExternAccess(SymbolName),
}
pub type BindingTableResult<T> = Result<T, BindingTableError>;

impl BindingTable {
    pub fn new() -> Self {
        BindingTable {
            entries: IndexMap::new(),
            tentative_entries: IndexMap::new(),
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
    
    fn preprocess_bindings(&mut self, source: &SemanticSource, module_name: &str) -> BindingTableResult<()> {
        for line in source {
            match line {
                SemanticRepr::BindingDef(def) => {
                    let symbol = SymbolName::new(&def.name, Some(module_name));
                    if self.entries.contains_key(&symbol) {
                        return Err(BindingTableError::DuplicateBindingDef(symbol));
                    }

                    self.tentative_entries.insert(symbol.clone(), def.clone());
                }
                SemanticRepr::Instruction(instruction) => {
                    for operand in &instruction.operands {
                        // external symbol discovery
                        let mut is_external_access = false;
                        if operand.is_binding() && let Some(external_symbol) = operand.get_external_symbol() {
                            let def = SemanticBindingDef {
                                name: external_symbol.name.to_string(),
                                constant: external_symbol.is_const,
                                public: false,
                                values: vec![],
                            };
                            is_external_access = true;

                            if !self.entries.contains_key(&external_symbol.name) {
                                self.tentative_entries.insert(external_symbol.name.clone(), def.clone());

                                let entry = BindingTableEntry {
                                    def,
                                    alloc: BindingAllocationType::Extern,
                                };
                                self.entries.insert(external_symbol.name.clone(), entry);

                                if external_symbol.is_deref {
                                    self.const_deref_map.insert(external_symbol.name, external_symbol.is_const);
                                }
                            }
                        }

                        match operand {
                            SemanticOperand::Immediate(SemanticImmediateType::Binding(symbol, offset)) => {
                                let symbol = symbol.name.with_current_module(module_name);
                                if let Some(is_const) = self.const_deref_map.get(&symbol) && *is_const {
                                    return Err(BindingTableError::MixedDynamicDerefOfConstantBinding(symbol));
                                }

                                if !self.ensure_valid_external_ref(&symbol, is_external_access) {
                                    return Err(BindingTableError::InvalidExternAccess(symbol));
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
                                        return Err(BindingTableError::MixedDynamicDerefOfConstantBinding(symbol));
                                    }

                                    if !self.ensure_valid_external_ref(&symbol, is_external_access) {
                                        return Err(BindingTableError::InvalidExternAccess(symbol));
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
                                    return Err(BindingTableError::MixedConstDerefOfConstantBinding(symbol));
                                }

                                if !self.ensure_valid_external_ref(&symbol, is_external_access) {
                                    return Err(BindingTableError::InvalidExternAccess(symbol));
                                }

                                // inline the binding's value
                                let entry = self.get_tentative_entry_or_err(&symbol)?;

                                // external check
                                if let Some(entry) = self.entries.get(&symbol) && matches!(entry.alloc, BindingAllocationType::Extern) {
                                    continue;
                                }

                                if entry.values.len() != 1 {
                                    return Err(BindingTableError::ConstDerefWithMultipleValues(symbol));
                                }

                                if !entry.constant {
                                    return Err(BindingTableError::ConstDerefOfNonConstantBinding(symbol));
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

    fn append_bindings(&mut self, source: &SemanticSource, module_name: &str) -> BindingTableResult<()> {
        let mut section_open = false;

        for (i, line) in source.iter().enumerate() {
            match line {
                SemanticRepr::SpecialSection(label) => {
                    if label == tokenizer::SPECIAL_SECTION_LABEL_DATA {
                        if section_open {
                            return Err(BindingTableError::DuplicateDataSection);
                        }

                        section_open = true;

                    } else {
                        section_open = false;
                    }
                }
                SemanticRepr::BindingDef(def) => {
                    if !section_open {
                        return Err(BindingTableError::BindingDefInNonDataSection);
                    }

                    let symbol = SymbolName::new(&def.name, Some(module_name));

                    if self.entries.contains_key(&symbol) {
                        return Err(BindingTableError::DuplicateBindingDef(symbol));
                    }

                    let alloc = if !self.can_inline(&symbol, def) {
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
                // replace with fully qualified symbol names
                operand.fully_qualify_symbols(module_name);

                if let SemanticOperand::Immediate(SemanticImmediateType::ConstDerefBinding(symbol)) = operand {
                    let symbol = symbol.name.with_current_module(module_name);
                    // inline the binding's value
                    let entry = self.get_entry_or_err(&symbol)?;

                    // external check
                    if let Some(entry) = self.entries.get(&symbol) && matches!(entry.alloc, BindingAllocationType::Extern) {
                        continue;
                    }

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

    pub fn get_offset_or_err(&self, symbol: &SymbolName) -> BindingTableResult<Option<usize>> {
        self.get_offset(symbol)
            .ok_or_else(|| BindingTableError::UndefinedBinding(symbol.clone()))
    }

    pub fn get_entry(&self, symbol: &SymbolName) -> Option<&SemanticBindingDef> {
        self.entries.get(symbol)
            .map(|c| &c.def)
    }

    pub fn get_entry_or_err(&self, symbol: &SymbolName) -> BindingTableResult<&SemanticBindingDef> {
        self.get_entry(symbol)
            .ok_or_else(|| BindingTableError::UndefinedBinding(symbol.clone()))
    }

    fn get_tentative_entry(&self, symbol: &SymbolName) -> Option<&SemanticBindingDef> {
        self.tentative_entries.get(symbol)
    }

    fn get_tentative_entry_or_err(&self, symbol: &SymbolName) -> BindingTableResult<&SemanticBindingDef> {
        self.get_tentative_entry(symbol)
            .ok_or_else(|| BindingTableError::UndefinedBinding(symbol.clone()))
    }

    fn can_inline(&self, symbol: &SymbolName, def: &SemanticBindingDef) -> bool {
        if def.public {
            return false;
        }

        if def.values.len() != 1 {
            return false;
        }

        if let Some(b) = self.const_deref_map.get(symbol) && *b {
            return true;
        }

        def.constant
    }

    fn ensure_valid_external_ref(&self, name: &SymbolName, extern_access: bool) -> bool {
        let Some(entry) = self.entries.get(name) else {
            // this has to be a non-external access
            return !extern_access;
        };

        let is_extern = matches!(entry.alloc, BindingAllocationType::Extern);
        is_extern == extern_access
    }
}

impl BytecodeSerialize for BindingTable {
    fn serialize(&self, stream: &mut BytecodeStream) {
        let mut entries = Vec::new();
        let mut current_namespace: Option<&str> = None;

        let mut written = 0usize;
        for (i, (symbol, entry)) in self.entries.iter().enumerate() {
            if matches!(entry.alloc, BindingAllocationType::Inline) {
                continue;
            }
            written += 1;

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

            if entry.def.public {
                flag |= 1 << 4;
            }

            stream.write_u8(flag);
            stream.write_varint64(self.get_symbol_ident(symbol).unwrap());
            if write_namespace {
                stream.write_cstr(namespace);
            }

            stream.write_cstr(&symbol.name);

            if let BindingAllocationType::Static(offset) = entry.alloc {
                stream.write_varint64(offset as u64);
            }
        }

        if written == 0 {
            info!("Binding table is empty, skipping data section");
            info!("Note: This is expected if all bindings are inlined or if there are no bindings defined.");

            stream.write_u8(1 << 5); // end
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

impl SymbolTable for LabelTable {
    fn get_symbol_ident(&self, symbol: &SymbolName) -> Option<u64> {
        self.entries.get_index_of(symbol)
            .map(|i| i as u64)
    }
}

#[derive(Debug, Clone)]
struct LabelTableEntry {
    symbol: SemanticSymbol,
    addr: u64,
}

impl LabelTable {
    pub fn parse_and_lower(lines: &mut IndexMap<String, Vec<SemanticRepr>>) -> CompileResult<Self> {
        let mut entries = IndexMap::new();
        let mut addr_counter = 0u64;

        // parse
        for (module_name, source) in lines.iter() {
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
        for (module_name, source) in lines.iter_mut() {
            for line in source {
                if let SemanticRepr::Instruction(instruction) = line {
                    for operand in instruction.operands.iter_mut() {
                        operand.fully_qualify_symbols(module_name);

                        // external label discovery
                        if operand.is_label() && let Some(external) = operand.get_external_symbol() {
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

        if self.entries.is_empty() {
            info!("Note: Label table is empty, skipping label section");
            stream.write_u8(1 << 3);
            return;
        }

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
            stream.write_varint64(self.get_symbol_ident(symbol).unwrap());
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

#[derive(Debug, Clone)]
pub struct EngCallTable {
    pub entries: IndexMap<String, u64>,
}

impl EngCallTable {
    pub fn new() -> Self {
        EngCallTable {
            entries: IndexMap::new(),
        }
    }

    pub fn parse(lines: &IndexMap<String, Vec<SemanticRepr>>) -> CompileResult<Self> {
        let mut entries = IndexMap::new();
        let mut count = 0u64;

        for (module_name, source) in lines {
            for line in source {
                let SemanticRepr::Instruction(instruction) = line else {
                    continue;
                };

                for operand in &instruction.operands {
                    if let SemanticOperand::EngineParam(param) = operand {
                        if !entries.contains_key(param) {
                            entries.insert(param.clone(), count);
                            count += 1;
                        }
                    }
                }
            }
        }

        Ok(Self {
            entries,
        })
    }

    pub fn get(&self, name: &str) -> Option<u64> {
        self.entries.get(name).copied()
    }
}

impl BytecodeSerialize for EngCallTable {
    fn serialize(&self, stream: &mut BytecodeStream) {
        let mut entries: Vec<(&String, &u64)> = self.entries.iter().collect();
        entries.sort_by(|a, b| a.1.cmp(b.1));

        if entries.is_empty() {
            stream.write_u8(1 << 1);
            return;
        }

        for (i, (name, id)) in entries.iter().enumerate() {
            let mut flag = 0u8;
            if i == entries.len() - 1 {
                flag |= 1 << 0;
            }

            stream.write_u8(flag);
            stream.write_varint64(**id);
            stream.write_cstr(name);
        }
    }
}