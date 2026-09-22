pub mod assign;
pub mod blocks;
pub mod expr;
pub mod functions;
pub mod stmt;
pub mod structs;
pub mod types;
pub mod variable_decl;
use std::{collections::HashMap, fs};

use crate::{
    lexer::{Lexer, TokenType, error},
    parser::{
        functions::{FunctionDecl, FunctionDef, parse_function_definition},
        structs::{StructType, struct_def},
        variable_decl::{VariableDeclare, variable_declare},
    },
};

/// Top level program items
#[derive(Debug, Clone)]
pub enum ContractItemType {
    /// Enums
    Enum,
    /// Struct Defenition
    Struct(StructType),
    /// Function Definitions
    Func(FunctionDecl),
}

#[derive(Debug, Clone)]
pub struct Contract {
    pub name: String,
    pub funcs: Vec<FunctionDef>,
    pub variables: HashMap<String, VariableDeclare>,
    pub types: HashMap<String, ContractItemType>,
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
            ),
        }
    }
    Contracts { items: contracts }
}

pub fn parse_contract(lexer: &mut Lexer) -> Contract {
    lexer.match_token(TokenType::Contract);
    let contract_name = lexer.get_token().literal;
    lexer.match_token(TokenType::Identifier);
    lexer.match_token(TokenType::OCurly);
    let mut funcs = Vec::<FunctionDef>::new();
    let mut variables = HashMap::new();
    let mut types = HashMap::new();
    let mut public = false;
    loop {
        if lexer.get_token_type() == TokenType::CCurly {
            lexer.match_token(TokenType::CCurly);
            break;
        }
        match lexer.get_token_type() {
            TokenType::Public => {
                lexer.match_token(TokenType::Public);
                public = true;
                continue;
            }
            TokenType::Func => {
                let function_def = parse_function_definition(lexer);
                let ident = function_def.decl.ident.clone();
                types.insert(ident, ContractItemType::Func(function_def.decl.clone()));
                funcs.push(function_def);
            }
            TokenType::Struct => {
                let struct_def = struct_def(lexer, public);
                let ident = struct_def.ident.clone();
                types.insert(ident, ContractItemType::Struct(struct_def));
            }
            TokenType::Identifier => {
                // public a := 0;
                // public a @u128 := 0;
                // public a @u128;
                let var_decl = variable_declare(lexer, public);
                let ident = var_decl.ident.clone();
                variables.insert(ident, var_decl);
            }
            _ => todo!("{}", lexer.get_token().literal),
        }
        public = false;
    }
    Contract {
        name: contract_name,
        funcs,
        types,
        variables,
    }
}

pub fn parse_source_file(path: String) -> Contracts {
    let source = fs::read_to_string(path.clone()).unwrap_or_else(|_| {
        eprintln!("Error reading file \"{}\"", path.clone());
        panic!("Can not open file!");
    });
    let mut lexer = Lexer::new(path, source);
    generate_ast(&mut lexer)
}
