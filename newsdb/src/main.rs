// ─── Tipos ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct NewsItem {
    title:       String,
    description: String,
    link:        String,
    timestamp:   Option<chrono::DateTime<chrono::Utc>>,
    image_url:   String,
    category:     String,
    specific_category:    String,
}

// ─── Fetch de un feed individual ──────────────────────────────────────────────

async fn fetch_feed(
    client:    &reqwest::Client,
    // OPTIMIZACIÓN 4: Semáforo compartido — cada llamada adquiere un permiso
    // antes de abrir el socket. El permit se libera al salir del scope,
    // garantizando que nunca haya más de MAX_CONCURRENT_FETCHES en vuelo.
    semaphore: &Semaphore,
    url:       &str,
    specific_category:  &str,
    tx:        &mpsc::Sender<NewsItem>,
) -> Result<usize> {
    let _permit = semaphore.acquire().await?;

    let response = match client.get(url).send().await {
        Ok(r)  => r,
        Err(e) => {
            eprintln!("  [HTTP] {specific_category}: {url} — {e}");
            return Ok(0);
        }
    };

    let status = response.status();
    if !status.is_success() {
        eprintln!("  [HTTP {}] {specific_category}: {url}", status.as_u16());
        return Ok(0);
    }

    // OPTIMIZACIÓN 5: bytes() en lugar de text() evita una decodificación UTF-8
    // intermedia; Channel::read_from parsea directamente desde bytes.
    let bytes   = response.bytes().await?;
    let channel = Channel::read_from(&bytes[..])?;

    let mut count = 0;
    for item in channel.items() {
        let image_url = item
            .extensions()
            .get("media")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.first())
            .and_then(|c| c.attrs().get("url"))
            .map(|s| s.to_string())
            .unwrap_or_default();

        let link_str = item.link().unwrap_or("");

        let timestamp = item
            .extensions()
            .get("")
            .and_then(|exts| exts.get("timestamp"))
            .and_then(|v| v.first())
            .and_then(|v| v.value())
            .and_then(|ts| {
                chrono::NaiveDateTime::parse_from_str(ts, "%Y%m%d%H%M%S")
                    .ok()
                    .map(|ndt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(ndt, chrono::Utc))
            })
            .or_else(|| parse_timestamp_from_link(link_str));

        let category = item
            .categories()
            .first()
            .map(|c| c.name().to_string())
            .unwrap_or_else(|| specific_category.to_string());

        let title = item
            .title()
            .unwrap_or("Sin título")
            .to_string();

        let news = NewsItem {
            title: title.clone(),
            description: item.description().unwrap_or(item.extensions().get("").and_then(|inner| inner.get("descent")).and_then(|v| v.first()).and_then(|e| e.value.as_deref()).unwrap_or(&title)).to_string(),
            link:        link_str.to_string(),
            timestamp,
            image_url,
            category,
            specific_category:    specific_category.to_string(),
        };

        // OPTIMIZACIÓN 6: try_send + back-off suave.
        // Si el channel está lleno, esperamos brevemente en lugar de bloquear
        // la tarea. Esto preserva la concurrencia del runtime de Tokio.
        if tx.send(news).await.is_err() {
            // El receiver se cerró (db_writer terminó prematuramente).
            break;
        }
        count += 1;
    }
    Ok(count)
}

fn parse_timestamp_from_link(link: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let date_segment = link.split('/').find(|seg| {
        seg.len() == 10
            && seg.as_bytes().get(4) == Some(&b'-')
            && seg.as_bytes().get(7) == Some(&b'-')
    })?;

    let after_date   = link.split_once(date_segment)?.1;
    let time_segment = after_date
        .trim_start_matches('/')
        .split('.')
        .next()
        .filter(|seg| seg.len() == 6 && seg.chars().all(|c| c.is_ascii_digit()))?;

    let combined = format!("{date_segment} {time_segment}");
    chrono::NaiveDateTime::parse_from_str(&combined, "%Y-%m-%d %H%M%S ")
        .ok()
        .map(|ndt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(ndt, chrono::Utc))
}

// ─── Sincronización de una categoría ──────────────────────────────────────────

async fn sync_category(
    client:    Arc<reqwest::Client>,
    semaphore: Arc<Semaphore>,
    specific_category:  &'static str,
    urls:      &'static [&'static str],
    tx:        mpsc::Sender<NewsItem>,
) {
    // FuturesUnordered: las futures progresan en paralelo, sin orden fijo.
    let mut futures: FuturesUnordered<_> = urls
        .iter()
        .map(|url| fetch_feed(&client, &semaphore, url, specific_category, &tx))
        .collect();

    while let Some(result) = futures.next().await {
        if let Err(e) = result {
            eprintln!("  [!] Error en {specific_category}: {e}");
        }
    }
}

// ─── Writer de base de datos ───────────────────────────────────────────────────

// OPTIMIZACIÓN 7: `select!` en lugar de `timeout` anidado.
// El patrón anterior usaba `timeout(BATCH_TIMEOUT, rx.recv())` que crea un
// Future de timeout cada iteración. Con `select!`, Tokio registra ambas ramas
// una sola vez por iteración del loop y cancela limpiamente la que no gana.
async fn db_writer(pool: PgPool, mut rx: mpsc::Receiver<NewsItem>, total: Arc<AtomicU64>) {
    let mut batch: Vec<NewsItem> = Vec::with_capacity(BATCH_SIZE);
    let mut interval = tokio::time::interval(BATCH_TIMEOUT);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // Consumir el primer tick inmediato para que el temporizador empiece
    // a contar desde ahora, no desde antes de la primera lectura.
    interval.tick().await;

    loop {
        tokio::select! {
            biased; // Prioridad a recibir ítems mientras el channel tenga datos.

            item = rx.recv() => {
                match item {
                    Some(news) => {
                        batch.push(news);
                        if batch.len() >= BATCH_SIZE {
                            flush_batch(&pool, &mut batch, &total).await;
                        }
                    }
                    None => {
                        // Channel cerrado: vaciar batch pendiente y salir.
                        if !batch.is_empty() {
                            flush_batch(&pool, &mut batch, &total).await;
                        }
                        return;
                    }
                }
            }

            _ = interval.tick() => {
                // Timeout: forzar flush aunque el batch esté incompleto.
                if !batch.is_empty() {
                    flush_batch(&pool, &mut batch, &total).await;
                }
            }
        }
    }
}

// ─── Flush de un batch a PostgreSQL ───────────────────────────────────────────

async fn flush_batch(pool: &PgPool, batch: &mut Vec<NewsItem>, total: &AtomicU64) {
    let n = batch.len();

    let titles:       Vec<&str>                         = batch.iter().map(|i| i.title.as_str()).collect();
    let descriptions: Vec<&str>                         = batch.iter().map(|i| i.description.as_str()).collect();
    let links:        Vec<&str>                         = batch.iter().map(|i| i.link.as_str()).collect();
    let timestamps:   Vec<chrono::DateTime<chrono::Utc>> =
        batch.iter().map(|i| i.timestamp.unwrap_or_else(chrono::Utc::now)).collect();
    let images:       Vec<&str>                         = batch.iter().map(|i| i.image_url.as_str()).collect();
    let category:   Vec<&str>                         = batch.iter().map(|i| i.category.as_str()).collect();
    let specific_category:   Vec<&str>                         = batch.iter().map(|i| i.specific_category.as_str()).collect();

    let result = sqlx::query!(
        r#"
        INSERT INTO news (title, description, link, timestamp, image_url, category, specific_category)
        SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::timestamptz[], $5::text[], $6::text[], $7::text[])
        ON CONFLICT (link) DO NOTHING
        "#,
        &titles       as &[&str],
        &descriptions as &[&str],
        &links        as &[&str],
        &timestamps,
        &images       as &[&str],
        &category   as &[&str],
        &specific_category   as &[&str],
    )
    .execute(pool)
    .await;

    match result {
        Ok(r) => {
            let inserted = r.rows_affected();
            // OPTIMIZACIÓN 8: Contador atómico de noticias nuevas por ciclo.
            // AtomicU64 con Relaxed es suficiente: solo necesitamos que el
            // valor sea consistente al final del ciclo, sin ordering de memoria.
            total.fetch_add(inserted, Ordering::Relaxed);
            println!("  ✅ Batch {n}: {inserted} nuevas");
        }
        Err(e) => eprintln!("  ❌ Error en batch: {e}"),
    }

    batch.clear();
}

// ─── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL debe estar configurada");

    // OPTIMIZACIÓN 9: Pool de conexiones configurado explícitamente.
    // - min_connections = 2: evita cold-start en cada ciclo.
    // - max_connections = 10: 1 por tarea de categoría más margen.
    // - acquire_timeout = 5 s: falla rápido si el pool está saturado en lugar
    //   de colgar indefinidamente.
    // - idle_timeout = 10 min: libera conexiones ociosas entre ciclos largos.
    let pool = PgPoolOptions::new()
        .min_connections(2)
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(600))
        .connect(&database_url)
        .await?;

    println!("🚀 Conexión a DB exitosa. Iniciando sincronizador...");

    // OPTIMIZACIÓN 10: Client HTTP con keep-alive agresivo.
    // tcp_keepalive asegura que conexiones persistentes al CDN de Cooperativa
    // no sean cerradas silenciosamente por firewalls de red.
    let client = Arc::new(
        reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .pool_max_idle_per_host(4)
            .tcp_keepalive(Duration::from_secs(30))
            .build()?
    );

    // El semáforo vive fuera del loop: se reutiliza entre ciclos sin realocar.
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_FETCHES));

    loop {
        let cycle_start = std::time::Instant::now();
        println!("\n--- Ciclo: {} ---", Utc::now());

        let total_inserted = Arc::new(AtomicU64::new(0));
        let (tx, rx)       = mpsc::channel::<NewsItem>(CHANNEL_BUFFER);
        let writer         = tokio::spawn(db_writer(
            pool.clone(),
            rx,
            Arc::clone(&total_inserted),
        ));

        let mut tasks = JoinSet::new();
        for (specific_category, urls) in news_feeds().iter() {
            tasks.spawn(sync_category(
                Arc::clone(&client),
                Arc::clone(&semaphore),
                specific_category,
                urls,
                tx.clone(),
            ));
        }

        // Esperar a que todas las tareas de fetch terminen antes de cerrar tx.
        while tasks.join_next().await.is_some() {}
        drop(tx); // Señal de EOF para db_writer.

        if let Err(e) = writer.await {
            eprintln!("❌ Error en db_writer: {e}");
        }

        let elapsed  = cycle_start.elapsed();
        let inserted = total_inserted.load(Ordering::Relaxed);
        println!(
            "——— Ciclo finalizado en {:.1}s — {inserted} noticias nuevas. Esperando 5 min... ———",
            elapsed.as_secs_f64()
        );

        sleep(Duration::from_secs(300)).await;
    }
}