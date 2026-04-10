import { query } from "./db.js";
import { initializeDatabase as initDB } from "./db.js";

const NEWS_XMLS = {
  Corporativo: ["https://cooperativa.cl/noticias/site/tax/port/all/rss_16___1.xml"],
  Cultura: ["https://cooperativa.cl/noticias/site/tax/port/all/rss_5___1.xml"],
};

const decodeHtmlEntities = (value) => {
  return value
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&nbsp;/g, " ");
};

const stripHtmlTags = (value) => value.replace(/<[^>]+>/g, "").trim();

const extractTag = (xml, tagName) => {
  const startTag = `<${tagName}>`;
  const endTag = `</${tagName}>`;
  const startIndex = xml.indexOf(startTag);
  if (startIndex === -1) return "";

  const endIndex = xml.indexOf(endTag, startIndex);
  if (endIndex === -1) return "";

  let content = xml.substring(startIndex + startTag.length, endIndex).trim();

  if (content.startsWith("<![CDATA[") && content.endsWith("]]>")) {
    content = content.substring(9, content.length - 3);
  }

  return decodeHtmlEntities(stripHtmlTags(content));
};

const extractMediaUrl = (xml) => {
  const mediaContentStart = xml.indexOf("<media:content");
  if (mediaContentStart === -1) return "";

  const urlMatch = xml
    .substring(mediaContentStart, mediaContentStart + 200)
    .match(/url="([^"]+)"/);
  return urlMatch ? urlMatch[1] : "";
};

const parseRssItems = (xmlText) => {
  const items = [];
  const itemStart = xmlText.indexOf("<item>");
  if (itemStart === -1) return items;

  let remaining = xmlText.substring(itemStart);
  let itemCount = 0;

  while (remaining.includes("<item>") && itemCount < 50) {
    const itemEnd = remaining.indexOf("</item>") + 7;
    if (itemEnd === 6) break;

    const itemText = remaining.substring(0, itemEnd);
    remaining = remaining.substring(itemEnd);

    const title = extractTag(itemText, "title");
    const link = extractTag(itemText, "link");
    const pubDateRaw = extractTag(itemText, "pubDate");
    const pubDate = pubDateRaw ? new Date(pubDateRaw) : new Date();
    const description = extractTag(itemText, "description") || extractTag(itemText, "descent") || extractTag(itemText, "content:encoded");
    const imageUrl = extractMediaUrl(itemText);
    const fecha = new Date();

    if (title && link && (pubDate.getFullYear() === fecha.getFullYear()) && (pubDate.getMonth() >= fecha.getMonth() - 2)) {
      items.push({
        title,
        link,
        pubDate,
        description,
        imageUrl,
      });
    }

    itemCount++;
  }

  return items;
};

export async function fetchNewsFromRss(category, dateLastNew) {
  try {
    if (!NEWS_XMLS[category]) {
      throw new Error(`Categoría no válida: ${category}`);
    }

    const rssUrl = NEWS_XMLS[category][0];
    const response = await fetch(rssUrl, { cache: "no-store" });

    if (!response.ok) {
      throw new Error(`Error en RSS: ${response.status}`);
    }

    const text = await response.text();
    const items = parseRssItems(text);

    return items;
  } catch (error) {
    console.error(`Error fetching RSS for ${category}:`, error);
    throw error;
  }
}

export async function getLatestNewDate(category) {
  if (!category) return null;
  const result = await query(`
    SELECT MAX(pub_date) as last_date 
    FROM news 
    WHERE category = $1;
  `, [category]);

  return result.rows?.[0]?.last_date || null;
}

export async function syncNewsToDatabase(category) {
  try {
      const dateLastNew = await getLatestNewDate(category);
      const items = await fetchNewsFromRss(category, dateLastNew);

    if (!items || items.length === 0) {
      return 0;
    }

    let insertedCount = 0;

    for (const item of items) {
      try {
        const result = await query(
          `INSERT INTO news (title, description, link, pub_date, image_url, category)
           VALUES ($1, $2, $3, $4, $5, $6)
           ON CONFLICT (link) DO UPDATE SET
           title = $1,
           description = $2,
           pub_date = $4,
           image_url = $5
           RETURNING id;`,
          [
            item.title,
            item.description,
            item.link,
            item.pubDate,
            item.imageUrl,
            category,
          ]
        );

        if (result.rowCount > 0) {
          insertedCount++;
        }
      } catch (error) {
        console.error(`Error insertando noticia: ${item.title}`, error);
      }
    }
    return insertedCount;
  } catch (error) {
    console.error(`Error sincronizando ${category}:`, error);
    throw error;
  }
}

export async function syncAllNews() {
  console.log("Sincronizando base de datos");

  const categories = Object.keys(NEWS_XMLS);
  const results = {};

  for (const category of categories) {
    try {
      results[category] = await syncNewsToDatabase(category);
    } catch (error) {
      console.error(`Error en categoría ${category}:`, error);
      results[category] = 0;
    }
  }

  console.log("Sincronizacion completa");
  return results;
}

export async function getNewsFromDatabase(category = null, limit = 50) {
  try {
    let sql = `SELECT * FROM news`;
    const params = [];
    
    console.log(category)
    if (category) {
      sql += ` WHERE category = $1`;
      params.push(category);
    }

    sql += ` ORDER BY pub_date DESC, created_at DESC LIMIT $${params.length + 1}`;
    params.push(limit);

    const result = await query(sql, params);
    console.log(result)
    return result.rows;
  } catch (error) {
    console.error("Error obteniendo noticias de BD:", error);
    return [];
  }
}

export { initDB as initializeDatabase };
