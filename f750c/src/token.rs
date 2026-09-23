//! This module defines constants representing special characters and strings used in the F750C language, as well utility functions.

use crate::token;

/// Denotes the beginning of a comment.
/// During tokenization of a line, all characters (including this character) beyond the comment delimiter are ignored.
pub const ATOM_COMMENT: char = ';';
/// Denotes whitespace.
pub const ATOM_WHITESPACE: char = ' ';
/// Denotes a separator. Separators are used to separate arguments in a list of arguments.
pub const ATOM_SEPARATOR: char = ',';
/// Denotes a colon character. Colons are used to separate binding names from their values, as well as marking end-of-line for label and section definitions.
pub const ATOM_COLON: char = ':';
/// Denotes a quote character. Quotes are used to denote string literals.
pub const ATOM_QUOTE: char = '"';
/// Denotes an underscore character. Underscores are used in binding names and labels.
pub const ATOM_UNDERLINE: char = '_';
/// Denotes an integer memory offset from an address.
pub const ATOM_PLUS: char = '+';
/// Denotes memory dereference.
pub const ATOM_DEREF: char = '&';
/// Denotes either a constant dereference, or the beginning of a compiler directive.
pub const ATOM_HASHTAG: char = '#';
/// Denotes the beginning of a compiler construct.
pub const ATOM_AT: char = '@';

/// Denotes a constant binding.
pub const TOKEN_CONST_BINDING: &str = "const";
/// The character dot(.).
pub const CHAR_DOT: char = '.';

/// The beginning of the special '.data' section.
pub const SPECIAL_SECTION_LABEL_DATA: &str = ".data";

pub fn is_special_atom(ch: char, exclude_digit: bool) -> bool {
    ch == ATOM_WHITESPACE || ch == ATOM_SEPARATOR || ch == ATOM_COLON || ch == ATOM_QUOTE || ch == ATOM_PLUS || ch == ATOM_DEREF || ch == ATOM_HASHTAG || ch == ATOM_AT
}