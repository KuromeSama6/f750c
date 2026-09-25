//! Bytecode file format constants and utilities.

use crate::opcode::Opcode;

/// Bytecode file format Magic Number.
pub const F750_MAGIC: u32 = 0xF7_50_07_21;
/// Bytecode file format version number.
pub const F750_VERSION: u32 = 1;

pub trait BytecodeSerialize {
    fn serialize(&self, stream: &mut BytecodeStream);
}

#[derive(Debug)]
pub struct BytecodeStream {
    bytes: Vec<u8>,
}

impl BytecodeStream {
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
        }
    }

    pub fn new_with_header() -> Self {
        let mut stream = Self::new();
        stream.write_u32(F750_MAGIC);
        stream.write_u32(F750_VERSION);
        stream
    }

    pub fn bytes_ref(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn write<T: BytecodeSerialize>(&mut self, value: &T) {
        value.serialize(self);
    }

    pub fn write_opcode(&mut self, opcode: Opcode) {
        self.bytes.push(opcode as u8);
    }

    pub fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn write_u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_f32(&mut self, value: f32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }
    
    pub fn write_f64(&mut self, value: f64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }
    
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub fn write_i8(&mut self, value: i8) {
        self.bytes.push(value as u8);
    }

    pub fn write_i16(&mut self, value: i16) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_varint32(&mut self, mut value: u32) {
        while value >= 0x80 {
            self.write_u8((value as u8) | 0x80);
            value >>= 7;
        }
        self.write_u8(value as u8);
    }

    pub fn write_varint64(&mut self, mut value: u64) {
        while value >= 0x80 {
            self.write_u8((value as u8) | 0x80);
            value >>= 7;
        }
        self.write_u8(value as u8);
    }

    pub fn write_cstr(&mut self, s: &str) {
        self.write_bytes(s.as_bytes());
        self.write_u8(0); // Null terminator
    }
}