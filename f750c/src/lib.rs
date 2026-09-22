use crate::error::CompilerResult;

mod opcode;
mod value;
mod constants;
mod semantic;
mod error;
mod parser;
mod util;
mod token;

pub fn compile(source: &str) -> CompilerResult<()> {
    let lines = source.lines();

    Ok(())
}

mod test {
    const TEST_SOURCE: &str = include_str!("test/test_script.f750");

    #[test]
    fn test_compile() {
        let res = crate::compile(TEST_SOURCE);
        if let Err(e) = res {
            panic!("Compilation failed: {:?}", e);
        }
    }
}