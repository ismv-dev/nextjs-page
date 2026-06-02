import dotenv from "dotenv";
dotenv.config();
import postgres from 'postgres';

const connectionString = process.env.DATABASE_URL
const SQL = postgres(connectionString)

export async function initializeDatabase() {
  const sql = SQL;
  try {
    // Crear tabla de categorías si no existe
    try {
      await sql`
        CREATE TABLE IF NOT EXISTS categories (
          id SERIAL PRIMARY KEY,
          name VARCHAR(100) NOT NULL UNIQUE,
          created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
      `;
    } catch (catError) {
      if (!catError.message.includes('duplicate key value') && !catError.message.includes('already exists')) {
        throw catError;
      }
    }

    // Crear tabla de noticias si no existe
    try {
      await sql`
        CREATE TABLE IF NOT EXISTS news (
          id SERIAL PRIMARY KEY,
          title VARCHAR(500) NOT NULL,
          description TEXT,
          link VARCHAR(1000) NOT NULL UNIQUE,
          timestamp TIMESTAMPTZ DEFAULT NOW(),
          image_url VARCHAR(1000),
          category VARCHAR(100) NOT NULL,
          created_at TIMESTAMPTZ DEFAULT NOW()
        );
      `;
    } catch (tableError) {

      // Ignorar errores de tabla ya existente o secuencia duplicada
      if (!tableError.message.includes('duplicate key value') && !tableError.message.includes('already exists')) {
        throw tableError;
      }
    }

    // Crear índices para mejor performance (uno por uno)
    await sql`CREATE INDEX IF NOT EXISTS idx_news_category ON news(category)`;
    await sql`CREATE INDEX IF NOT EXISTS idx_news_timestamp ON news(timestamp DESC)`;
    await sql`CREATE INDEX IF NOT EXISTS idx_news_created_at ON news(created_at DESC)`;

    console.log("DB cargada");
  } catch (error) {
    console.error("Error en creacion:", error);
    throw error;
  }
}

export async function query(sql, params = []) {
  try {
    const client = SQL;
    let result;

    if (params && params.length > 0) {
      result = await client.unsafe(sql, params);
    } else {
      result = await client.unsafe(sql);
    }
    const rows = Array.isArray(result) ? result : (result && result.rows) ? result.rows : [];
    const rowCount = Array.isArray(rows) ? rows.length : (result && typeof result.rowCount === 'number' ? result.rowCount : 0);

    return {
      rows,
      rowCount
    };
  } catch (error) {
    console.error("Error en query:", error);
    throw error;
  }
}

export default SQL;
