use crate::error::{Error, Result};
use crate::types::Type;
use std::collections::HashMap;
use witch_runtime::{value::Value, vm::InfixOp};

use super::{
    ast::Ast,
    either,
    lexer::{Kind, Lexer},
    maybe, maybe_type,
    r#type::{properties, type_literal},
    statement::statement,
    Parser,
};

pub fn expression<'input>(p: &mut Parser<'input, Lexer<'input>>) -> Result<Ast> {
    let start = p.cursor;
    let mut expr = match p.peek() {
        Some(lit @ Kind::Int) | Some(lit @ Kind::String) | Some(lit @ Kind::Float) => {
            let token = p.consume(&lit)?;
            let txt = p.text(&token);
            match lit {
                Kind::Int => Ast::Value(Value::Usize(txt.parse().expect("invalid integer"))),
                Kind::Float => Ast::Value(Value::F32(txt.parse().expect("invalid 32bit float"))),
                Kind::String => Ast::Value(Value::String(txt.to_string())),
                _ => unreachable!(),
            }
        }
        Some(Kind::Ident) => {
            // An expression starting with an identifier can be
            // - A variable: my_var
            // - A struct: Foo { }
            let token = p.consume(&Kind::Ident)?;
            let ident = p.text(&token).to_string();

            if p.at(Kind::LBrace) {
                // Struct expression
                p.consume(&Kind::LBrace)?;
                let fields = map_values(p, HashMap::default())?;
                p.consume(&Kind::RBrace)?;

                Ast::Struct {
                    ident: Some(ident),
                    fields,
                    span: start..p.cursor,
                }
            } else {
                let var = Ast::Var(ident);

                // Variables can be called as functions
                if p.at(Kind::LParen) {
                    function_call(p, Box::new(var))?
                } else {
                    var
                }
            }
        }
        Some(Kind::LParen) => {
            // An expression starting with a left paren can be
            // - A function expression: () -> {}
            // - A nested expression: (varname + 6 * (2 - 1))
            either(p, vec![function_expression, nested_expression])?
        }
        Some(Kind::LSquare) => {
            // A list literal
            p.consume(&Kind::LSquare)?;
            let items = list_expressions(p, vec![])?;
            p.consume(&Kind::RSquare)?;

            Ast::List {
                items,
                span: start..p.cursor,
            }
        }
        x => panic!("invalid expression start: {:?} at {}", x, p.cursor),
    };

    expr = member_or_func_call(p, expr)?;

    if let Ok(op) = maybe(p, infix_operator) {
        let end = p.cursor;
        expr = Ast::Infix {
            lhs: Box::new(expr),
            op,
            rhs: Box::new(expression(p)?),
            span: start..end,
        }
    }

    Ok(expr)
}

pub fn infix_operator<'input>(p: &mut Parser<'input, Lexer<'input>>) -> Result<InfixOp> {
    let start = p.cursor;
    let op = match p.peek() {
        Some(kind @ Kind::Eqq) => {
            p.consume(&kind)?;
            InfixOp::Eq
        }
        Some(kind @ Kind::Gte) => {
            p.consume(&kind)?;
            InfixOp::Gte
        }
        Some(kind @ Kind::Lte) => {
            p.consume(&kind)?;
            InfixOp::Lte
        }
        Some(kind @ Kind::Plus) => {
            p.consume(&kind)?;
            InfixOp::Add
        }
        Some(kind @ Kind::Minus) => {
            p.consume(&kind)?;
            InfixOp::Sub
        }
        Some(kind @ Kind::Times) => {
            p.consume(&kind)?;
            InfixOp::Mul
        }
        Some(kind @ Kind::Slash) => {
            p.consume(&kind)?;
            InfixOp::Div
        }
        Some(kind @ Kind::And) => {
            p.consume(&kind)?;
            InfixOp::And
        }
        Some(kind @ Kind::Or) => {
            p.consume(&kind)?;
            InfixOp::Or
        }
        Some(kind @ Kind::Percent) => {
            p.consume(&kind)?;
            InfixOp::Mod
        }
        x => {
            return Err(Error::new(
                &format! {"Invalid infix operator: {:?}", x},
                start..p.cursor,
                p.input,
            ));
        }
    };
    Ok(op)
}

pub fn list_expressions<'input>(
    p: &mut Parser<'input, Lexer<'input>>,
    mut list: Vec<Ast>,
) -> Result<Vec<Ast>> {
    list.push(expression(p)?);
    if p.at(Kind::Comma) {
        p.consume(&Kind::Comma)?;
        return list_expressions(p, list);
    }
    Ok(list)
}

/// Recursively resolve function calls or member access for the expression
pub fn member_or_func_call<'input>(
    p: &mut Parser<'input, Lexer<'input>>,
    expr: Ast,
) -> Result<Ast> {
    let start = p.cursor;

    match p.peek() {
        Some(Kind::Dot) => {
            p.consume(&Kind::Dot)?;
            let token = p.consume(&Kind::Ident)?;
            let key = p.text(&token).to_string();
            member_or_func_call(
                p,
                Ast::Member {
                    container: Box::new(expr),
                    key,
                    span: start..p.cursor,
                },
            )
        }

        // Any expression can be called
        Some(Kind::LParen) => {
            let fn_call = function_call(p, Box::new(expr))?;
            member_or_func_call(p, fn_call)
        }
        _ => Ok(expr),
    }
}

/// A map of Ident -> Expression bindings
/// ## Example
/// ```no
/// field: "Hello",
/// field_2: some_var
pub fn map_values<'input>(
    p: &mut Parser<'input, Lexer<'input>>,
    mut properties_: HashMap<String, Ast>,
) -> Result<HashMap<String, Ast>> {
    let token = p.consume(&Kind::Ident)?;
    let name = p.text(&token).to_string();

    let expr = if p.at(Kind::Colon) {
        p.consume(&Kind::Colon)?;
        expression(p)?
    } else {
        Ast::Var(name.clone())
    };

    properties_.insert(name, expr);

    let res = match p.peek() {
        Some(Kind::Comma) => {
            p.consume(&Kind::Comma)?;
            if p.at(Kind::Ident) {
                map_values(p, properties_)?
            } else {
                properties_
            }
        }
        _ => properties_,
    };
    // May have an automatic semicolon. Disregard it.
    if p.at(Kind::Semicolon) {
        p.consume(&Kind::Semicolon)?;
    }

    Ok(res)
}

fn nested_expression<'input>(p: &mut Parser<'input, Lexer<'input>>) -> Result<Ast> {
    p.consume(&Kind::LParen)?;
    let expr = if p.at(Kind::RParen) {
        Ast::Nop
    } else {
        expression(p)?
    };
    p.consume(&Kind::RParen)?;
    Ok(expr)
}

fn function_call<'input>(p: &mut Parser<'input, Lexer<'input>>, expr: Box<Ast>) -> Result<Ast> {
    let mut args = vec![];
    let start = p.cursor;
    p.consume(&Kind::LParen)?;
    while !p.at(Kind::RParen) {
        let arg = expression(p)?;
        args.push(arg);
        if p.at(Kind::Comma) {
            p.consume(&Kind::Comma)?;
        }
    }
    p.consume(&Kind::RParen)?;
    Ok(Ast::Call {
        expr,
        args,
        span: start..p.cursor,
    })
}

pub fn function_expression<'input>(p: &mut Parser<'input, Lexer<'input>>) -> Result<Ast> {
    // Possibly type variables
    // <T, U>
    let type_vars = if let Some(Kind::LAngle) = p.peek() {
        p.consume(&Kind::LAngle)?;
        let vars = p.repeating(vec![], Kind::Ident, Some(Kind::Comma))?;
        p.consume(&Kind::RAngle)?;
        vars.iter()
            .map(|t| p.text(t).to_string())
            .collect::<Vec<String>>()
    } else {
        vec![]
    };

    p.consume(&Kind::LParen)?;
    let args = list_args(p, vec![])?;
    let mut is_variadic = false;
    if p.at(Kind::DotDotDot) {
        p.consume(&Kind::DotDotDot)?;
        is_variadic = true;
    }
    p.consume(&Kind::RParen)?;

    let constraints = where_constraints(p)?;

    let mut generics = HashMap::default();
    for v in type_vars.into_iter() {
        generics.insert(v, Type::Any);
    }
    for (k, v) in constraints.into_iter() {
        generics.entry(k).and_modify(|e| *e = v);
    }

    p.consume(&Kind::Arrow)?;

    let mut returns = Type::Unknown;
    let body = if let Ok(ty) = maybe_type(p, type_literal) {
        returns = ty;
        p.consume(&Kind::LBrace)?;

        let stmt = statement(p).unwrap();
        p.consume(&Kind::RBrace)?;
        stmt
    } else {
        expression(p)?
    };

    Ok(Ast::Function {
        args,
        returns,
        body: Box::new(body),
        is_variadic,
        generics,
    })
}

pub fn where_constraints<'input>(
    p: &mut Parser<'input, Lexer<'input>>,
) -> Result<HashMap<String, Type>> {
    let mut constraints = HashMap::default();
    if p.at(Kind::KwWhere) {
        p.consume(&Kind::KwWhere)?;
        constraints = properties(p, Kind::Comma, constraints)?;
    }

    Ok(constraints)
}

fn list_args<'input>(
    p: &mut Parser<'input, Lexer<'input>>,
    mut args: Vec<(String, Type)>,
) -> Result<Vec<(String, Type)>> {
    if p.at(Kind::RParen) {
        return Ok(args);
    }

    let token = p.consume(&Kind::Ident)?;
    let name = p.text(&token).to_string();

    let ty = if matches!(p.peek(), Some(Kind::Colon)) {
        p.consume(&Kind::Colon)?;
        type_literal(p)?
    } else {
        Type::Unknown
    };

    args.push((name, ty));

    if matches!(p.peek(), Some(Kind::Comma)) {
        p.consume(&Kind::Comma)?;
        return list_args(p, args);
    }

    Ok(args)
}

mod tests {

    use super::*;
    use std::assert_matches::assert_matches;

    #[test]
    fn it_parses_function_expressions() {
        let mut p = Parser::new("() -> 1");
        let result = expression(&mut p).unwrap();
        assert_matches!(result, Ast::Function { .. });

        let mut p = Parser::new("(a, b, c) -> 1");
        let result = expression(&mut p).unwrap();
        assert_matches!(result, Ast::Function { .. });

        let mut p = Parser::new("(a) -> i32 { return 1 }");
        let result = expression(&mut p).unwrap();
        assert_matches!(result, Ast::Function { .. });

        let mut p = Parser::new("(a: string, b: i32) -> i32 { return 1 }");
        let result = expression(&mut p).unwrap();
        assert_matches!(result, Ast::Function { .. });
    }

    #[test]
    fn it_parses_basic_expressions() {
        let mut p = Parser::new("some_variable");
        let result = expression(&mut p).unwrap();
        assert_matches!(result, Ast::Var(_));

        let mut p = Parser::new("1");
        let result = expression(&mut p).unwrap();
        assert_matches!(result, Ast::Value(Value::Usize(_)));

        let mut p = Parser::new("\"a string literal\"");
        let result = expression(&mut p).unwrap();
        assert_matches!(result, Ast::Value(Value::String(_)));

        let mut p = Parser::new("[1, 2, 3]");
        let result = expression(&mut p).unwrap();
        assert_matches!(result, Ast::List { .. });
    }

    #[test]
    fn it_parses_infixes() {
        let mut p = Parser::new("1 + 1");
        let result = expression(&mut p).unwrap();
        assert_matches!(
            result,
            Ast::Infix {
                op: InfixOp::Add,
                ..
            }
        );

        let mut p = Parser::new("1 + 1 - 2 + 5 * 17");
        let result = expression(&mut p).unwrap();
        assert_matches!(
            result,
            Ast::Infix {
                op: InfixOp::Add,
                ..
            }
        );
    }
}
