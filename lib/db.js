import { neon } from '@neondatabase/serverless';
import dotenv from "dotenv";
dotenv.config();

const SQL = neon(process.env.DATABASE_URL);

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
    const neonSql = SQL;
    const result = params.length > 0 ? await neonSql.query(sql, params) : await neonSql.query(sql);
    return {
      rows: result,
      rowCount: result.length
    };
  } catch (error) {
    console.error("Error en query:", error);
    throw error;
  }
}

export default SQL;
