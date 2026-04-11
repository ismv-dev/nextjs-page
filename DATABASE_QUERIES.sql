-- ==========================================
-- CONSULTAS ÚTILES PARA LA BASE DE DATOS DE NOTICIAS
-- ==========================================

-- 1. VER TODAS LAS NOTICIAS RECIENTEMENTE SINCRONIZADAS
SELECT id, title, timestamp, category, created_at 
FROM news 
ORDER BY timestamp DESC 
LIMIT 20;

-- 2. CONTAR NOTICIAS POR CATEGORÍA
SELECT category, COUNT(*) as total
FROM news
GROUP BY category
ORDER BY total DESC;

-- 3. ÚLTIMAS 10 NOTICIAS DE UNA CATEGORÍA
SELECT title, description, link, timestamp, image_url
FROM news
WHERE category = 'Corporativo'
ORDER BY timestamp DESC
LIMIT 10;

-- 4. NOTICIAS DE LOS ÚLTIMOS 7 DÍAS
SELECT title, category, timestamp
FROM news
WHERE timestamp >= NOW() - INTERVAL '7 days'
ORDER BY timestamp DESC;

-- 5. VER HISTÓRICO DE SINCRONIZACIONES
SELECT category, last_sync, items_count, status
FROM sync_logs
ORDER BY last_sync DESC
LIMIT 20;

-- 6. SINCRONIZACIÓN MÁS RECIENTE POR CATEGORÍA
SELECT category, MAX(last_sync) as última_sincronización
FROM sync_logs
WHERE status = 'success'
GROUP BY category;

-- 7. BUSCAR NOTICIA POR TÍTULO
SELECT title, link, timestamp, category
FROM news
WHERE LOWER(title) LIKE LOWER('%palabra clave%')
ORDER BY timestamp DESC;

-- 8. VER NOTICIAS SIN IMAGEN
SELECT id, title, category, timestamp
FROM news
WHERE image_url IS NULL OR image_url = ''
LIMIT 20;

-- 9. ELIMINAR NOTICIAS DUPLICADAS (GUARDAR LA MÁS RECIENTE)
DELETE FROM news 
WHERE id NOT IN (
  SELECT MAX(id) FROM news 
  GROUP BY link
);

-- 10. LIMPIAR NOTICIAS MÁS ANTIGUAS DE 30 DÍAS
DELETE FROM news 
WHERE created_at < NOW() - INTERVAL '30 days';

-- 11. ESTADÍSTICAS DE LA BASE DE DATOS
SELECT 
  'Total de noticias' as métrica,
  COUNT(*) as valor
FROM news
UNION ALL
SELECT 'Categorías diferentes' as métrica,
COUNT(DISTINCT category) as valor
FROM news
UNION ALL
SELECT 'Noticias con imagen' as métrica,
COUNT(*) as valor
FROM news
WHERE image_url IS NOT NULL AND image_url != '';

-- 12. ARTÍCULOS MÁS POPULARES (POR CATEGORÍA MÁS FRECUENTE)
SELECT category, COUNT(*) as cantidad
FROM news
GROUP BY category
ORDER BY cantidad DESC;

-- 13. TABLA DE ÍNDICES Y PERFORMANCE
SELECT schemaname, tablename, indexname
FROM pg_indexes
WHERE schemaname != 'pg_toast'
ORDER BY tablename, indexname;

-- 14. VACIAR COMPLETAMENTE LA TABLA (CUIDADO)
-- DELETE FROM news;
-- DELETE FROM sync_logs;
-- RESTART IDENTITY;

-- 15. VER TAMAÑO DE LA BASE DE DATOS
SELECT pg_size_pretty(pg_total_relation_size('news')) as tamaño_tabla_news;

-- ==========================================
-- QUERIES PARA DEBUGGING
-- ==========================================

-- Ver última sincronización de cada categoría
SELECT 
  sl.category,
  sl.last_sync,
  sl.items_count,
  COUNT(n.id) as articulos_en_db
FROM sync_logs sl
LEFT JOIN news n ON sl.category = n.category
GROUP BY sl.category, sl.last_sync, sl.items_count
ORDER BY sl.last_sync DESC;

-- Detectar problemas de conexión
SELECT version();

-- Ver conexiones activas
SELECT datname, usename, state, count(*)
FROM pg_stat_activity
GROUP BY datname, usename, state;
