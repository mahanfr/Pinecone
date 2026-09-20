mod structs;
mod types;
use std::{collections::BTreeMap, fs};

use crate::{lexer::{Lexer, TokenType, error}, parser::structs::{StructType, struct_def}};

/// Top level program items
#[derive(Debug, Clone)]
pub enum ContractItem {
    /// Enums
    Enum,
    /// Struct Defenition
    Struct(StructType),
    /// Function Definitions
    Func,
    /// Global Variable
    GBVariable,
}

#[derive(Debug, Clone)]
pub struct Contract {
    pub name: String,
    pub items: BTreeMap<String, ContractItem>,
}

#[derive(Debug, Clone)]
pub struct Contracts {
    // pub attrs: Vec<Attr>
    pub items: Vec<Contract>,
}

pub fn generate_ast(lexer: &mut Lexer) -> Contracts {
    lexer.next_token();
    let mut contracts = Vec::new();
    loop {
        if lexer.get_token().is_empty() {
            break;
        }
        let loc = lexer.get_token_loc();
        match lexer.get_token_type() {
            TokenType::Contract => {
                let contract = parse_contract(lexer);
                contracts.push(contract);
            }
            _ => error(
                format!(
                    "Unexpected Token ({}) for the top level file",
                    lexer.get_token_type()
                ),
                loc,
            )
        }
    }
    Contracts {
        items: contracts,
    }
}

pub fn parse_contract(lexer: &mut Lexer) -> Contract {
    lexer.match_token(TokenType::Contract);
    let contract_name = lexer.get_token().literal;
    lexer.match_token(TokenType::Identifier);
    lexer.match_token(TokenType::OCurly);
    let mut items = BTreeMap::<String, ContractItem>::new();
    let mut public = false;
    loop {
        if lexer.get_token_type() == TokenType::CCurly {
            lexer.match_token(TokenType::CCurly);
            break;
        }
        let loc = lexer.get_token_loc();
        match lexer.get_token_type() {
            TokenType::Public => {
                lexer.match_token(TokenType::Public);
                public = true;
                continue;
            }
            TokenType::Struct => {
                let struct_def = struct_def(lexer, public);
                let ident = struct_def.ident.clone();
                let prv_value = items.insert(ident.clone(), ContractItem::Struct(struct_def));
                if prv_value.is_some() {
                    error(
                        format!("Struct with the name {} already exists", ident.clone()),
                        loc,
                    );
                }
            },
            _ => todo!()
        }
        public = false;

    }
    Contract { name: contract_name, items}
}

pub fn parse_source_file(path: String) -> Contracts {
    let source = fs::read_to_string(path.clone()).unwrap_or_else(|_| {
        eprintln!("Error reading file \"{}\"", path.clone());
        panic!("Can not open file!");
    });
    let mut lexer = Lexer::new(path, source);
    generate_ast(&mut lexer)
}
