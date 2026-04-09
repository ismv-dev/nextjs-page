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
  const body = await request.json();
  const pregunta = body.pregunta;
  const respuesta = body.respuesta;

  if (!pregunta || !respuesta) {
    return NextResponse.json({ error: "Faltan datos en la petición" }, { status: 400 });
  }

  try {
    const text = await askAI(
      `Dada la siguiente pregunta: \"${pregunta}\" ¿Es correcta esta respuesta? \"${respuesta}\"`,
      "Responde solo con \"Si\" si consideras correcta la respuesta o \"No es correcto, porque [justificacion en español]\" en caso contrario. No agregues nada másni uses emojis, caracteres o simbolos"
    );

    const validation = text.toLowerCase() === "si" || text.toLowerCase() === "yes" ? false : text;
    return NextResponse.json({ validation });
  } catch (error) {
    return NextResponse.json({ error: "Error interno de AI" }, { status: 500 });
  }
}
