"use client";

import { useEffect, useState } from "react";
import NewsArticle from "./NewsArticle.js";

export default function NewsSection({ 
  articles, 
  availableCategories,
  loading, 
  error, 
  lastUpdate, 
  hasMore, 
  fetchNextPage,
  setFilters,
  filters
}) {
  const [categories, setCategories] = useState([]);
  const [specificCategories, setSpecificCategories] = useState([]);
  const [showCategoryFilter, setShowCategoryFilter] = useState(false);
  const [showSpecificCategoryFilter, setShowSpecificCategoryFilter] = useState(false);
  const [showDateFilter, setShowDateFilter] = useState(false);
  const [categorySearch, setCategorySearch] = useState("");
  const [specificCategorySearch, setSpecificCategorySearch] = useState("");

  useEffect(() => {
    if (availableCategories.categories) {
      setCategories([...availableCategories.categories]);
    }
  }, [availableCategories.categories]);
  
  useEffect(() => {
    if (availableCategories.specific_categories) {
      setSpecificCategories([...availableCategories.specific_categories]);
    }
  }, [availableCategories.specific_categories]);

  const toggleCategory = (cat) => {
    const current = filters.selectedCategories || [];
    const next = current.includes(cat) 
      ? current.filter(c => c !== cat) 
      : [...current, cat];
    
    setFilters({ ...filters, selectedCategories: next });
  };

  const toggleSpecificCategory = (cat) => {
    const current = filters.selectedSpecificCategories || [];
    const next = current.includes(cat) 
      ? current.filter(c => c !== cat) 
      : [...current, cat];
    
    setFilters({ ...filters, selectedSpecificCategories: next });
  };

  useEffect(() => {
    if (!hasMore || loading) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting && articles.length >= 10) {
          fetchNextPage();
        }
      },
      { rootMargin: "600px" }
    );

    const sentinel = document.getElementById("news-sentinel");
    if (sentinel) observer.observe(sentinel);

    return () => observer.disconnect();
  }, [hasMore, loading, fetchNextPage]);

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
                  <input type="date" value={filters.startDate} onChange={e => setFilters({ ...filters, startDate: e.target.value })} className="filter-input" />
                </label>
                <label className="filter-label">Hasta: 
                  <input type="date" value={filters.endDate} onChange={e => setFilters({ ...filters, endDate: e.target.value })} className="filter-input" />
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
                {categories.filter(c => c.toLowerCase().includes(categorySearch.toLowerCase())).map(cat => (
                  <label key={cat} className="category-option">
                    <span className="category-name">{cat}</span>
                    <input 
                      type="checkbox" 
                      checked={(filters.selectedCategories || []).includes(cat)} 
                      onChange={() => toggleCategory(cat)} 
                      className="category-checkbox"
                    />
                  </label>
                ))}
              </div>
            )}
          </div>

          <div className="filter-container">
            <button 
              onClick={() => setShowSpecificCategoryFilter(!showSpecificCategoryFilter)}
              className="filter-btn"
            >
              📁 Categorías específicas
            </button>
            {showSpecificCategoryFilter && (
              <div className="filter-dropdown category-filter">
                <input 
                  type="text" 
                  placeholder="Buscar categoría específica..." 
                  value={specificCategorySearch} 
                  onChange={(e) => setSpecificCategorySearch(e.target.value)} 
                  className="category-search-input" 
                />
                {specificCategories.filter(c => c.toLowerCase().includes(specificCategorySearch.toLowerCase())).map(cat => (
                  <label key={cat} className="category-option">
                    <span className="category-name">{cat}</span>
                    <input 
                      type="checkbox" 
                      checked={(filters.selectedSpecificCategories || []).includes(cat)} 
                      onChange={() => toggleSpecificCategory(cat)} 
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

      {loading && (
        <div className="news-status-container">
          <div className="news-spinner spinner-loading" />
          <p className="status-text-loading">Cargando noticias...</p>
        </div>
      )}

      {error && !loading && (
        <p className="error-text news-error-banner">
          ⚠️ {error}
        </p>
      )}

      {!loading && !error && articles.length === 0 && (
        <p className="news-empty-text">
          No se encontraron noticias{(filters.selectedCategories || []).length > 0 ? ` para las categorías seleccionadas` : ""}.
        </p>
      )}

      <div className="news-list">
        {articles.map((article, index) => 
          <NewsArticle article={article} index={index} key={index}/>
        )}
        <div id="news-sentinel" className="news-sentinel" />
      </div>
    </div>
  );
}
