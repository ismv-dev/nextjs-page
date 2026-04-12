import { NextResponse } from "next/server";
import { syncNewsToDatabase, initializeDatabase, getAvailableCategories } from "@/lib/newsSync";
import getNewsFromDatabase from "@/lib/getNews";

export async function GET(request) {
  try {
    const newsCategories = getAvailableCategories();
    const { searchParams } = request.nextUrl;
    const categories = searchParams.get("categories") ? decodeURIComponent(searchParams.get("categories")).split(",") : newsCategories;
    const startDate = searchParams.get("startDate");
    const endDate = searchParams.get("endDate");
    
    // Validar y limitar paginación para evitar ataques de denegación de servicio (DoS)
    const rawLimit = parseInt(searchParams.get("limit") || "10");
    const limit = Math.min(Math.max(rawLimit, 1), 100); // Mínimo 1, Máximo 100
    
    const rawOffset = parseInt(searchParams.get("offset") || "0");
    const offset = Math.max(rawOffset, 0);
    
    // REMOVED: syncParam = searchParams.get("sync") === "true" to prevent DoS attacks via public endpoint

    // Validar categorías si se proporcionan
    if (categories && categories.length > 0) {
      const invalidCategories = categories.filter(cat => !newsCategories.includes(cat));
      if (invalidCategories.length > 0) {
        return NextResponse.json({ error: "Una o más categorías no son válidas" }, { status: 400 });
      }
    }

    // Inicializar BD si es necesario
    try {
      await initializeDatabase();
    } catch (dbError) {
      console.error("Error inicializando BD:", dbError);
      // Continuar aunque falle la inicialización
    }

    // Intentar obtener noticias de la BD
    let items = [];
    let fromDatabase = true;

    try {
      items = await getNewsFromDatabase({ categories, startDate, endDate }, limit, offset);
      fromDatabase = items.length > 0;
    } catch (dbError) {
      console.error("Error obteniendo noticias de BD:", dbError);
    }

    // Sincronizar en segundo plano si es la primera página (offset 0)
    // NO usamos await aquí para no bloquear la respuesta al cliente
    if (offset === 0) {
      (async () => {
        try {
          for (const cat of categories) {
            try {
              await syncNewsToDatabase(cat);
            } catch (error) {
              console.error(`Error sincronizando ${cat}:`, error);
            }
          }
          console.log("Sincronización de fondo completada.");
        } catch (syncError) {
          console.error("Error crítico en proceso de sincronización de fondo:", syncError);
        }
      })();
    }

    // Transformar datos de BD para que coincidan con el formato esperado
    if (!Array.isArray(items)) {
      console.error("Items no es un array:", items);
      items = [];
    }
    const formattedItems = items.map((item) => ({
      title: item.title,
      description: item.description || "",
      link: item.link,
      timestamp: item.timestamp,
      imageUrl: item.image_url || "",
      category: item.category,
    }));

    return NextResponse.json({
      allCategories: newsCategories,
      items: formattedItems,
      fromDatabase,
      count: formattedItems.length,
      timestamp: new Date(),
    });
  } catch (error) {
    console.error("Error en API /news:", error);
    return NextResponse.json(
      { error: "Error interno al procesar noticias", requiresSync: true },
      { status: 500 }
    );
  }
}
