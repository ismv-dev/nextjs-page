import { query } from "./db.js";

export async function getAllCategories() {
  const result = await query(`SELECT DISTINCT category FROM news`);
  return result.rows.map(r => r.category).filter(Boolean);
}

export async function getAvailableCategories(filters = {}) {
  const { selectedCategories, startDate, endDate } = filters;
  const sql = {
    category: `SELECT DISTINCT category FROM news`,
    specific_category: `SELECT DISTINCT specific_category FROM news`
  };
  const conditions = {
    category: [],
    specific_category: []
  };
  const params = {
    category: [],
    specific_category: []
  };

  if (selectedCategories) {
      if (selectedCategories.length === 1) {
        conditions.specific_category.push(`category = $${params.specific_category.length + 1}`);
        params.specific_category.push(selectedCategories[0]);
      } else {
        conditions.specific_category.push(`category = ANY($${params.specific_category.length + 1})`);
        params.specific_category.push(selectedCategories);
      }
  }

  let result = {
    category: [],
    specific_category: []
  }

  for (let ctype in sql) {
    if (startDate) {
      conditions[ctype].push(`timestamp >= $${params[ctype].length + 1}`);
      const start = new Date(startDate);
      start.setHours(0, 0, 0, 0);
      params[ctype].push(start);
    }

    if (endDate) {
      conditions[ctype].push(`timestamp <= $${params[ctype].length + 1}`);
      const end = new Date(endDate);
      end.setDate(end.getDate() + 1);
      params[ctype].push(end);
    }

    if (conditions[ctype].length > 0) {
      sql[ctype] += ` WHERE ` + conditions[ctype].join(' AND ');
    }
    const db_result = await query(sql[ctype], params[ctype]);
    result[ctype] = db_result.rows.map(r => r[ctype]).filter(Boolean);
  }

  return { categories: result.category, specific_categories: result.specific_category };
}

export async function getNewsFromDatabase(filters = {}, limit = 50, offset = 0) {
  try {
    const { selectedCategories, selectedSpecificCategories, startDate, endDate } = filters;
    let sql = `SELECT * FROM news`;
    const params = [];
    const conditions = [];
    
    if (selectedCategories && selectedCategories.length > 0) {
      if (selectedCategories.length === 1) {
        conditions.push(`category = $${params.length + 1}`);
        params.push(selectedCategories[0]);
      } else {
        conditions.push(`category = ANY($${params.length + 1})`);
        params.push(selectedCategories);
      }
    }

    if (selectedSpecificCategories && selectedSpecificCategories.length > 0) {
      if (selectedSpecificCategories.length === 1) {
        conditions.push(`specific_category = $${params.length + 1}`);
        params.push(selectedSpecificCategories[0]);
      } else {
        conditions.push(`specific_category = ANY($${params.length + 1})`);
        params.push(selectedSpecificCategories);
      }
    }

    if (startDate) {
      conditions.push(`timestamp >= $${params.length + 1}`);
      const start = new Date(startDate);
      start.setHours(0, 0, 0);
      params.push(start);
    }

    if (endDate) {
      conditions.push(`timestamp <= $${params.length + 1}`);
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