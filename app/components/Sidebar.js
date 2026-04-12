"use client";

export default function Sidebar({ view, theme, onViewChange, onThemeChange }) {
  return (
    <aside className="sidebar">
      <nav className="sidebar-nav">
        <button
          type="button"
          className={`sidebar-button ${view === "trivias" ? "active" : ""}`}
          onClick={() => onViewChange("trivias")}
        >
          Trivias
        </button>
        <button
          type="button"
          className={`sidebar-button ${view === "operaciones" ? "active" : ""}`}
          onClick={() => onViewChange("operaciones")}
        >
          Aritmética
        </button>
        <button
          type="button"
          className={`sidebar-button ${view === "juegos" ? "active" : ""}`}
          onClick={() => onViewChange("juegos")}
        >
          Juegos
        </button>
        <button
          type="button"
          className={`sidebar-button ${view === "noticias" ? "active" : ""}`}
          onClick={() => onViewChange("noticias")}
        >
          Noticias
        </button>
      </nav>
      <div className="theme-switch-wrapper">
        <button
          type="button"
          className={`theme-toggle-button ${theme}`}
          onClick={() => onThemeChange(theme === "dark" ? "light" : "dark")}
        >
          {theme === "dark" ? "☀️" : "🌙"}
        </button>
      </div>
    </aside>
  );
}
