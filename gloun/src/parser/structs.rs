/**********************************************************************************************
*
*   parser/structs: parsing strucure defenitions
*
*   LICENSE: MIT
*
*   Copyright (c) 2023-2024 Mahan Farzaneh (@mahanfr)
*
*   This software is provided "as-is", without any express or implied warranty. In no event
*   will the authors be held liable for any damages arising from the use of this software.
*
*   Permission is granted to anyone to use this software for any purpose, including commercial
*   applications, and to alter it and redistribute it freely, subject to the following restrictions:
*
*     1. The origin of this software must not be misrepresented; you must not claim that you
*     wrote the original software. If you use this software in a product, an acknowledgment
*     in the product documentation would be appreciated but is not required.
*
*     2. Altered source versions must be plainly marked as such, and must not be misrepresented
*     as being the original software.
*
*     3. This notice may not be removed or altered from any source distribution.
*
**********************************************************************************************/
use std::collections::HashMap;
use crate::{lexer::{Lexer, TokenType}, parser::types::{VariableType, type_def}};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StructType {
    pub public: bool,
    pub ident: String,
    pub items: HashMap<String, StructItemType>,
}

impl StructType {
    pub fn size(&self) -> usize {
        let mut size = 0;
        for item in self.items.values() {
            size += item.vtype.size();
        }
        size as usize
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct StructItemType {
    pub ident: String,
    pub vtype: VariableType,
}

impl StructItemType {
    pub fn new(ident: String, vtype: VariableType) -> Self {
        Self {
            ident,
            vtype,
        }
    }
}
pub fn struct_def(lexer: &mut Lexer, public: bool) -> StructType {
    lexer.match_token(TokenType::Struct);
    let struct_ident_token = lexer.get_token();
    lexer.match_token(TokenType::Identifier);
    lexer.match_token(TokenType::OCurly);
    let mut items = HashMap::<String, StructItemType>::new();
    loop {
        if lexer.get_token_type() == TokenType::CCurly {
            lexer.match_token(TokenType::CCurly);
            break;
        }
        let ident = lexer.get_token().literal;
        lexer.match_token(TokenType::Identifier);
        if lexer.get_token_type() == TokenType::ATSign {
            let ttype = type_def(lexer);
            items.insert(
                ident.clone(),
                StructItemType::new(ident.clone(), ttype),
            );
        }
        if lexer.get_token_type() != TokenType::CCurly {
            lexer.match_token(TokenType::Comma);
        }
    }
    StructType {
        public,
        ident: struct_ident_token.literal,
        items,
    }
}
