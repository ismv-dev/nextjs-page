"use client";

import { useEffect, useState } from "react";

const NEWS_CATEGORIES = [
  "Corporativo",
  "Cultura",
  "Deportes",
  "Economia",
  "Entretencion",
  "Mundo",
  "Pais",
  "Sociedad",
  "Tecnologia",
];

export default function NewsSection() {
  const [category, setCategory] = useState("Corporativo");
  const [articles, setArticles] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    const controller = new AbortController();
    const fetchNews = async () => {
      setLoading(true);
      setError("");
      try {
        const response = await fetch(`/api/news?category=${encodeURIComponent(category)}`, {
          signal: controller.signal,
        });

        if (!response.ok) {
          const body = await response.json().catch(() => ({}));
          throw new Error(body.error || "No se pudo cargar la noticia");
        }

        const data = await response.json();
        setArticles(data.items || []);
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
    return () => controller.abort();
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
        >
          {NEWS_CATEGORIES.map((item) => (
            <option key={item} value={item}>
              {item}
            </option>
          ))}
        </select>
      </div>

      {loading && <p>Cargando noticias...</p>}
      {error && <p className="error-text">{error}</p>}
      {!loading && !error && articles.length === 0 && (
        <p>No se encontraron noticias para esta categoría.</p>
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
