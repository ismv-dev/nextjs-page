import { NextResponse } from "next/server";
import { getNewsFromDatabase, syncNewsToDatabase, initializeDatabase } from "@/lib/newsSync";

const NEWS_CATEGORIES = ["Todas", "Corporativo", "Cultura"];

export async function GET(request) {
  try {
    const category = request.nextUrl.searchParams.get("category");
    const syncParam = request.nextUrl.searchParams.get("sync") === "true";

    // Validar categoría si se proporciona
    if (category && !NEWS_CATEGORIES.includes(category)) {
      return NextResponse.json({ error: "Categoría no válida" }, { status: 400 });
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
      items = await getNewsFromDatabase(category, 100);
      fromDatabase = items.length > 0;
    } catch (dbError) {
      console.error("Error obteniendo noticias de BD:", dbError);
    }

    // Si no hay datos en BD o se solicita sincronización, sincronizar en el momento
    if (items.length === 0 || syncParam) {
      try {
        if (category && category !== NEWS_CATEGORIES[0]) {
          // Sincronizar solo la categoría solicitada
          await syncNewsToDatabase(category);
        } else {
          // Sincronizar todas las categorías reales (excluyendo "Todas")
          const realCategories = NEWS_CATEGORIES.filter(cat => cat !== "Todas");
          for (const cat of realCategories) {
            try {
              await syncNewsToDatabase(cat);
            } catch (error) {
              console.error(`Error sincronizando ${cat}:`, error);
            }
          }
        }

        // Obtener datos sincronizados
        items = await getNewsFromDatabase(category || null, 100);
        fromDatabase = true;
        requiresSync = false;
      } catch (syncError) {
        console.error("Error sincronizando noticias:", syncError);
        requiresSync = true;

        if (items.length === 0) {
          return NextResponse.json(
            {
              error: "No se pudieron cargar las noticias. Intente nuevamente.",
              requiresSync: true,
              category: category || "todas",
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
      pubDate: item.pub_date,
      imageUrl: item.image_url || "",
      category: item.category,
    }));

    return NextResponse.json({
      category: category || "todas",
      items: formattedItems,
      fromDatabase,
      requiresSync,
      count: formattedItems.length,
      timestamp: new Date().toISOString(),
    });
  } catch (error) {
    console.error("Error en API /news:", error);
    return NextResponse.json(
      { error: "Error interno al procesar noticias", requiresSync: true },
      { status: 500 }
    );
  }
}
