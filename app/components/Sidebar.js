"use client";

export default function Sidebar({ view, theme, onViewChange, onThemeChange }) {
  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <h2 className="sidebar-title">Menú</h2>
      </div>
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
        <span className="theme-label">Modo oscuro</span>
        <label className="theme-switch">
          <input
            type="checkbox"
            checked={theme === "dark"}
            onChange={() => onThemeChange(theme === "dark" ? "light" : "dark")}
          />
          <span className="slider round" />
        </label>
      </div>
    </aside>
  );
}
