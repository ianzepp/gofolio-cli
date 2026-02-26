use super::{eval, evaluate};

fn calc(expr: &str) -> f64 {
    match eval(expr) {
        Ok(value) => value,
        Err(e) => panic!("calculator eval failed for '{expr}': {e}"),
    }
}

#[test]
fn basic_arithmetic() {
    assert_eq!(calc("2 + 3"), 5.0);
    assert_eq!(calc("10 - 4"), 6.0);
    assert_eq!(calc("3 * 7"), 21.0);
    assert_eq!(calc("15 / 4"), 3.75);
}

#[test]
fn precedence() {
    assert_eq!(calc("2 + 3 * 4"), 14.0);
    assert_eq!(calc("(2 + 3) * 4"), 20.0);
}

#[test]
fn power() {
    assert_eq!(calc("2 ^ 10"), 1024.0);
    assert_eq!(calc("2 ^ 3 ^ 2"), 512.0); // right-associative: 2^(3^2) = 2^9
}

#[test]
fn modulo() {
    assert_eq!(calc("17 % 5"), 2.0);
}

#[test]
fn unary_minus() {
    assert_eq!(calc("-5 + 3"), -2.0);
    assert_eq!(calc("-(2 + 3)"), -5.0);
}

#[test]
fn functions() {
    assert_eq!(calc("sqrt(16)"), 4.0);
    assert_eq!(calc("abs(-42)"), 42.0);
    assert_eq!(calc("round(3.7)"), 4.0);
    assert_eq!(calc("round(3.14159, 2)"), 3.14);
    assert_eq!(calc("floor(3.9)"), 3.0);
    assert_eq!(calc("ceil(3.1)"), 4.0);
    assert_eq!(calc("min(5, 3, 8, 1)"), 1.0);
    assert_eq!(calc("max(5, 3, 8, 1)"), 8.0);
    assert_eq!(calc("pow(2, 8)"), 256.0);
}

#[test]
fn constants() {
    assert!((calc("pi") - std::f64::consts::PI).abs() < 1e-10);
    assert!((calc("e") - std::f64::consts::E).abs() < 1e-10);
}

#[test]
fn financial_expressions() {
    // Percentage gain
    assert_eq!(calc("(150 - 100) / 100 * 100"), 50.0);
    // Compound interest: 1000 * (1 + 0.05)^10
    let result = calc("1000 * (1 + 0.05) ^ 10");
    assert!((result - 1628.894626777442).abs() < 1e-6);
}

#[test]
fn division_by_zero() {
    assert!(eval("1 / 0").is_err());
}

#[test]
fn number_separators() {
    assert_eq!(calc("1_000_000 + 500_000"), 1_500_000.0);
}

#[test]
fn evaluate_requires_expression_field() {
    let err = evaluate(&serde_json::json!({})).expect_err("expected missing field error");
    assert_eq!(err, "missing 'expression' field");
}
