// Utility functions for arithmetic operations

export function buildArithmeticExpression() {
  const operations = ["+", "-", "*", "/"];
  const numOperands = Math.floor(Math.random() * 3) + 2; // 2-4 operands
  let expression = "";
  let result = Math.floor(Math.random() * 10) + 1; // Start with 1-10

  for (let i = 0; i < numOperands; i += 1) {
    const operation = operations[Math.floor(Math.random() * operations.length)];
    let operand;

    switch (operation) {
      case "+":
        operand = Math.floor(Math.random() * 20) + 1;
        result += operand;
        break;
      case "-":
        operand = Math.floor(Math.random() * result) + 1; // Ensure non-negative result
        result -= operand;
        break;
      case "*":
        operand = Math.floor(Math.random() * 5) + 2; // 2-6
        result *= operand;
        break;
      case "/":
        // Find a divisor that results in integer
        const possibleDivisors = [];
        for (let d = 2; d <= 10; d += 1) {
          if (result % d === 0) possibleDivisors.push(d);
        }
        if (possibleDivisors.length === 0) {
          operand = 1; // Fallback
        } else {
          operand = possibleDivisors[Math.floor(Math.random() * possibleDivisors.length)];
          result /= operand;
        }
        break;
    }

    if (i === 0) {
      expression = String(result);
    } else {
      expression += ` ${operation} ${operand}`;
    }
  }

  return expression;
}

export function getBinaryConversion() {
  const decimal = Math.floor(Math.random() * 256); // 0-255
  return `0b${decimal.toString(2).padStart(8, "0")}`;
}