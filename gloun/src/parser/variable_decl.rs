/**********************************************************************************************
*
*   parser/variable_decl: parsing variable declearation and initial assginemnt
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
use crate::{
    lexer::{Lexer, Loc, TokenType, error},
    parser::types::type_def,
};

use super::{
    expr::{expr, Expr},
    types::VariableType,
};

/// Variable Declearation
/// * mutable - if the value of the variable can be changed
/// * static - if the variable is global
/// * ident - variable identifier
/// * v_type - Variable Type
/// * init_value - Expr of initial value
#[derive(Debug, Clone)]
pub struct VariableDeclare {
    pub public: bool,
    pub ident: String,
    pub v_type: VariableType,
    pub init_value: Option<Expr>,
    pub loc: Loc,
}

/// parse variable declare
pub fn inline_variable_declare(lexer: &mut Lexer) -> VariableDeclare {
    let ident_token = lexer.get_token();
    let _loc = lexer.get_token_loc();
    lexer.match_token(TokenType::Identifier);
    let mut v_type: VariableType = VariableType::Null;
    let mut init_value: Option<Expr> = None;
    if lexer.get_token_type() == TokenType::ATSign {
        v_type = type_def(lexer);
    }
    let loc = lexer.get_current_loc();
    match lexer.get_token_type() {
        TokenType::DoubleColon => {
            lexer.match_token(TokenType::ColonEq);
            init_value = Some(expr(lexer));
        }
        TokenType::ColonEq => {
            lexer.match_token(TokenType::ColonEq);
            init_value = Some(expr(lexer));
        }
        TokenType::Eq => {
            lexer.match_token(TokenType::Eq);
            init_value = Some(expr(lexer));
        }
        TokenType::SemiColon | TokenType::To => (),
        _ => {
            error(
                format!(
                    "Expected \"=\" or \":=\" found ({})",
                    lexer.get_token_type()
                ),
                loc,
            );
        }
    }
    VariableDeclare {
        public: false,
        ident: ident_token.literal,
        v_type,
        init_value,
        loc,
    }
}

pub fn raw_variable_declare(lexer: &mut Lexer, ident: String) -> VariableDeclare {
    let loc = lexer.get_token_loc();
    let mut init_value : Option<Expr> = None;
    let mut v_type = VariableType::Null;
    match lexer.get_token_type() {
        TokenType::ColonEq => {
            lexer.match_token(TokenType::ColonEq);
            init_value = Some(expr(lexer));
        }
        TokenType::ATSign => {
            v_type = type_def(lexer);
            if lexer.get_token_type() == TokenType::ColonEq {
                lexer.match_token(TokenType::ColonEq);
                init_value = Some(expr(lexer));
            }
        }
        _ => {
            error(
                format!(
                    "Expected \":=\" or a type found ({})",
                    lexer.get_token_type()
                ),
                loc,
            );
        }
    }
    lexer.match_token(TokenType::SemiColon);
    VariableDeclare { public: false, ident, v_type, init_value, loc }
}

/// Parse Variable Declaration
pub fn variable_declare(lexer: &mut Lexer, public: bool) -> VariableDeclare {
    let ident = lexer.get_token().literal;
    let loc = lexer.get_token_loc();
    let mut init_value : Option<Expr> = None;
    let mut v_type = VariableType::Null;
    lexer.match_token(TokenType::Identifier);
    match lexer.get_token_type() {
        TokenType::ColonEq => {
            lexer.match_token(TokenType::ColonEq);
            init_value = Some(expr(lexer));
        }
        TokenType::ATSign => {
            v_type = type_def(lexer);
            if lexer.get_token_type() == TokenType::ColonEq {
                lexer.match_token(TokenType::ColonEq);
                init_value = Some(expr(lexer));
            }
        }
        _ => {
            error(
                format!(
                    "Expected \":=\" or a type found ({})",
                    lexer.get_token_type()
                ),
                loc,
            );
        }
    }
    lexer.match_token(TokenType::SemiColon);
    VariableDeclare { public, ident, v_type, init_value, loc }
}
