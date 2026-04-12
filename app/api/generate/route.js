import { NextResponse } from "next/server";
import { GoogleGenAI } from "@google/genai";

const AI = new GoogleGenAI({ apiKey: process.env.GOOGLE_API_KEY });
const AI2 = new GoogleGenAI({ apiKey: process.env.GOOGLE_API_KEY_2 });

const TIPOS_PREGUNTA = ["Verdadero o falso", "Abierta", "Alternativas", "Escenario"];
const NIVELES_PREGUNTA = ["Bajo", "Normal", "Avanzado", "Experto"];

const capitalize = (value) => {
  if (!value) return "";
  const text = value.toString().trim();
  return text.charAt(0).toUpperCase() + text.slice(1);
};

async function askAI(prompt, instructions) {
  try {
    const response = await AI.models.generateContent({
      model: "gemini-2.5-flash",
      contents: prompt,
      systemInstruction: instructions,
    });
    return capitalize(response.text);
  } catch (error) {
    const response = await AI2.models.generateContent({
      model: "gemini-2.5-flash",
      contents: prompt,
      systemInstruction: instructions,
    });
    return capitalize(response.text);
  }
}

export async function POST(request) {
  // Validar token de autorización en producción
  if (process.env.NODE_ENV === "production") {
    const authHeader = request.headers.get("authorization");
    const expectedToken = process.env.API_SECRET;
    if (!expectedToken || authHeader !== `Bearer ${expectedToken}`) {
      return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
    }
  }

  const body = await request.json();
  const theme = typeof body.theme === 'string' ? body.theme.trim().substring(0, 200) : null;
  const level = typeof body.level === 'string' ? body.level.trim() : "Normal";
  const type = typeof body.type === 'string' ? body.type.trim() : null;

  if (!theme || !type || !TIPOS_PREGUNTA.includes(type) || !NIVELES_PREGUNTA.includes(level)) {
    return NextResponse.json({ error: "Faltan datos o formato inválido en la petición" }, { status: 400 });
  }

  const tipo = capitalize(type);
  const nivel = capitalize(level);
  let prompt = `Crea pregunta: ${theme}, tipo ${tipo}, nivel "${nivel}", [20-500] caracteres`;

  if (tipo.toLowerCase().includes("verdadero") && tipo.toLowerCase().includes("falso")) {
    prompt += `"--" Verdadero o Falso según corresponda.`;
  } else if (tipo.toLowerCase().includes("alternativ")) {
    prompt += `"--". Crea 2-5 opciones separadas por "--". Marca la correcta con "*" al final`;
  }

  try {
    const text = await askAI(
      `Crea una pregunta basada en el siguiente tema:
      ### TEMA: ${theme}
      ### TIPO: ${tipo}
      ### NIVEL: ${nivel}
      
      Instrucción: Genera la pregunta siguiendo el formato solicitado.`,
      'Responde solo lo solicitado. Prohibido: intros, saludos, cierres, explicaciones, caracteres no solicitados o muletillas (ej. "Aquí tienes"). Si la respuesta es un dato, entrega solo el dato. Sin ningún formato Markdown. Sé puramente funcional'
    );
    const qaParts = text.split("--").map((item) => item.trim());
    const pregunta = capitalize(qaParts[0] || "");
    const alternativas = [];
    let correcta = 0;

    if (tipo.toLowerCase().includes("verdadero") && tipo.toLowerCase().includes("falso")) {
      alternativas.push("Verdadero", "Falso");
      correcta = capitalize(qaParts[1] || "") === "Verdadero" ? 0 : 1;
    } else if (qaParts.length > 1) {
      qaParts.forEach((alt, idx) => {
        if (idx === 0) return;
        if (!alt) return;
        const formatted = capitalize(alt.replace(/\*$/, ""));
        if (formatted) {
          alternativas.push(formatted);
          if (alt.trim().endsWith("*")) {
            correcta = alternativas.length - 1;
          }
        }
      });
    }

    return NextResponse.json({ pregunta, alternativas, correcta });
  } catch (error) {
    return NextResponse.json({ error: "Error interno de AI" }, { status: 500 });
  }
}
