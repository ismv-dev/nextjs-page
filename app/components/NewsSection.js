"use client";

import { useEffect, useState } from "react";

const NEWS_CATEGORIES = [
  "Todas",
  "Corporativo",
  "Cultura"
];

export default function NewsSection() {
  const [category, setCategory] = useState("Todas");
  const [articles, setArticles] = useState([]);
  const [loading, setLoading] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState("");
  const [lastUpdate, setLastUpdate] = useState(null);

  useEffect(() => {
    const controller = new AbortController();
    let retryCount = 0;
    let retryTimeout;

    const fetchNews = async () => {
      setLoading(true);
      setSyncing(false);
      setError("");
      try {
        // Construir query de categoría
        const query = category === "Todas" ? "" : `category=${encodeURIComponent(category)}`;
        const response = await fetch(`/api/news?${query}`, {
          signal: controller.signal,
        });

        if (!response.ok) {
          const body = await response.json().catch(() => ({}));
          
          // Si requiere sincronización, mostrar loading de sincronización
          if (body.requiresSync && retryCount < 3) {
            setSyncing(true);
            // Reintentar después de mostrar el estado de sincronización
            retryCount++;
            retryTimeout = setTimeout(() => {
              if (!controller.signal.aborted) {
                fetchNews();
              }
            }, 2000);
            return;
          }
          
          throw new Error(body.error || "No se pudo cargar las noticias");
        }

        const data = await response.json();
        setArticles(data.items || []);
        setLastUpdate(new Date(data.timestamp).toLocaleTimeString("es-ES"));
        setSyncing(false);
        retryCount = 0;
      } catch (fetchError) {
        if (fetchError.name !== "AbortError") {
          setError(fetchError.message || "Error al obtener noticias");
          setArticles([]);
        }
      } finally {
        setLoading(false);
      }
    };

    fetchNews();
    return () => {
      controller.abort();
      if (retryTimeout) clearTimeout(retryTimeout);
    };
  }, [category]);

  return (
    <div className="trivia-card">
      <div className="page-title-row">
        <h1 className="page-title">Noticias</h1>
        <select
          value={category}
          onChange={(event) => setCategory(event.target.value)}
          className="news-select"
          aria-label="Seleccionar tipo de noticia"
          disabled={loading || syncing}
        >
          {NEWS_CATEGORIES.map((item) => (
            <option key={item} value={item}>
              {item}
            </option>
          ))}
        </select>
      </div>

      {lastUpdate && (
        <p style={{ fontSize: "0.85rem", color: "#666", marginBottom: "10px" }}>
          Última actualización: {lastUpdate}
        </p>
      )}

      {syncing && (
        <div style={{ textAlign: "center", padding: "20px" }}>
          <div style={{
            display: "inline-block",
            width: "30px",
            height: "30px",
            border: "4px solid #f3f3f3",
            borderTop: "4px solid #3498db",
            borderRadius: "50%",
            animation: "spin 1s linear infinite",
          }} />
          <p style={{ marginTop: "10px", color: "#3498db" }}>
            Sincronizando noticias de la web...
          </p>
          <style>{`
            @keyframes spin {
              0% { transform: rotate(0deg); }
              100% { transform: rotate(360deg); }
            }
          `}</style>
        </div>
      )}

      {loading && !syncing && (
        <div style={{ textAlign: "center", padding: "20px" }}>
          <div style={{
            display: "inline-block",
            width: "30px",
            height: "30px",
            border: "4px solid #f3f3f3",
            borderTop: "4px solid #2ecc71",
            borderRadius: "50%",
            animation: "spin 1s linear infinite",
          }} />
          <p style={{ marginTop: "10px", color: "#2ecc71" }}>Cargando noticias...</p>
        </div>
      )}

      {error && !loading && !syncing && (
        <p className="error-text" style={{ textAlign: "center", padding: "15px", backgroundColor: "#ffe6e6", borderRadius: "5px" }}>
          ⚠️ {error}
        </p>
      )}

      {!loading && !syncing && !error && articles.length === 0 && (
        <p style={{ textAlign: "center", padding: "20px", color: "#999" }}>
          No se encontraron noticias{category !== "Todas" ? ` para ${category}` : ""}.
        </p>
      )}

      <div className="news-list">
        {articles.map((article, index) => (
          <article key={`${article.link}-${index}`} className="news-item">
            {article.imageUrl && (
              <img
                src={article.imageUrl}
                alt={article.title || "Imagen de noticia"}
                className="news-item-image"
                loading="lazy"
              />
            )}
            <div className="news-item-content">
              {article.category && (
                <span style={{ 
                  display: "inline-block",
                  fontSize: "0.75rem",
                  padding: "4px 8px",
                  backgroundColor: "#e8f4f8",
                  borderRadius: "3px",
                  color: "#0066cc",
                  marginBottom: "8px",
                  fontWeight: "500"
                }}>
                  {article.category}
                </span>
              )}
              {article.pubDate && (
                <p className="news-item-date">
                  {new Date(article.pubDate).toLocaleString("es-ES", {
                    day: "2-digit",
                    month: "2-digit",
                    year: "numeric",
                    hour: "2-digit",
                    minute: "2-digit",
                  }).replace(",", "")}
                </p>
              )}
              <a href={article.link} target="_blank" rel="noreferrer noopener" className="news-item-title">
                {article.title || "Título no disponible"}
              </a>
              {article.description && <p className="news-item-description">{article.description}</p>}
            </div>
          </article>
        ))}
      </div>
    </div>
  );
}
