use std::env::args;

use crate::parser::parse_source_file;

mod compiler;
mod errors;
mod lexer;
mod parser;

pub fn main() {
    let mut args = args();
    let program_name = args.next().unwrap();
    println!("{program_name} Copyright 2026-2027");
    let file = args.next().expect("No file is provided");
    let contracts = parse_source_file(file);
    println!("Contracts: {:#?}", contracts);
}
