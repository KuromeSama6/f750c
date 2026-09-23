# f750c

F750 (Fight 750 State Script) is a lightweight and general-purpose state machine scripting language designed for fighting games.

`f750c` is the compiler for F750, which compiles F750 scripts into bytecode (`.f750b` files) that can be executed by a F750 engine implementation.

# Compilation Flow

Below is a general outline of the compilation flow for F750 scripts:

## Tokenization

Each line of the text source file is translated into a sequence of Tokens. A token can be a string literal, a number literal, special characters, whitespace, or a sequence of characters (represented as String) that does not fit in any of the other Token types.

This is represented by `tokenizer::Token`.

## Parsing

Each sequence of Tokens (representing a line) is parsed into a structured semantic representation (SR). SRs may also represent a compiler construct.

This is represented by `parser::SemanticRepr.`

## Compiler Construct Expansion

SRs representing compiler constructs are expanded into sequences of SRs that represent normal instructions. This step eliminates all compiler constructs from the SRs.

## Binding Table and Inlining

- A virtual binding table is created, in which each binding is assigned a virtual address, starting from 0 for the first binding.
- All defined bindings have their values packed and emitted into the bytecode stream.
- All binding references in the SRs are replaced with their corresponding virtual addresses. This step eliminates all binding references from the SRs.
- Constant bindings that are inline-able are inlined into the SRs, which replaces the binding references with their corresponding values.

## Label Inlining

All label references in the SRs are replaced with their corresponding virtual addresses. This step eliminates all label references from the SRs.

## Instruction Validation and Bytecode Emission

Instructions are validated for correctness and then emitted into the bytecode stream.

## Output

The resulting bytecode stream is produced.

# Current State

This project has just exited the conceptual phase and is currently in very early stages of development.