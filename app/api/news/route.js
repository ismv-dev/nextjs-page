import { NextResponse } from "next/server";
import { getNewsFromDatabase, syncNewsToDatabase, initializeDatabase, getAvailableCategories } from "@/lib/newsSync";

export async function GET(request) {
  try {
    const newsCategories = getAvailableCategories();
    const { searchParams } = request.nextUrl;
    const categories = searchParams.get("categories") ? decodeURIComponent(searchParams.get("categories")).split(",") : null;
    const startDate = searchParams.get("startDate");
    const endDate = searchParams.get("endDate");
    const limit = parseInt(searchParams.get("limit") || "10");
    const offset = parseInt(searchParams.get("offset") || "0");
    const syncParam = searchParams.get("sync") === "true";

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
    let fromDatabase = false;
    let requiresSync = false;

    try {
      items = await getNewsFromDatabase({ categories, startDate, endDate }, limit, offset);
      fromDatabase = items.length > 0;
    } catch (dbError) {
      console.error("Error obteniendo noticias de BD:", dbError);
    }

    // Si no hay datos en BD o se solicita sincronización, sincronizar en el momento
    if (items.length === 0 && offset === 0 || syncParam) {
        if (categories) {
          // Sincronizar todas las categorías seleccionadas
          for (const cat of categories) {
            try {
              await syncNewsToDatabase(cat);
            } catch (error) {
              console.error(`Error sincronizando ${cat}:`, error);
            }
          }
        }

        // Obtener datos sincronizados
        try {
          items = await getNewsFromDatabase({ categories, startDate, endDate }, limit, offset);
          fromDatabase = true;
          requiresSync = false;
        } catch (syncError) {
          console.error("Error sincronizando noticias:", syncError);
          requiresSync = true;

          if (items.length === 0 && offset === 0) {
            return NextResponse.json(
              {
                error: "No se pudieron cargar las noticias. Intente nuevamente.",
                requiresSync: true,
              },
              { status: 502 }
            );
          }
        }
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
      requiresSync,
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
