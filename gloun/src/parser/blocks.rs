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
    errors::CompilationError, lexer::{Lexer, TokenType}, parser::stmt::Stmt
};

use super::{
    assign::assign,
    expr::expr,
    stmt::{for_loop, if_stmt, while_stmt, StmtType},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlockType {
    Condition,
    Loop,
    Function,
    UnScoped,
}

pub fn parse_statement_outside_of_block(lexer: &mut Lexer, master: &String) -> Stmt {
    let mut block = Block::new_unscoped(master.to_string());
    block.parse_stmt(lexer)
}

pub fn get_last_loop_block_id(child_id: &str) -> Result<String, CompilationError> {
    let mut last_index = 0;
    let mut reached_loop = false;
    for (i, chr) in child_id.chars().enumerate() {
        if chr == '.' && reached_loop {
            last_index = i
        }
        if chr == '$' {
            reached_loop = true;
        }
    }
    if last_index == 0 {
        return Err(CompilationError::NotLoopBlock);
    }
    Ok(child_id.split_at(last_index).0.to_string())
}

pub fn get_first_block_id(child_id: &str) -> String {
    let mut id = String::new();
    for chr in child_id.chars() {
        if chr == '.' {
            break;
        }
        id.push(chr);
    }
    id
}

pub fn get_parent_id(child_id: &str) -> String {
    let mut last_index = 0;
    for (i, chr) in child_id.chars().enumerate() {
        if chr == '.' {
            last_index = i;
        }
    }
    child_id.split_at(last_index).0.to_string()
}

/// Block Stmt
/// Holds a list of stmt in a block of code
#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub btype: BlockType,
    pub id: String,
    num_of_children: i32,
}

impl Block {
    pub fn new(master: &mut Block, btype: BlockType) -> Self {
        let id = match btype {
            BlockType::Loop => format!("{}.${}", master.id, master.num_of_children),
            _ => format!("{}.{}", master.id, master.num_of_children),
        };
        master.num_of_children += 1;
        Self {
            num_of_children: 0,
            stmts: Vec::new(),
            btype,
            id,
        }
    }

    pub fn new_unscoped(master_id: String) -> Self {
        Self {
            id: master_id,
            btype: BlockType::UnScoped,
            num_of_children: 0,
            stmts: Vec::new(),
        }
    }

    pub fn new_global(ident: String, btype: BlockType) -> Self {
        Self {
            num_of_children: 0,
            stmts: Vec::new(),
            btype,
            id: ident,
        }
    }

    #[allow(dead_code)]
    pub fn master_start_name(&self) -> String {
        get_first_block_id(&self.id)
    }
    pub fn master_end_name(&self) -> String {
        format!("{}.Defer", get_first_block_id(&self.id))
    }
    pub fn last_loop_start_name(&self) -> Result<String, CompilationError> {
        Ok(format!("{}.BS__", get_last_loop_block_id(&self.id)?))
    }
    pub fn last_loop_end_name(&self) -> Result<String, CompilationError> {
        Ok(format!("{}.BE__", get_last_loop_block_id(&self.id)?))
    }
    #[allow(dead_code)]
    pub fn parent_start_name(&self) -> String {
        let parn_id = get_parent_id(&self.id);
        if parn_id.contains('.') {
            format!("{}.BS__", get_parent_id(&self.id))
        } else {
            parn_id
        }
    }
    #[allow(dead_code)]
    pub fn parent_end_name(&self) -> String {
        let parn_id = get_parent_id(&self.id);
        if parn_id.contains('.') {
            format!("{}.Defer", parn_id)
        } else {
            format!("{}.BE__", parn_id)
        }
    }
    pub fn start_name(&self) -> String {
        if self.btype == BlockType::Function {
            self.id.clone()
        } else {
            format!("{}.BS__", self.id)
        }
    }

    pub fn end_name(&self) -> String {
        if self.btype == BlockType::Function {
            format!("{}.Defer", self.id)
        } else {
            format!("{}.BE__", self.id)
        }
    }

    pub fn name_with_prefix(&self, prefix: &str) -> String {
        format!("{}.{prefix}__", self.id)
    }

    pub fn parse_stmt(&mut self, lexer: &mut Lexer) -> Stmt {
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
                    stype: StmtType::If(if_stmt(lexer, self)),
                    loc,
                }
            }
            TokenType::While => {
                let loc = lexer.get_token_loc();
                Stmt {
                    stype: StmtType::While(while_stmt(lexer, self)),
                    loc,
                }
            }
            TokenType::For => {
                let loc = lexer.get_token_loc();
                Stmt {
                    stype: StmtType::ForLoop(for_loop(lexer, self)),
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
                assign(lexer)
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
    pub fn parse_block(&mut self, lexer: &mut Lexer) {
        lexer.match_token(TokenType::OCurly);
        let mut stmts = Vec::<Stmt>::new();
        loop {
            if lexer.get_token_type() == TokenType::CCurly {
                break;
            }
            stmts.push(self.parse_stmt(lexer));
        }
        lexer.match_token(TokenType::CCurly);
        self.stmts.append(&mut stmts);
    }
}
