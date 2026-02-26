/// Evaluate a mathematical expression string.
///
/// Supports: +, -, *, /, ^ (power), % (modulo), parentheses, and
/// common functions: sqrt, abs, round, floor, ceil, min, max, log, ln.
///
/// Returns the result as a JSON value or an error string.
pub fn evaluate(input: &serde_json::Value) -> Result<serde_json::Value, String> {
    let expr = input["expression"]
        .as_str()
        .ok_or_else(|| "missing 'expression' field".to_string())?;

    let result = eval(expr)?;
    Ok(serde_json::json!({ "result": result, "expression": expr }))
}

fn eval(expr: &str) -> Result<f64, String> {
    let tokens = tokenize(expr)?;
    let mut pos = 0;
    let result = parse_expr(&tokens, &mut pos)?;
    if pos < tokens.len() {
        return Err(format!(
            "unexpected token at position {pos}: {:?}",
            tokens[pos]
        ));
    }
    Ok(result)
}

#[derive(Debug, Clone)]
enum Token {
    Num(f64),
    Op(char),
    LParen,
    RParen,
    Comma,
    Func(String),
}

fn tokenize(expr: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = expr.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\n' => {
                chars.next();
            }
            '0'..='9' | '.' => {
                let mut num = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() || c == '.' || c == '_' {
                        if c != '_' {
                            num.push(c);
                        }
                        chars.next();
                    } else {
                        break;
                    }
                }
                let val = num
                    .parse::<f64>()
                    .map_err(|_| format!("invalid number: {num}"))?;
                tokens.push(Token::Num(val));
            }
            '+' | '*' | '/' | '^' | '%' => {
                tokens.push(Token::Op(c));
                chars.next();
            }
            '-' => {
                // Unary minus: if at start, after operator, after left paren, or after comma
                let is_unary = matches!(
                    tokens.last(),
                    None | Some(Token::Op(_)) | Some(Token::LParen) | Some(Token::Comma)
                );
                if is_unary {
                    chars.next();
                    // Parse the number or parenthesized expression
                    if let Some(&nc) = chars.peek() {
                        if nc.is_ascii_digit() || nc == '.' {
                            let mut num = String::from("-");
                            while let Some(&c) = chars.peek() {
                                if c.is_ascii_digit() || c == '.' || c == '_' {
                                    if c != '_' {
                                        num.push(c);
                                    }
                                    chars.next();
                                } else {
                                    break;
                                }
                            }
                            let val = num
                                .parse::<f64>()
                                .map_err(|_| format!("invalid number: {num}"))?;
                            tokens.push(Token::Num(val));
                        } else {
                            // Unary minus before paren or function: push -1 * (...)
                            tokens.push(Token::Num(-1.0));
                            tokens.push(Token::Op('*'));
                        }
                    }
                } else {
                    tokens.push(Token::Op(c));
                    chars.next();
                }
            }
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }
            ',' => {
                tokens.push(Token::Comma);
                chars.next();
            }
            'a'..='z' | 'A'..='Z' => {
                let mut name = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        name.push(c.to_ascii_lowercase());
                        chars.next();
                    } else {
                        break;
                    }
                }
                // Check for constants
                match name.as_str() {
                    "pi" => tokens.push(Token::Num(std::f64::consts::PI)),
                    "e" => tokens.push(Token::Num(std::f64::consts::E)),
                    _ => tokens.push(Token::Func(name)),
                }
            }
            _ => return Err(format!("unexpected character: {c}")),
        }
    }
    Ok(tokens)
}

// Precedence climbing parser
// expr     = term (('+' | '-') term)*
// term     = power (('*' | '/' | '%') power)*
// power    = unary ('^' unary)*
// unary    = primary | func '(' args ')'
// primary  = number | '(' expr ')'

fn parse_expr(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_term(tokens, pos)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Op('+') => {
                *pos += 1;
                left += parse_term(tokens, pos)?;
            }
            Token::Op('-') => {
                *pos += 1;
                left -= parse_term(tokens, pos)?;
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_term(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_power(tokens, pos)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Op('*') => {
                *pos += 1;
                left *= parse_power(tokens, pos)?;
            }
            Token::Op('/') => {
                *pos += 1;
                let right = parse_power(tokens, pos)?;
                if right == 0.0 {
                    return Err("division by zero".to_string());
                }
                left /= right;
            }
            Token::Op('%') => {
                *pos += 1;
                let right = parse_power(tokens, pos)?;
                if right == 0.0 {
                    return Err("modulo by zero".to_string());
                }
                left %= right;
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_power(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let base = parse_primary(tokens, pos)?;
    if *pos < tokens.len() && matches!(&tokens[*pos], Token::Op('^')) {
        *pos += 1;
        let exp = parse_power(tokens, pos)?; // right-associative
        Ok(base.powf(exp))
    } else {
        Ok(base)
    }
}

fn parse_primary(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    if *pos >= tokens.len() {
        return Err("unexpected end of expression".to_string());
    }

    match &tokens[*pos] {
        Token::Num(n) => {
            let val = *n;
            *pos += 1;
            Ok(val)
        }
        Token::LParen => {
            *pos += 1;
            let val = parse_expr(tokens, pos)?;
            if *pos >= tokens.len() || !matches!(&tokens[*pos], Token::RParen) {
                return Err("missing closing parenthesis".to_string());
            }
            *pos += 1;
            Ok(val)
        }
        Token::Func(name) => {
            let name = name.clone();
            *pos += 1;
            if *pos >= tokens.len() || !matches!(&tokens[*pos], Token::LParen) {
                return Err(format!("expected '(' after function '{name}'"));
            }
            *pos += 1; // skip (

            let mut args = Vec::new();
            if *pos < tokens.len() && !matches!(&tokens[*pos], Token::RParen) {
                args.push(parse_expr(tokens, pos)?);
                while *pos < tokens.len() && matches!(&tokens[*pos], Token::Comma) {
                    *pos += 1;
                    args.push(parse_expr(tokens, pos)?);
                }
            }

            if *pos >= tokens.len() || !matches!(&tokens[*pos], Token::RParen) {
                return Err(format!("missing ')' after function '{name}'"));
            }
            *pos += 1; // skip )

            apply_func(&name, &args)
        }
        _ => Err(format!("unexpected token: {:?}", tokens[*pos])),
    }
}

fn apply_func(name: &str, args: &[f64]) -> Result<f64, String> {
    match name {
        "sqrt" => {
            ensure_args(name, args, 1)?;
            Ok(args[0].sqrt())
        }
        "abs" => {
            ensure_args(name, args, 1)?;
            Ok(args[0].abs())
        }
        "round" => {
            if args.len() == 1 {
                Ok(args[0].round())
            } else if args.len() == 2 {
                let factor = 10f64.powi(args[1] as i32);
                Ok((args[0] * factor).round() / factor)
            } else {
                Err(format!(
                    "{name} expects 1 or 2 arguments, got {}",
                    args.len()
                ))
            }
        }
        "floor" => {
            ensure_args(name, args, 1)?;
            Ok(args[0].floor())
        }
        "ceil" => {
            ensure_args(name, args, 1)?;
            Ok(args[0].ceil())
        }
        "min" => {
            if args.len() < 2 {
                return Err(format!("{name} expects at least 2 arguments"));
            }
            Ok(args.iter().cloned().fold(f64::INFINITY, f64::min))
        }
        "max" => {
            if args.len() < 2 {
                return Err(format!("{name} expects at least 2 arguments"));
            }
            Ok(args.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
        }
        "log" | "log10" => {
            ensure_args(name, args, 1)?;
            Ok(args[0].log10())
        }
        "ln" => {
            ensure_args(name, args, 1)?;
            Ok(args[0].ln())
        }
        "log2" => {
            ensure_args(name, args, 1)?;
            Ok(args[0].log2())
        }
        "pow" => {
            ensure_args(name, args, 2)?;
            Ok(args[0].powf(args[1]))
        }
        _ => Err(format!("unknown function: {name}")),
    }
}

fn ensure_args(name: &str, args: &[f64], expected: usize) -> Result<(), String> {
    if args.len() != expected {
        Err(format!(
            "{name} expects {expected} argument(s), got {}",
            args.len()
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "calculator_test.rs"]
mod tests;
