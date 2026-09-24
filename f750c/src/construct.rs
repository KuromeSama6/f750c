//! This module contains logic for the compiler construct expansion phase.

use std::fmt::{Display, Formatter};
use std::mem;
use log::warn;
use thiserror::Error;
use crate::opcode::{CompilerConstruct, OpcodeMnemonic, Register};
use crate::semantic::{SemanticCompilerConstruct, SemanticDeref, SemanticDerefKind, SemanticImmediateType, SemanticInstruction, SemanticOperand, SemanticRepr, SemanticReprStream, SemanticSymbol};
use crate::{tokenizer, util};
use crate::value::{RegisterSpec};

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
    #[error("Construct '{0}' not allowed here.")]
    InvalidConstruct(CompilerConstruct),
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
    },
    #[error("Unterminated block starting at line {start_line} with construct '{start_construct}'.")]
    UnterminatedBlock {
        start_line: usize,
        start_construct: SemanticCompilerConstruct,
    },
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

#[derive(Debug, Clone)]
enum BlockConstruct {
    Conditional(BlockConstructConditional),
}

impl BlockConstruct {
    pub fn expand(&self, out: &mut Vec<SemanticRepr>) {
        match self {
            BlockConstruct::Conditional(cond_block) => cond_block.expand(out),
        }
    }
}

#[derive(Debug, Clone)]
enum BlockBodyLine {
    Line(SemanticRepr),
    NestedBlock(BlockConstruct),
}
type BlockBody = Vec<BlockBodyLine>;

#[derive(Debug, Clone)]
struct BlockConstructConditional {
    if_statement: SemanticCompilerConstruct,
    if_body: BlockBody,
    else_if_statements: Vec<(SemanticCompilerConstruct, BlockBody)>,
    else_statement: Option<BlockBody>
}

impl BlockConstructConditional {
    fn parse(source: &[SemanticRepr], source_line: usize, construct: &SemanticCompilerConstruct) -> Result<(Self, usize), ConstructExpansionErrorDetails> {
        Self::validate_statement_args(&construct.operands)
            .map_err(|e| e.into_details(&SemanticRepr::CompilerConstruct(construct.clone()), source_line))?;

        let mut builder = ConditionalBlockBuilder::default();
        let mut normal_end = false;
        let mut i = 0usize;

        loop {
            if i >= source.len() {
                break;
            }
            let line = &source[i];

            let mut single_buf = Vec::new();

            let SemanticRepr::CompilerConstruct(construct) = line else {
                let line = BlockBodyLine::Line(line.clone());
                builder.push_line(line);
                i += 1;
                continue;
            };

            if construct.opcode == CompilerConstruct::BlockEnd {
                normal_end = true;
                i += 1;
                break;
            }

            if construct.opcode.is_block_start() {
                let (nested_block, offset) = parse_block_body(&source[i+1..], source_line + i + 1, construct)?;
                let line = BlockBodyLine::NestedBlock(nested_block);
                builder.push_line(line);
                i += offset + 1;
                continue;
            }

            // handle else if and else constructs
            if construct.opcode == CompilerConstruct::Else {
                // if we are already in an else block, this is an error
                if builder.is_else() {
                    println!("Error: Already in an else block, cannot have an else if. {:?}", builder.has_else);
                    return Err(ConstructExpansionError::InvalidConstruct(construct.opcode).into_details(line, source_line + i));
                }

                if construct.operands.len() != 0 {
                    return Err(ConstructExpansionError::ExpectedExactOperands {
                        opcode: CompilerConstruct::Else,
                        expected: 0,
                    }.into_details(line, source_line + i));
                }

                builder.has_else = true;
                i += 1;
                continue;
            }

            if construct.opcode == CompilerConstruct::ElseIf {
                // if we are already in an else block, this is an error
                if builder.is_else() {
                    return Err(ConstructExpansionError::InvalidConstruct(construct.opcode).into_details(line, source_line + i));
                }

                Self::validate_statement_args(&construct.operands)
                    .map_err(|e| e.into_details(&SemanticRepr::CompilerConstruct(construct.clone()), source_line))?;

                builder.next_elif(construct.clone());
                i += 1;
                continue;
            }

            // parse single
            if let Err(e) = expand_construct_single(construct, &mut single_buf) {
                return Err(e.into_details(line, source_line + i));
            }

            for item in single_buf {
                let line = BlockBodyLine::Line(item);
                builder.push_line(line);
            }

            i += 1;
        }

        // handle the result
        if normal_end {
            Ok((builder.build(construct), i))

        } else {
            return Err(ConstructExpansionError::UnterminatedBlock {
                start_line: source_line,
                start_construct: construct.clone(),
            }.into_details(&SemanticRepr::CompilerConstruct(construct.clone()), source_line));
        }
    }

    fn expand(&self, out: &mut Vec<SemanticRepr>) {
        // 1. symbols
        let id = util::random_internal_label();
        let if_symbol: SemanticSymbol = format!("_cg_if_{id}").into();
        let elif_symbols: Vec<SemanticSymbol> = self.else_if_statements.iter()
            .enumerate()
            .map(|(i, _)| format!("_cg_elif_{id}_{i}").into())
            .collect();
        let else_symbol: SemanticSymbol = format!("_cg_else_{id}").into();
        let endif_symbol: SemanticSymbol = format!("_cg_endif_{id}").into();

        Self::push_statement_jump(out, &self.if_statement, &if_symbol);
        for (i, (elif_statement, _)) in self.else_if_statements.iter().enumerate() {
            Self::push_statement_jump(out, elif_statement, &elif_symbols[i]);
        }

        if self.else_statement.is_some() {
            out.push(SemanticRepr::Instruction(SemanticInstruction {
                opcode: OpcodeMnemonic::Jmp,
                operands: vec![SemanticOperand::Immediate(SemanticImmediateType::Label(else_symbol.clone(), 0))],
            }));
        }

        // 2. body
        // if body
        Self::push_body(out, &if_symbol, &self.if_body, &endif_symbol);
        // elif bodies
        for (i, (_, elif_body)) in self.else_if_statements.iter().enumerate() {
            Self::push_body(out, &elif_symbols[i], elif_body, &endif_symbol);
        }
        // else body
        if let Some(else_body) = &self.else_statement {
            Self::push_body(out, &else_symbol, else_body, &endif_symbol);
        }

        // 3. end
        out.push(SemanticRepr::Label(endif_symbol));
    }

    fn push_statement_jump(out: &mut Vec<SemanticRepr>, construct: &SemanticCompilerConstruct, target: &SemanticSymbol) {
        let operands = &construct.operands;
        if operands.len() < 3 {
            panic!("Invalid number of operands for if construct: expected at least 2, got {}", operands.len());
        }

        out.push(SemanticRepr::Instruction(SemanticInstruction {
            opcode: OpcodeMnemonic::Cmp,
            operands: vec![operands[0].clone(), operands[1].clone()],
        }));

        let SemanticOperand::Mnemonic(mnemonic) = &operands[2] else {
            panic!("Invalid third operand for if construct: expected a jump mnemonic, got {:?}", operands[2]);
        };
        out.push(SemanticRepr::Instruction(SemanticInstruction {
            opcode: *mnemonic,
            operands: vec![SemanticOperand::Immediate(SemanticImmediateType::Label(target.clone(), 0))],
        }));
    }

    fn push_body(out: &mut Vec<SemanticRepr>, start_label: &SemanticSymbol, body: &BlockBody, end_label: &SemanticSymbol) {
        out.push(SemanticRepr::Label(start_label.clone()));
        for line in body {
            match line {
                BlockBodyLine::Line(line) => out.push(line.clone()),
                BlockBodyLine::NestedBlock(block) => block.expand(out),
            }
        }
        out.push(SemanticRepr::Instruction(SemanticInstruction {
            opcode: OpcodeMnemonic::Jmp,
            operands: vec![SemanticOperand::Immediate(SemanticImmediateType::Label(end_label.clone(), 0))],
        }));

    }

    fn validate_statement_args(args: &[SemanticOperand]) -> Result<(), ConstructExpansionError> {
        if args.len() != 3 {
            return Err(ConstructExpansionError::ExpectedExactOperands {
                opcode: CompilerConstruct::If,
                expected: 3,
            });
        }

        if !args[0].is_immediate_or_register() {
            return Err(ConstructExpansionError::InvalidOperand {
                index: 0,
                operand: args[0].clone(),
                reason: "Expected an immediate value or a register.".to_string(),
            });
        }

        if !args[1].is_immediate_or_register() {
            return Err(ConstructExpansionError::InvalidOperand {
                index: 1,
                operand: args[1].clone(),
                reason: "Expected an immediate value or a register.".to_string(),
            });
        }

        if let SemanticOperand::Mnemonic(mnemonic) = &args[2] && mnemonic.is_jump() {} else {
            return Err(ConstructExpansionError::InvalidOperand {
                index: 2,
                operand: args[2].clone(),
                reason: "Expected a jump mnemonic.".to_string(),
            });
        }

        Ok(())
    }
}

#[derive(Debug, Default)]
struct ConditionalBlockBuilder {
    if_body: BlockBody,
    elif_statements: Vec<(SemanticCompilerConstruct, BlockBody)>,
    cur_elif_statement: Option<SemanticCompilerConstruct>,
    cur_elif_body: BlockBody,
    has_else: bool,
    else_body: BlockBody,
}

impl ConditionalBlockBuilder {
    pub fn build(mut self, if_statement: &SemanticCompilerConstruct) -> BlockConstructConditional {
        if let Some(cur_elif_statement) = self.cur_elif_statement.take() {
            // push the current elif statement and body to the list of elif statements
            self.elif_statements.push((cur_elif_statement, mem::take(&mut self.cur_elif_body)));
        }

        let else_body = if self.has_else {
            Some(self.else_body)
        } else {
            None
        };

        BlockConstructConditional {
            if_statement: if_statement.clone(),
            if_body: self.if_body,
            else_if_statements: self.elif_statements,
            else_statement: else_body,
        }
    }

    pub fn push_line(&mut self, line: BlockBodyLine) {
        if self.cur_elif_statement.is_some() {
            // we are currently in an elif block
            self.cur_elif_body.push(line);

        } else if self.has_else {
            // we are currently in an else block
            self.else_body.push(line);

        } else {
            // we are currently in the if block
            self.if_body.push(line);
        }
    }

    pub fn next_elif(&mut self, elif_statement: SemanticCompilerConstruct) {
        if let Some(cur_elif_statement) = self.cur_elif_statement.take() {
            // push the current elif statement and body to the list of elif statements
            self.elif_statements.push((cur_elif_statement, mem::take(&mut self.cur_elif_body)));
        }

        // set the new elif statement as the current one
        self.cur_elif_statement = Some(elif_statement);
    }

    pub fn is_else(&self) -> bool {
        self.has_else
    }
}

pub fn expand_construct_source(source: &[SemanticRepr]) -> Result<Vec<SemanticRepr>, ConstructExpansionErrorDetails> {
    let mut ret = Vec::with_capacity(source.len());
    let mut data_section = false;
    let mut i = 0usize;

    loop {
        if i >= source.len() {
            break;
        }
        let line = &source[i];

        // toggle data section
        if let SemanticRepr::SpecialSection(name) = line && name == tokenizer::SPECIAL_SECTION_LABEL_DATA {
            data_section = true;
        }

        if matches!(line, SemanticRepr::Label(_)) || matches!(line, SemanticRepr::Instruction(_)) {
            data_section = false;
        }

        let SemanticRepr::CompilerConstruct(construct) = line else {
            ret.push(line.clone());
            i += 1;
            continue;
        };

        if data_section {
            return Err(ConstructExpansionError::ConstructInDataSection.into_details(line, i));
        }

        // blocks
        if construct.opcode.is_block_start() {
            if i + 1 >= source.len() {
                return Err(ConstructExpansionError::UnterminatedBlock {
                    start_line: i,
                    start_construct: construct.clone(),
                }.into_details(line, i));
            }

            let (block_body, offset) = parse_block_body(&source[i+1..], i, construct)?;
            block_body.expand(&mut ret);
            i += offset + 1;
            continue;
        }

        let result = expand_construct_single(construct, &mut ret);
        i += 1;

        if let Err(e) = result {
            return Err(e.into_details(line, i));
        }
    }

    Ok(ret)
}

fn parse_block_body(source: &[SemanticRepr], source_line: usize, construct: &SemanticCompilerConstruct) -> Result<(BlockConstruct, usize), ConstructExpansionErrorDetails> {
    match construct.opcode {
        CompilerConstruct::If => {
            let (block, offset) = BlockConstructConditional::parse(source, source_line, construct)?;
            Ok((BlockConstruct::Conditional(block), offset))
        },
        _ => Err(ConstructExpansionError::InvalidConstruct(construct.opcode).into_details(&SemanticRepr::CompilerConstruct(construct.clone()), source_line)),
    }
}

fn expand_construct_single(construct: &SemanticCompilerConstruct, out: &mut Vec<SemanticRepr>) -> Result<(), ConstructExpansionError> {
    match construct.opcode {
        CompilerConstruct::Pry => expand_construct_pry(construct, out),
        CompilerConstruct::GetArg => expand_construct_getarg(construct, out),
        CompilerConstruct::EngineCall => expand_construct_engcall(construct, out),
        _ => Err(ConstructExpansionError::InvalidConstruct(construct.opcode)),
    }
}

// @pry <binding>, <value>
fn expand_construct_pry(construct: &SemanticCompilerConstruct, out: &mut Vec<SemanticRepr>) -> Result<(), ConstructExpansionError> {
    warn!("Use of unsafe compiler construct '@pry': '{construct}'");

    construct.expect_exact_operands(2)?;

    let arg0 = &construct.operands[0];
    let SemanticOperand::Immediate(SemanticImmediateType::Binding(binding, offset)) = arg0 else {
        return construct.invalid_operand_error(0, "Expected a binding as the first operand.");
    };

    if *offset != 0 {
        return construct.invalid_operand_error(0, "This binding must not have an offset.");
    }

    let arg1 = &construct.operands[1];
    if !(arg1.is_immediate() || arg1.is_register()) {
        return construct.invalid_operand_error(1, "Expected an immediate value or a register.");
    }

    // generate code

    // mov rsi, <binding>
    out.push(SemanticRepr::Instruction(SemanticInstruction {
        opcode: OpcodeMnemonic::Mov,
        operands: vec![
            SemanticOperand::Register(RegisterSpec::qword(Register::SourceIndex)),
            SemanticOperand::Immediate(SemanticImmediateType::Binding(binding.clone(), 0)),
        ],
    }));

    // mov &rsi, <value>
    out.push(SemanticRepr::Instruction(SemanticInstruction {
        opcode: OpcodeMnemonic::Mov,
        operands: vec![
            SemanticOperand::Deref(SemanticDeref {
                kind: SemanticDerefKind::Register(RegisterSpec::qword(Register::SourceIndex)),
                offset: 0,
            }),
            arg1.clone(),
        ],
    }));

    Ok(())
}

// @getarg <register>, <offset>
fn expand_construct_getarg(construct: &SemanticCompilerConstruct, out: &mut Vec<SemanticRepr>) -> Result<(), ConstructExpansionError> {
    construct.expect_exact_operands(2)?;

    let arg0 = &construct.operands[0];
    let SemanticOperand::Register(reg) = arg0 else {
        return construct.invalid_operand_error(0, "Expected a register as the first operand.");
    };

    let arg1 = &construct.operands[1];
    let SemanticOperand::Immediate(SemanticImmediateType::Literal(lit)) = arg1 else {
        return construct.invalid_operand_error(1, "Expected a literal as the second operand.");
    };

    if lit.to_data_type().data_type().is_floating_point() {
        return construct.invalid_operand_error(1, "This literal must not be a integer value.");
    }

    let offset_amount = lit.to_data_type().as_int().unwrap();

    out.push(SemanticRepr::Instruction(SemanticInstruction {
        opcode: OpcodeMnemonic::Mov,
        operands: vec![
            SemanticOperand::Register(*reg),
            SemanticOperand::Deref(SemanticDeref {
                kind: SemanticDerefKind::Register(RegisterSpec::qword(Register::BasePointer)),
                offset: 16 + offset_amount,
            })
        ],
    }));

    Ok(())
}

fn expand_construct_engcall(construct: &SemanticCompilerConstruct, out: &mut Vec<SemanticRepr>) -> Result<(), ConstructExpansionError> {
    if construct.operands.len() < 1 {
        return Err(ConstructExpansionError::MinimumOperands {
            opcode: construct.opcode,
            minimum: 1,
        });
    }

    let arg0 = &construct.operands[0];
    let SemanticOperand::EngineParam(engine_param) = arg0 else {
        return construct.invalid_operand_error(0, "Expected an engine parameter as the first operand.");
    };

    let mut args: Vec<&SemanticOperand> = Vec::new();
    for (i, operand) in construct.operands.iter()
        .enumerate()
        .skip(1)
    {
        if matches!(operand, SemanticOperand::Immediate(_)) || matches!(operand, SemanticOperand::Register(_)) {
            args.push(operand);

        } else {
            return construct.invalid_operand_error(i, format!("For parameter {i} '{operand}': Expected an immediate or a register.").as_str());
        }
    }

    // generate code
    // mov a64, <engine_param>
    out.push(SemanticRepr::Instruction(SemanticInstruction {
        opcode: OpcodeMnemonic::Mov,
        operands: vec![
            SemanticOperand::Register(RegisterSpec::qword(Register::A)),
            SemanticOperand::EngineParam(engine_param.clone()),
        ],
    }));

    for arg in args {
        // push <arg>
        out.push(SemanticRepr::Instruction(SemanticInstruction {
            opcode: OpcodeMnemonic::Push,
            operands: vec![arg.clone()],
        }));
    }

    Ok(())
}