
import { parseHTMLDescription } from "../lib/newsUtils";
import { useEffect, useState } from "react";


export default function NewsArticle({ article, index }) {
    const [resume, setResume] = useState(true);
    return (
    <article key={`${article.link}-${index}`} className="news-item">
        {article.imageUrl && (
            <img
            src={article.imageUrl}
            alt={article.title || "Imagen de noticia"}
            className="news-item-image"
            loading="lazy"
            />
        )}
        <div className={resume ? "news-item-content resume" : "news-item-content"}>
            <div className="news-item-meta">
            {article.category && (
                <span className="news-category-tag">
                {article.category}
                </span>
            )}
            {article.timestamp && (
                <p className="news-item-date">
                {new Date(article.timestamp).toLocaleString(undefined, {
                    timeZone: 'UTC',
                    day: "2-digit",
                    month: "2-digit",
                    year: "numeric",
                    hour: "2-digit",
                    minute: "2-digit",
                    hour12: false
                }).replace(',', ' ')}
                </p>
            )}
            </div>
            <a href={article.link} target="_blank" rel="noreferrer noopener" className="news-item-title">
            {article.title || "Título no disponible"}
            </a>
            {article.description && <div className="news-item-description" dangerouslySetInnerHTML={{ __html: parseHTMLDescription(article.description) }} />}

        </div>

        {article.description && article.description.length > 275 && (
            <button 
                style={{backgroundColor: "#00000000", border: "none"}} 
                onClick={() => setResume(!resume)} className="bi-caret-down-fill"
            >
                <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" fill="currentColor" viewBox="0 0 16 16">
                    <path d="M7.247 11.14 2.451 5.658C1.885 5.013 2.345 4 3.204 4h9.592a1 1 0 0 1 .753 1.659l-4.796 5.48a1 1 0 0 1-1.506 0z"/>
                </svg>
            </button>
        )}
    </article>
    );
}