use std::{collections::HashMap, fmt::Display};

use crate::{lexer::{Loc, error}, parser::{blocks::Block, functions::FunctionDecl, structs::StructType, types::VariableType}};

#[derive(Debug)]
pub enum UserDefType {
    Func(FunctionDecl),
    Struct(StructType),
}

impl Display for UserDefType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Func(fun) => write!(f, "Func({})", fun.ident),
            Self::Struct(s) => write!(f, "Struct {}", s.ident),
        }
    }
}

impl UserDefType {
    pub fn ident(&self) -> String {
        match self {
            Self::Func(f) => f.ident.clone(),
            Self::Struct(s) => s.ident.clone(),
        }
    }

    pub fn loc(&self) -> Loc {
        match self {
            Self::Func(f) => f.loc.clone(),
            Self::Struct(s) => s.loc.clone(),
        }
    }
}

#[derive(Debug)]
pub struct Namespace {
    types: HashMap<String, UserDefType>,
    variables: HashMap<String, Vec<(String, VariableType)>>,
}

impl Namespace {
    pub fn add_user_type(&mut self, typ: UserDefType) {
        let ident = typ.ident();
        if let Some(t) = self.types.insert(ident, typ) {
            error(format!("Type {} already exists", t), t.loc())
        };
    }

    pub fn add_variable(&mut self, block: &Block, ident: &str, vt: VariableType) {
        self.variables.entry(block.id.clone())
            .or_insert_with(Vec::new).push((ident.to_string(), vt));
    }
}
