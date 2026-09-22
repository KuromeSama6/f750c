use bitfield_struct::bitfield;
use crate::opcode::OpcodeMnemonic;
use crate::parser::{ParseResult, TokenStream};
use crate::value::{DataType, RegisterSpec};

#[derive(Debug, Clone)]
pub struct SemanticBindingDef {
    pub name: String,
    pub constant: bool,
    pub values: Vec<SemanticLiteral>,
}

impl SemanticBindingDef {
    pub fn parse_values(stream: &mut TokenStream) -> ParseResult<Vec<SemanticLiteral>> {
        let mut ret = Vec::new();
        stream.skip_whitespace();
        
        while stream.has_more() {
            
        }
        
        Ok(ret)
    }
}

#[derive(Debug, Clone)]
pub enum SemanticLiteral {
    Byte(u8),
    Word(u16),
    DWord(u32),
    QWord(u64),
    Float(f32),
    Double(f64),
    String(String),
}

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
pub struct SemanticSymbol {
    pub name: String,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SemanticArgBody {
    Literal(DataType),
    Register(RegisterSpec),
    Label(SemanticSymbol),
    Binding(SemanticSymbol),
}