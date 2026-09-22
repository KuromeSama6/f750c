use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompilerError {
    
}
pub type CompilerResult<T> = Result<T, CompilerError>;