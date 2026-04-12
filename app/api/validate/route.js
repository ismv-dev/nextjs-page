import { NextResponse } from "next/server";
import { GoogleGenAI } from "@google/genai";

const AI = new GoogleGenAI({ apiKey: process.env.GOOGLE_API_KEY });
const AI2 = new GoogleGenAI({ apiKey: process.env.GOOGLE_API_KEY_2 });

const capitalize = (value) => {
  if (!value) return "";
  const text = value.toString().trim();
  return text.charAt(0).toUpperCase() + text.slice(1);
};

async function askAI(content, instructions) {
  try {
    const response = await AI.models.generateContent({
      model: "gemini-2.5-flash",
      contents: content,
      systemInstruction: instructions,
    });
    return capitalize(response.text);
  } catch (error) {
    const response = await AI2.models.generateContent({
      model: "gemini-2.5-flash",
      contents: content,
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
  const pregunta = typeof body.pregunta === 'string' ? body.pregunta.trim().substring(0, 1000) : null;
  const respuesta = typeof body.respuesta === 'string' ? body.respuesta.trim().substring(0, 1000) : null;

  if (!pregunta || !respuesta) {
    return NextResponse.json({ error: "Faltan datos o formato inválido en la petición" }, { status: 400 });
  }

  try {
    const text = await askAI(
      `Analiza la siguiente interacción:
      ### PREGUNTA: ${pregunta}
      ### RESPUESTA DEL USUARIO: ${respuesta}
      
      Instrucción: Determina si la respuesta es correcta basándote estrictamente en la pregunta.`,
      "Responde solo con \"Si\" si consideras correcta la respuesta o \"No es correcto, porque [justificacion en español]\" en caso contrario. No agregues nada másni uses emojis, caracteres o simbolos"
    );

    const validation = text.toLowerCase() === "si" || text.toLowerCase() === "yes" ? false : text;
    return NextResponse.json({ validation });
  } catch (error) {
    return NextResponse.json({ error: "Error interno de AI" }, { status: 500 });
  }
}
