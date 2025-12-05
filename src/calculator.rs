use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculatorResult {
    pub decimal: String,
    pub hex: String,
    pub binary: String,
    pub percentage: String,
}

pub fn evaluate(expression: &str) -> Result<CalculatorResult> {
    let result = meval::eval_str(expression)?;

    let decimal = format!("{}", result);
    let hex = format!("0x{:X}", result as i64);
    let binary = format!("0b{:b}", result as i64);
    let percentage = format!("{}%", result * 100.0);

    Ok(CalculatorResult {
        decimal,
        hex,
        binary,
        percentage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let result = evaluate("2 + 2").unwrap();
        assert_eq!(result.decimal, "4");

        let result = evaluate("10 * 5").unwrap();
        assert_eq!(result.decimal, "50");

        let result = evaluate("100 / 4").unwrap();
        assert_eq!(result.decimal, "25");
    }

    // Test subtraction
    #[test]
    fn test_subtraction() {
        let result = evaluate("10 - 3").unwrap();
        assert_eq!(result.decimal, "7");

        let result = evaluate("100 - 50").unwrap();
        assert_eq!(result.decimal, "50");
    }

    // Test decimals
    #[test]
    fn test_decimal_operations() {
        let result = evaluate("2.5 + 2.5").unwrap();
        assert_eq!(result.decimal, "5");

        let result = evaluate("10.5 * 2").unwrap();
        assert_eq!(result.decimal, "21");

        let result = evaluate("7.5 / 2.5").unwrap();
        assert_eq!(result.decimal, "3");
    }

    // Test negative numbers
    #[test]
    fn test_negative_numbers() {
        let result = evaluate("-5 + 10").unwrap();
        assert_eq!(result.decimal, "5");

        let result = evaluate("-3 * -3").unwrap();
        assert_eq!(result.decimal, "9");
    }

    // Test parentheses
    #[test]
    fn test_parentheses() {
        let result = evaluate("(2 + 3) * 4").unwrap();
        assert_eq!(result.decimal, "20");

        let result = evaluate("2 * (3 + 4)").unwrap();
        assert_eq!(result.decimal, "14");

        let result = evaluate("((1 + 2) * (3 + 4))").unwrap();
        assert_eq!(result.decimal, "21");
    }

    // Test exponents
    #[test]
    fn test_exponents() {
        let result = evaluate("2^3").unwrap();
        assert_eq!(result.decimal, "8");

        let result = evaluate("10^2").unwrap();
        assert_eq!(result.decimal, "100");
    }

    // Test hex formatting
    #[test]
    fn test_hex_formatting() {
        let result = evaluate("255").unwrap();
        assert_eq!(result.hex, "0xFF");

        let result = evaluate("16").unwrap();
        assert_eq!(result.hex, "0x10");

        let result = evaluate("10 + 6").unwrap();
        assert_eq!(result.hex, "0x10");
    }

    // Test binary formatting
    #[test]
    fn test_binary_formatting() {
        let result = evaluate("8").unwrap();
        assert_eq!(result.binary, "0b1000");

        let result = evaluate("15").unwrap();
        assert_eq!(result.binary, "0b1111");

        let result = evaluate("2 + 2").unwrap();
        assert_eq!(result.binary, "0b100");
    }

    // Test percentage calculation
    #[test]
    fn test_percentage_calculation() {
        let result = evaluate("0.5").unwrap();
        assert_eq!(result.percentage, "50%");

        let result = evaluate("0.25").unwrap();
        assert_eq!(result.percentage, "25%");

        let result = evaluate("1").unwrap();
        assert_eq!(result.percentage, "100%");

        let result = evaluate("2").unwrap();
        assert_eq!(result.percentage, "200%");
    }

    // Test error handling
    #[test]
    fn test_invalid_expression() {
        let result = evaluate("hello");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_expression() {
        let result = evaluate("");
        assert!(result.is_err());
    }

    #[test]
    fn test_incomplete_expression() {
        let result = evaluate("2 +");
        assert!(result.is_err());
    }

    // Test CalculatorResult struct
    #[test]
    fn test_calculator_result_struct() {
        let result = evaluate("100").unwrap();

        // All fields should be populated
        assert!(!result.decimal.is_empty());
        assert!(!result.hex.is_empty());
        assert!(!result.binary.is_empty());
        assert!(!result.percentage.is_empty());
    }

    #[test]
    fn test_calculator_result_serialization() {
        let result = evaluate("42").unwrap();
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: CalculatorResult = serde_json::from_str(&json).unwrap();

        assert_eq!(result.decimal, deserialized.decimal);
        assert_eq!(result.hex, deserialized.hex);
        assert_eq!(result.binary, deserialized.binary);
        assert_eq!(result.percentage, deserialized.percentage);
    }

    // Test mathematical constants
    #[test]
    fn test_pi_constant() {
        let result = evaluate("pi").unwrap();
        assert!(result.decimal.starts_with("3.14"));
    }

    #[test]
    fn test_e_constant() {
        let result = evaluate("e").unwrap();
        assert!(result.decimal.starts_with("2.71"));
    }

    // Test mathematical functions
    #[test]
    fn test_sqrt_function() {
        let result = evaluate("sqrt(16)").unwrap();
        assert_eq!(result.decimal, "4");

        let result = evaluate("sqrt(25)").unwrap();
        assert_eq!(result.decimal, "5");
    }

    #[test]
    fn test_abs_function() {
        let result = evaluate("abs(-5)").unwrap();
        assert_eq!(result.decimal, "5");
    }

    // Test order of operations
    #[test]
    fn test_order_of_operations() {
        let result = evaluate("2 + 3 * 4").unwrap();
        assert_eq!(result.decimal, "14"); // Not 20

        let result = evaluate("10 - 6 / 2").unwrap();
        assert_eq!(result.decimal, "7"); // Not 2
    }

    // Test large numbers
    #[test]
    fn test_large_numbers() {
        let result = evaluate("1000000 + 1").unwrap();
        assert_eq!(result.decimal, "1000001");
    }

    // Test small decimals
    #[test]
    fn test_small_decimals() {
        let result = evaluate("0.001 + 0.001").unwrap();
        assert_eq!(result.decimal, "0.002");
    }
}
