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
      setLastUpdate(new Date(data.timestamp).toLocaleTimeString("es-CL"));
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
    <div className="news-card">
      <div className="page-title-row">
        <h1 className="page-title">Noticias</h1>
        <div className="news-filters">
          <div className="filter-container">
            <button 
              onClick={() => setShowDateFilter(!showDateFilter)}
              className="filter-btn"
            >
              📅 Fecha
            </button>
            {showDateFilter && (
              <div className="filter-dropdown date-filter">
                <label className="filter-label">Desde: 
                  <input type="date" value={startDate} onChange={e => setStartDate(e.target.value)} className="filter-input" />
                </label>
                <label className="filter-label">Hasta: 
                  <input type="date" value={endDate} onChange={e => setEndDate(e.target.value)} className="filter-input" />
                </label>
              </div>
            )}
          </div>

          <div className="filter-container">
            <button 
              onClick={() => setShowCategoryFilter(!showCategoryFilter)}
              className="filter-btn"
            >
              📁 Categorías
            </button>
            {showCategoryFilter && (
              <div className="filter-dropdown category-filter">
                <input 
                  type="text" 
                  placeholder="Buscar categoría..." 
                  value={categorySearch} 
                  onChange={(e) => setCategorySearch(e.target.value)} 
                  className="category-search-input" 
                />
                {categories.filter(c => c !== "Todas" && c.toLowerCase().includes(categorySearch.toLowerCase())).map(cat => (
                  <label key={cat} className="category-option">
                    <span className="category-name">{cat}</span>
                    <input 
                      type="checkbox" 
                      checked={selectedCategories.includes(cat)} 
                      onChange={() => toggleCategory(cat)} 
                      className="category-checkbox"
                    />
                  </label>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>

      {lastUpdate && (
        <p className="news-last-update">
          Última actualización: {lastUpdate}
        </p>
      )}

      {syncing && (
        <div className="news-status-container">
          <div className="news-spinner spinner-sync" />
          <p className="status-text-sync">
            Sincronizando noticias de la web...
          </p>
        </div>
      )}

      {loading && !syncing && (
        <div className="news-status-container">
          <div className="news-spinner spinner-loading" />
          <p className="status-text-loading">Cargando noticias...</p>
        </div>
      )}

      {error && !loading && !syncing && (
        <p className="error-text news-error-banner">
          ⚠️ {error}
        </p>
      )}

      {!loading && !syncing && !error && articles.length === 0 && (
        <p className="news-empty-text">
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
              <div className="news-item-meta">
                {article.category && (
                  <span className="news-category-tag">
                    {article.category}
                  </span>
                )}
                {article.timestamp && (
                  <p className="news-item-date">
                    {new Date(article.timestamp).toLocaleString("es-CL", {
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
        <div id="news-sentinel" className="news-sentinel" />
      </div>
    </div>
  );
}
