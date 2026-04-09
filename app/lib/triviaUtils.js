export const QUESTION_TYPES = [
  "Verdadero o falso",
  "Abierta",
  "Alternativas",
  "Escenario",
];

export const LEVELS = ["Bajo", "Normal", "Avanzado", "Experto"];

export const randomQuestionType = () =>
  QUESTION_TYPES[Math.floor(Math.random() * QUESTION_TYPES.length)];
