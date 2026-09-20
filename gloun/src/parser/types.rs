use std::fmt::Display;

use crate::{lexer::{Lexer, TokenType}};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum VariableType {
    /// Type of a Variable Before type refering
    /// Will cause unreachable code if used
    Null,
    Void,
    U8,
    U16,
    U32,
    U64,
    U128,
    U256,
    I8,
    I16,
    I32,
    I64,
    I128,
    I256,
    Bool,
    /// e.g. @[address; 32]
    Array(Box<VariableType>, usize),
    /// e.g., @Map<address, @i32>
    Map(Box<VariableType>, Box<VariableType>),
    /// e.g., @List<address>
    List(Box<VariableType>),
    Address,
    Pk,
    Hash,
    Custom(String),
}

impl VariableType {
    /// Convert String literal to Variable Type
    pub fn from_string(literal: String) -> Self {
        match literal.as_str() {
            "u8" => Self::U8,
            "u16" => Self::U16,
            "u32" => Self::U32,
            "u64" => Self::U64,
            "u128" => Self::U128,
            "u256" => Self::U256,
            "i8" => Self::I8,
            "i16" => Self::I16,
            "i32" => Self::I32,
            "i64" => Self::I64,
            "i128" => Self::I128,
            "i256" => Self::I256,
            "bool" => Self::Bool,
            "hash" => Self::Hash,
            "pk" => Self::Pk,
            "address" => Self::Address,
            _ => Self::Custom(literal),
        }
    }

    /// returns size of the type
    pub fn size(&self) -> usize {
        match self {
            Self::Void | Self::Null => 0,
            Self::U8 | Self::I8 | Self::Bool => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 => 4,
            Self::U64 | Self::I64 => 8,
            Self::U128 | Self::I128 => 16,
            Self::U256 | Self::I256 => 32,
            Self::Address | Self::Pk | Self::Hash => 32,
            Self::Array(t, s) => t.size() * s,
            Self::Map(_, _) | Self::List(_) => 32,
            Self::Custom(_) => 32,
        }
    }

    pub fn is_predifined_type(t: &str) -> bool {
        match t {
            "Map" | "List" => true,
            _ => false,
        }
    }

    /// checks if type is any
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

}

impl Display for VariableType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Void => write!(f, "@void"),
            Self::Null => write!(f, "@Null"),
            Self::U8 => write!(f, "@uint8"),
            Self::U16 => write!(f, "@uint16"),
            Self::U32 => write!(f, "@uint32"),
            Self::U64 => write!(f, "@uint64"),
            Self::U128 => write!(f, "@uint128"),
            Self::U256 => write!(f, "@uint256"),
            Self::I8 => write!(f, "@i8"),
            Self::I16 => write!(f, "@i16"),
            Self::I32 => write!(f, "@i32"),
            Self::I64 => write!(f, "@i64"),
            Self::I128 => write!(f, "@i128"),
            Self::I256 => write!(f, "@i256"),
            Self::Bool => write!(f, "@bool"),
            Self::Array(vt, s) => write!(f, "@[{vt}; {s}]"),
            Self::Map(kt, vt) => write!(f, "@Map<{kt}, {vt}>"),
            Self::List(vt) => write!(f, "@List<{vt}>"),
            Self::Address => write!(f, "@address"),
            Self::Pk => write!(f, "@pk"),
            Self::Hash => write!(f, "@hash"),
            Self::Custom(t) => write!(f, "@{t}"),
        }
    }
}

fn parse_custom(lexer: &mut Lexer, typ_string: &str) -> VariableType {
    lexer.match_token(TokenType::Identifier);
    match typ_string {
        "Map" => {
            lexer.match_token(TokenType::Smaller);
            let key_type_ident = lexer.get_token().literal;
            lexer.match_token(TokenType::Identifier);
            lexer.match_token(TokenType::Comma);
            let value_type_ident = lexer.get_token().literal;
            lexer.match_token(TokenType::Identifier);
            lexer.match_token(TokenType::Bigger);

            let key_type = VariableType::from_string(key_type_ident);
            let value_type = VariableType::from_string(value_type_ident);
            VariableType::Map(Box::new(key_type), Box::new(value_type))
        },
        "List" => {
            lexer.match_token(TokenType::Smaller);
            let type_ident = lexer.get_token().literal;
            lexer.match_token(TokenType::Identifier);
            lexer.match_token(TokenType::Bigger);

            let typ = VariableType::from_string(type_ident);
            VariableType::List(Box::new(typ))
        },
        _ => unreachable!("is_predifined_type() should be false"),
    }
}

/// Parse type definition
pub fn type_def(lexer: &mut Lexer) -> VariableType {
    // let loc = lexer.get_current_loc();
    lexer.match_token(TokenType::ATSign);
    match lexer.get_token_type() {
        TokenType::Identifier => {
            let ident = lexer.get_token().literal;
            if VariableType::is_predifined_type(&ident) {
                parse_custom(lexer, &ident)
            } else {
                lexer.match_token(TokenType::Identifier);
                VariableType::from_string(ident)
            }
        }
        TokenType::OBracket => todo!("Not implmented yet"),
        _ => unreachable!("Not a type"),
    }
}
