/**********************************************************************************************
*
*   parser/block: parse blocks syntax (e.g: if/while block)
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
    lexer::{Lexer, TokenType},
    parser::{stmt::Stmt, variable_decl::VariableDeclare},
};

use super::{
    assign::assign,
    expr::expr,
    stmt::{StmtType, for_loop, if_stmt, while_stmt},
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BlockId(pub usize);

/// Block Stmt
/// Holds a list of stmt in a block of code
#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub variables: Vec<VariableDeclare>,
    pub master: Option<BlockId>,
}

#[derive(Debug, Clone, Default)]
pub struct BlockTree {
    pub blocks: Vec<Block>,
}

impl BlockTree {
    pub fn new_block(&mut self, master: Option<BlockId>) -> BlockId {
        let id = BlockId(self.blocks.len());
        self.blocks.push(Block {
            stmts: Vec::new(),
            variables: Vec::new(),
            master,
        });
        id
    }

    pub fn get_variable(&self, id: BlockId, ident: &str) -> Option<VariableDeclare> {
        let b = &self.blocks[id.0];
        for var in b.variables.iter().rev() {
            if var.ident == ident {
                return Some(var.clone());
            }
        }
        b.master.and_then(|p| self.get_variable(p, ident))
    }

    pub fn set_variable(&mut self, id: BlockId, decl: VariableDeclare) {
        let b = self.blocks.get_mut(id.0).unwrap();
        b.variables.push(decl);
    }
}
impl Block {
    pub fn parse_stmt(ast: &mut BlockTree, current: BlockId, lexer: &mut Lexer) -> Stmt {
        match lexer.get_token_type() {
            TokenType::Print => {
                let loc = lexer.get_token_loc();
                lexer.match_token(TokenType::Print);
                let expr = expr(lexer);
                let stmt = Stmt {
                    stype: StmtType::Print(expr),
                    loc,
                };
                lexer.match_token(TokenType::SemiColon);
                stmt
            }
            TokenType::Break => {
                let loc = lexer.get_token_loc();
                lexer.match_token(TokenType::Break);
                let stmt = Stmt {
                    stype: StmtType::Break,
                    loc,
                };
                lexer.match_token(TokenType::SemiColon);
                stmt
            }
            TokenType::Continue => {
                let loc = lexer.get_token_loc();
                lexer.match_token(TokenType::Continue);
                let stmt = Stmt {
                    stype: StmtType::Continue,
                    loc,
                };
                lexer.match_token(TokenType::SemiColon);
                stmt
            }
            TokenType::If => {
                let loc = lexer.get_token_loc();
                Stmt {
                    stype: StmtType::If(if_stmt(ast, current, lexer)),
                    loc,
                }
            }
            TokenType::While => {
                let loc = lexer.get_token_loc();
                Stmt {
                    stype: StmtType::While(while_stmt(ast, current, lexer)),
                    loc,
                }
            }
            TokenType::For => {
                let loc = lexer.get_token_loc();
                Stmt {
                    stype: StmtType::ForLoop(for_loop(ast, current, lexer)),
                    loc,
                }
            }
            TokenType::Return => {
                let loc = lexer.get_token_loc();
                lexer.match_token(TokenType::Return);
                let stmt = Stmt {
                    stype: StmtType::Return(expr(lexer)),
                    loc,
                };
                lexer.match_token(TokenType::SemiColon);
                stmt
            }
            TokenType::Identifier => {
                //Assgin Op
                assign(ast, current, lexer)
            }
            _ => {
                todo!();
            }
        }
    }

    /// Parse Blocks
    /// # Argumenrs
    /// * lexer - address of mutable lexer
    ///     Returns a vec of stmts
    pub fn parse_block(ast: &mut BlockTree, current: BlockId, lexer: &mut Lexer) {
        lexer.match_token(TokenType::OCurly);
        loop {
            if lexer.get_token_type() == TokenType::CCurly {
                break;
            }
            let stmt = Self::parse_stmt(ast, current, lexer);
            ast.blocks[current.0].stmts.push(stmt);
        }
        lexer.match_token(TokenType::CCurly);
    }
}
