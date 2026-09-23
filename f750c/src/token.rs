use crate::token;

pub const ATOM_COMMENT: char = ';';
pub const ATOM_WHITESPACE: char = ' ';
pub const ATOM_SEPARATOR: char = ',';
pub const ATOM_COLON: char = ':';
pub const ATOM_QUOTE: char = '"';
pub const ATOM_UNDERLINE: char = '_';
pub const ATOM_PLUS: char = '+';
pub const ATOM_DEREF: char = '&';
pub const ATOM_HASHTAG: char = '#';
pub const ATOM_AT: char = '@';

pub const TOKEN_CONST_BINDING: &str = "const";
pub const CHAR_DOT: char = '.';

pub const SPECIAL_SECTION_LABEL_DATA: &str = ".data";

pub fn is_special_atom(ch: char, exclude_digit: bool) -> bool {
     ch == ATOM_WHITESPACE || ch == ATOM_SEPARATOR || ch == ATOM_COLON || ch == ATOM_QUOTE || ch == ATOM_PLUS || ch == ATOM_DEREF || ch == ATOM_HASHTAG || ch == ATOM_AT
}