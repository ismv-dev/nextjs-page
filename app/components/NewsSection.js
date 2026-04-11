"use client";

import { useEffect, useState, useRef } from "react";

export default function NewsSection() {
  const [selectedCategories, setSelectedCategories] = useState([]);
  const [categories, setCategories] = useState(["Todas"]);
  const [startDate, setStartDate] = useState(() => {
    const date = new Date();
    date.setMonth(date.getMonth() - 3);
    return date.toISOString().split('T')[0];
  });
  const [endDate, setEndDate] = useState(new Date().toISOString().split('T')[0]);
  const [showCategoryFilter, setShowCategoryFilter] = useState(false);
  const [showDateFilter, setShowDateFilter] = useState(false);
  const [categorySearch, setCategorySearch] = useState("");
  const [articles, setArticles] = useState([]);
  const [loading, setLoading] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState("");
  const [lastUpdate, setLastUpdate] = useState(null);
  const [offset, setOffset] = useState(0);
  const [hasMore, setHasMore] = useState(true);
  const isInitialFetchDone = useRef(false);
  const fetchingRef = useRef(false);

  const LIMIT = 10;

  const fetchNews = async (currentOffset, isInitial, signal) => {
    if (fetchingRef.current) return;
    fetchingRef.current = true;

    if (isInitial) setLoading(true);
    if (isInitial) {
      setSyncing(false);
      setError("");
    }
    try {
      const categoryQuery = selectedCategories.length > 0 
        ? `categories=${encodeURIComponent(selectedCategories.join(','))}` 
        : "";
      const dateQuery = `startDate=${startDate}&endDate=${endDate}`;
      const query = [categoryQuery, dateQuery].filter(Boolean).join('&');
      
      const response = await fetch(`/api/news?${query}&limit=${LIMIT}&offset=${currentOffset}`, {
        signal: signal,
      });

      if (!response.ok) {
        const body = await response.json().catch(() => ({}));
        
        if (body.requiresSync) {
          setSyncing(true);
          return { syncing: true };
        }
        
        throw new Error(body.error || "No se pudo cargar las noticias");
      }

      const data = await response.json();
      const newArticles = data.items || [];
      
      if (data.allCategories && categories.length === 1) {
        setCategories(data.allCategories);
      }
      
      setArticles(prev => isInitial ? newArticles : [...prev, ...newArticles]);
      const hasMoreArticles = newArticles.length === LIMIT;
      setHasMore(hasMoreArticles);
      setLastUpdate(new Date(data.timestamp).toLocaleTimeString("es-ES"));
      setSyncing(false);
      
      return { syncing: false };
    } catch (fetchError) {
      if (fetchError.name !== "AbortError") {
        setError(fetchError.message || "Error al obtener noticias");
        if (isInitial) setArticles([]);
      }
      return { error: true };
    } finally {
      setLoading(false);
      fetchingRef.current = false;
    }
  };

  useEffect(() => {
    const controller = new AbortController();
    
    setOffset(0);
    setHasMore(true);
    
    fetchNews(0, true, controller.signal);

    return () => controller.abort();
  }, [selectedCategories, startDate, endDate]);

  const toggleCategory = (cat) => {
    setSelectedCategories(prev => 
      prev.includes(cat) ? prev.filter(c => c !== cat) : [...prev, cat]
    );
  };

  useEffect(() => {
    if (!hasMore || loading || syncing) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting && !fetchingRef.current) {
          setOffset(prev => {
            const nextOffset = prev + LIMIT;
            fetchNews(nextOffset, false, undefined);
            return nextOffset;
          });
        }
      },
      { rootMargin: "800px" }
    );

    const sentinel = document.getElementById("news-sentinel");
    if (sentinel) observer.observe(sentinel);

    return () => observer.disconnect();
  }, [hasMore, loading, syncing]);

  return (
    <div className="trivia-card">
      <div className="page-title-row">
        <h1 className="page-title">Noticias</h1>
        <div className="news-filters" style={{ display: "flex", gap: "10px" }}>
          <div style={{ position: "relative" }}>
            <button 
              onClick={() => setShowDateFilter(!showDateFilter)}
              className="filter-btn"
              style={{ padding: "8px 12px", borderRadius: "5px", border: "1px solid #ccc", cursor: "pointer", backgroundColor: "white" }}
            >
              📅 Fecha
            </button>
            {showDateFilter && (
              <div style={{ 
                position: "absolute", right: 0, top: "40px", zIndex: 10, 
                backgroundColor: "white", padding: "10px", border: "1px solid #ccc", borderRadius: "5px",
                display: "flex", flexDirection: "column", gap: "5px", boxShadow: "0 2px 5px rgba(0,0,0,0.2)" 
              }}>
                <label style={{ fontSize: "0.8rem" }}>Desde: 
                  <input type="date" value={startDate} onChange={e => setStartDate(e.target.value)} style={{ marginLeft: "5px" }} />
                </label>
                <label style={{ fontSize: "0.8rem" }}>Hasta: 
                  <input type="date" value={endDate} onChange={e => setEndDate(e.target.value)} style={{ marginLeft: "5px" }} />
                </label>
              </div>
            )}
          </div>

          <div style={{ position: "relative" }}>
            <button 
              onClick={() => setShowCategoryFilter(!showCategoryFilter)}
              className="filter-btn"
              style={{ padding: "8px 12px", borderRadius: "5px", border: "1px solid #ccc", cursor: "pointer", backgroundColor: "white" }}
            >
              📁 Categorías
            </button>
            {showCategoryFilter && (
              <div style={{ 
                position: "absolute", right: 0, top: "40px", zIndex: 10, 
                backgroundColor: "white", padding: "15px", border: "1px solid #ccc", borderRadius: "5px",
                minWidth: "250px", maxHeight: "350px", overflowY: "auto", boxShadow: "0 2px 5px rgba(0,0,0,0.2)",
                textAlign: "right"
              }}>
                <input 
                  type="text" 
                  placeholder="Buscar categoría..." 
                  value={categorySearch} 
                  onChange={(e) => setCategorySearch(e.target.value)} 
                  style={{ 
                    width: "100%", marginBottom: "15px", padding: "8px", 
                    borderRadius: "4px", border: "1px solid #ccc", 
                    fontSize: "0.9rem", textAlign: "right", boxSizing: "border-box" 
                  }} 
                />
                {categories.filter(c => c !== "Todas" && c.toLowerCase().includes(categorySearch.toLowerCase())).map(cat => (
                  <label key={cat} style={{ display: "flex", alignItems: "center", justifyContent: "flex-end", fontSize: "1rem", cursor: "pointer", marginBottom: "8px", padding: "4px 0" }}>
                    <span style={{ marginRight: "12px" }}>{cat}</span>
                    <input 
                      type="checkbox" 
                      checked={selectedCategories.includes(cat)} 
                      onChange={() => toggleCategory(cat)} 
                      style={{ width: "18px", height: "18px", cursor: "pointer" }}
                    />
                  </label>
                ))}
              </div>
            )}
          </div>
        </div>
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
          No se encontraron noticias{selectedCategories.length > 0 ? ` para las categorías seleccionadas` : ""}.
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
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "8px" }}>
                {article.category && (
                  <span style={{ 
                    display: "inline-block",
                    fontSize: "0.75rem",
                    padding: "4px 8px",
                    backgroundColor: "#e8f4f8",
                    borderRadius: "3px",
                    color: "#0066cc",
                    fontWeight: "500"
                  }}>
                    {article.category}
                  </span>
                )}
                {article.timestamp && (
                  <p className="news-item-date" style={{ margin: 0 }}>
                    {new Date(article.timestamp).toLocaleString("es-ES", {
                      day: "2-digit",
                      month: "2-digit",
                      year: "numeric",
                      hour: "2-digit",
                      minute: "2-digit",
                    }).replace(",", "")}
                  </p>
                )}
              </div>
              <a href={article.link} target="_blank" rel="noreferrer noopener" className="news-item-title">
                {article.title || "Título no disponible"}
              </a>
              {article.description && <p className="news-item-description">{article.description}</p>}
            </div>
          </article>
        ))}
        <div id="news-sentinel" style={{ height: "10px" }} />
      </div>
    </div>
  );
}
