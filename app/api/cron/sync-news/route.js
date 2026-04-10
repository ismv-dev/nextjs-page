import { NextResponse } from "next/server";
import { syncAllNews } from "@/lib/newsSync";
import { initializeDatabase } from "@/lib/db";

// Este endpoint se ejecuta diariamente a las 08:00 AM (UTC-3 Chile)
// Configurado en vercel.json con la propiedad "crons"

export async function GET(request) {
  // Verificar que la solicitud proviene de Vercel Crons
  const authHeader = request.headers.get("authorization");

  // En desarrollo, permitir sin validación. En producción, validar token
  if (process.env.NODE_ENV === "production") {
    const expectedToken = process.env.CRON_SECRET;
    if (!expectedToken || authHeader !== `Bearer ${expectedToken}`) {
      return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
    }
  }

  try {
    // Inicializar BD en caso de ser necesario
    await initializeDatabase();

    // Sincronizar todas las noticias
    const results = await syncAllNews();

    return NextResponse.json(
      {
        success: true,
        message: "Sincronización de noticias completada",
        results,
        timestamp: new Date().toISOString(),
      },
      { status: 200 }
    );
  } catch (error) {
    console.error("Error en cron de sincronización:", error);

    return NextResponse.json(
      {
        success: false,
        error: error.message || "Error durante la sincronización",
        timestamp: new Date().toISOString(),
      },
      { status: 500 }
    );
  }
}

// También permitir POST para triggers manuales en desarrollo
export async function POST(request) {
  // En producción, validar token
  if (process.env.NODE_ENV === "production") {
    const authHeader = request.headers.get("authorization");
    const expectedToken = process.env.CRON_SECRET;
    if (!expectedToken || authHeader !== `Bearer ${expectedToken}`) {
      return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
    }
  }

  try {
    await initializeDatabase();
    const results = await syncAllNews();

    return NextResponse.json(
      {
        success: true,
        message: "Sincronización manual completada",
        results,
        timestamp: new Date().toISOString(),
      },
      { status: 200 }
    );
  } catch (error) {
    console.error("Error en sincronización manual:", error);

    return NextResponse.json(
      {
        success: false,
        error: error.message || "Error durante la sincronización",
        timestamp: new Date().toISOString(),
      },
      { status: 500 }
    );
  }
}
