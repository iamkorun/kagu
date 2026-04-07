pub mod cli;
pub mod hook;
pub mod report;
pub mod scanner;
pub mod validator;

pub fn run() {
    std::process::exit(cli::main());
}
