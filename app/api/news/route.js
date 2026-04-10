import { NextResponse } from "next/server";

const NEWS_XMLS = {'Corporativo': ['https://cooperativa.cl/noticias/site/tax/port/all/rss_16___1.xml'], 'Cultura': ['https://cooperativa.cl/noticias/site/tax/port/all/rss_5___1.xml']}
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

  // Handle CDATA sections
  if (content.startsWith('<![CDATA[') && content.endsWith(']]>')) {
    content = content.substring(9, content.length - 3);
  }

  return decodeHtmlEntities(stripHtmlTags(content));
};

const extractMediaUrl = (xml) => {
  const mediaContentStart = xml.indexOf('<media:content');
  if (mediaContentStart === -1) return "";

  const urlMatch = xml.substring(mediaContentStart, mediaContentStart + 200).match(/url="([^"]+)"/);
  return urlMatch ? urlMatch[1] : "";
};

const parseRssItems = (xmlText) => {
  // Simple approach: split by <item> and </item>
  const items = [];
  const itemStart = xmlText.indexOf('<item>');
  if (itemStart === -1) return items;

  let remaining = xmlText.substring(itemStart);
  let itemCount = 0;

  while (remaining.includes('<item>') && itemCount < 10) {
    const itemEnd = remaining.indexOf('</item>') + 7; // include </item>
    if (itemEnd === 6) break; // no more items

    const itemText = remaining.substring(0, itemEnd);
    remaining = remaining.substring(itemEnd);

    const title = extractTag(itemText, "title");
    const link = extractTag(itemText, "link");
    const pubDate = extractTag(itemText, "pubDate");
    const description = extractTag(itemText, "descent") || extractTag(itemText, "description") || extractTag(itemText, "content:encoded");
    const imageUrl = extractMediaUrl(itemText);

    items.push({
      title,
      link,
      pubDate,
      description,
      imageUrl,
    });

    itemCount++;
  }

  return items;
};

export async function GET(request) {
  try {
    const category = request.nextUrl.searchParams.get("category") || "Corporativo";
    if (!NEWS_XMLS[category]) {
      return NextResponse.json({ error: "Categoría no válida" }, { status: 400 });
    }

    const response = await fetch(NEWS_XMLS[category], { cache: "no-store" });
    if (!response.ok) {
      return NextResponse.json({ error: "No se pudo cargar el feed RSS" }, { status: 502 });
    }

    const text = await response.text();
    const items = parseRssItems(text);

    return NextResponse.json({ category, items });
  } catch {
    return NextResponse.json({ error: "Error interno al procesar noticias" }, { status: 500 });
  }
}
