function getRandomInt(min, max) {
  const minCeil = Math.ceil(min);
  const maxFloor = Math.floor(max);
  return Math.floor(Math.random() * (maxFloor - minCeil + 1)) + minCeil;
}

export function buildArithmeticExpression() {
  const rules = {
    "+": [2, 1000],
    "-": [2, 1000],
    "*": [2, 20],
    "/": [2, 20],
    "**": [2, 20],
  };

  const operators = Object.keys(rules);
  const operator = operators[Math.floor(Math.random() * operators.length)];
  const [min, max] = rules[operator];
  const n1 = getRandomInt(min, max).toString();
  let n2 = getRandomInt(min, operator === "**" ? 4 : max).toString();

  if (operator === "/") {
    let attempts = 0;
    while (attempts < 50) {
      n2 = getRandomInt(1, parseInt(n1, 10)).toString();
      if (parseInt(n1, 10) % parseInt(n2, 10) === 0) {
        break;
      }
      attempts += 1;
    }
  }

  return `${n1}${operator}${n2}`;
}

export const getBinaryConversion = () =>
  Math.floor(Math.random() * 129)
    .toString(2)
    .padStart(8, "0");
