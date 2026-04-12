import { query } from "./db.js";

export default async function getNewsFromDatabase(filters = {}, limit = 50, offset = 0) {
  try {
    const { categories, startDate, endDate } = filters;
    let sql = `SELECT * FROM news`;
    const params = [];
    const conditions = [];
    
    if (categories && Array.isArray(categories) && categories.length > 0) {
      if (categories.length > 1) {
        conditions.push(`category = ANY($${params.length + 1})`);
        params.push(categories);
      } else {
        conditions.push(`category = $${params.length + 1}`);
        params.push(categories[0]);
      }
    }

    if (startDate) {
      conditions.push(`timestamp >= $${params.length + 1}`);
      const start = new Date(startDate);
      start.setHours(0, 0, 0, 0);
      params.push(start);
    }

    if (endDate) {
      conditions.push(`timestamp < $${params.length + 1}`);
      const end = new Date(endDate);
      end.setDate(end.getDate() + 1);
      params.push(end);
    }

    if (conditions.length > 0) {
      sql += ` WHERE ` + conditions.join(' AND ');
    }

    sql += ` ORDER BY timestamp DESC LIMIT $${params.length + 1} OFFSET $${params.length + 2}`;
    params.push(limit, offset);

    const result = await query(sql, params);
    return result.rows;
  } catch (error) {
    console.error("Error obteniendo noticias de BD:", error);
    return [];
  }
}