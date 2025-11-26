use serde::{Deserialize, Serialize};
use anyhow::Result;

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
}
