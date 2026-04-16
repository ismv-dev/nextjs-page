import { NextResponse } from "next/server";
import { initializeDatabase } from "@/lib/db.js";
import { getNewsFromDatabase, getAvailableCategories, getAllCategories } from "@/lib/getNews.js";

export async function GET(request) {
  try {
    const { searchParams } = request.nextUrl;
    const selectedCategories = searchParams.get("categories") !== null ? decodeURIComponent(searchParams.get("categories")).split(",") : null;
    const selectedSpecificCategories = searchParams.get("specific_categories") !== null ? decodeURIComponent(searchParams.get("specific_categories")).split(",") : null;
    const startDate = searchParams.get("startDate");
    const endDate = searchParams.get("endDate");
    const filters = { selectedCategories, selectedSpecificCategories, startDate, endDate };
    let availableCategories;

    try {
      availableCategories = await getAvailableCategories(filters);
    } catch (dbError) {
      console.error("Error obteniendo categorias de BD:", dbError);
    }
    
    // Validar y limitar paginación para evitar ataques de denegación de servicio (DoS)
    const rawLimit = parseInt(searchParams.get("limit") || "10");
    const limit = Math.max(rawLimit, 1)
    
    const rawOffset = parseInt(searchParams.get("offset") || "0");
    const offset = Math.max(rawOffset, 0);
    
    // REMOVED: syncParam = searchParams.get("sync") === "true" to prevent DoS attacks via public endpoint

    // Validar categorías si se proporcionan
    if (selectedCategories && selectedCategories.length > 0) {
      const invalidCategories = selectedCategories.filter(cat => !availableCategories.categories.includes(cat));
      if (invalidCategories.length > 0) {
        return NextResponse.json({ error: "Una o más categorías no son válidas" }, { status: 400 });
      }
    }

    if (selectedSpecificCategories && selectedSpecificCategories.length > 0) {
      const invalidCategories = selectedSpecificCategories.filter(cat => !availableCategories.specific_categories.includes(cat));
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
      console.log(filters);
      items = await getNewsFromDatabase(filters, limit, offset);
      fromDatabase = items.length > 0;
    } catch (dbError) {
      console.error("Error obteniendo noticias de BD:", dbError);
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
      specific_category: item.specific_category || ""
    }));

    return NextResponse.json({
      availableCategories,
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
