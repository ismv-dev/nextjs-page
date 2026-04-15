use anyhow::Result;
use chrono::Utc;
use futures::stream::{FuturesUnordered, StreamExt};
use rss::Channel;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinSet;
use tokio::time::sleep;

// ─── Constantes ───────────────────────────────────────────────────────────────

const BATCH_SIZE: usize = 100;
const BATCH_TIMEOUT: Duration = Duration::from_secs(5);
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

// OPTIMIZACIÓN 1: Semáforo de concurrencia.
// Sin límite, 20 fetches simultáneos saturan el pool de conexiones del OS y
// presionan al servidor remoto. 8 permisos es un equilibrio: satura bien el
// ancho de banda sin provocar errores 429/503.
const MAX_CONCURRENT_FETCHES: usize = 8;

// OPTIMIZACIÓN 2: Buffer del channel calibrado.
// Antes era 500 (arbitrario). Con BATCH_SIZE=100 y hasta ~20 feeds activos,
// 2×BATCH_SIZE da backpressure útil sin desperdiciar memoria de heap.
const CHANNEL_BUFFER: usize = BATCH_SIZE * 2;

// ─── Mapa de feeds (OnceLock en lugar de lazy_static) ─────────────────────────
//
// OPTIMIZACIÓN 3: OnceLock<HashMap> es estable desde Rust 1.70 y no requiere
// la dependencia `lazy_static`. Elimina una crate del árbol de dependencias y
// produce código más idiomático.

type FeedMap = std::collections::HashMap<&'static str, &'static [&'static str]>;

fn news_feeds() -> &'static FeedMap {
    static MAP: OnceLock<FeedMap> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = FeedMap::new();
        m.insert("Corporativo",   &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_16___1.xml"] as &[_]);
        m.insert("Cultura", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5___1.xml"]);
        m.insert("Deportes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1___1.xml"]);
        m.insert("Economía", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6___1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_70_619_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_80_634_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_73_622_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_624_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_77_626_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_78_629_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_152_1227_1.xml"]);
        m.insert("Entretención", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4___1.xml"]);
        m.insert("Mundo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2___1.xml"]);
        m.insert("País", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3___1.xml"]);
        m.insert("Sociedad", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7___1.xml"]);
        m.insert("Tecnología", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8___1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_71_665_1.xml"]);
        m.insert("Efecto China", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_16_312__1.xml"]);
        m.insert("Encuestas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1470_1.xml"]);
        m.insert("Especiales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_16_267__1.xml"]);
        m.insert("Noticias", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_16_256__1.xml"]);
        m.insert("Programas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_16_257__1.xml"]);
        m.insert("Arte", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_2__1.xml"]);
        m.insert("Cultura popular", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_6__1.xml"]);
        m.insert("Literatura", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_9__1.xml"]);
        m.insert("Museos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_10__1.xml"]);
        m.insert("Patrimonio", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_13__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_183_1421_1.xml"]);
        m.insert("Patrimonio cultural", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_12__1.xml"]);
        m.insert("Premios", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_14__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_4_691_1.xml"]);
        m.insert("Teatro", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_15__1.xml"]);
        m.insert("Teatro Municipal", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_16__1.xml"]);
        m.insert("Al Aire Libre", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_66__1.xml"]);
        m.insert("Artes marciales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_43__1.xml"]);
        m.insert("Atletismo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_23__1.xml"]);
        m.insert("Automovilismo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_24__1.xml"]);
        m.insert("Baloncesto", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_25__1.xml"]);
        m.insert("Balonmano", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_26__1.xml"]);
        m.insert("Boxeo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_27__1.xml"]);
        m.insert("Ciclismo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_28__1.xml"]);
        m.insert("Copa América", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_31__1.xml"]);
        m.insert("Copa Davis", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_534_1.xml"]);
        m.insert("Copa Libertadores", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_223_1.xml"]);
        m.insert("Eurocopa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_35__1.xml"]);
        m.insert("Fútbol", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30__1.xml"]);
        m.insert("Fuera de Juego", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_307__1.xml"]);
        m.insert("Gimnasia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_36__1.xml"]);
        m.insert("Gobierno", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_37__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_175__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_115_920_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1034_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_77_1106_1.xml"]);
        m.insert("Golf", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_38__1.xml"]);
        m.insert("Hockey césped", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_39__1.xml"]);
        m.insert("Hockey patín", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_40__1.xml"]);
        m.insert("Industria", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_246__1.xml"]);
        m.insert("Juegos Mundiales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_65__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_1905_1.xml"]);
        m.insert("Juegos Olímpicos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_22__1.xml"]);
        m.insert("Juegos Olímpicos de Invierno", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_42__1.xml"]);
        m.insert("Mejores deportistas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_47__1.xml"]);
        m.insert("Motociclismo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_48__1.xml"]);
        m.insert("Mundiales de Fútbol", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_20__1.xml"]);
        m.insert("Natación", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_50__1.xml"]);
        m.insert("Olimpismo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_45__1.xml"]);
        m.insert("Paralímpicos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_51__1.xml"]);
        m.insert("Polideportivo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19__1.xml"]);
        m.insert("Rally", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_49__1.xml"]);
        m.insert("Rodeo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_56__1.xml"]);
        m.insert("Rugby", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_57__1.xml"]);
        m.insert("Tenis", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58__1.xml"]);
        m.insert("Tenis de mesa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_59__1.xml"]);
        m.insert("Triatlón", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_62__1.xml"]);
        m.insert("Velerismo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_63__1.xml"]);
        m.insert("Voleibol", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_64__1.xml"]);
        m.insert("Banco Central", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_82__1.xml"]);
        m.insert("Bolsas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_84__1.xml"]);
        m.insert("Competitividad", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_85__1.xml"]);
        m.insert("Consumidores", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_68__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_88__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_223__1.xml"]);
        m.insert("Crecimiento", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_89__1.xml"]);
        m.insert("Crisis financiera", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_74__1.xml"]);
        m.insert("Divisas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_75__1.xml"]);
        m.insert("Empresas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_71__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_91_648_1.xml"]);
        m.insert("Foros internacionales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_72__1.xml"]);
        m.insert("Impuestos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_93__1.xml"]);
        m.insert("Materias primas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_69__1.xml"]);
        m.insert("Presupuesto", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_94__1.xml"]);
        m.insert("Pymes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_95__1.xml"]);
        m.insert("Retail", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_86__1.xml"]);
        m.insert("Sectores productivos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_91__1.xml"]);
        m.insert("Servicios financieros", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_83__1.xml"]);
        m.insert("Sistema previsional", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_212__1.xml"]);
        m.insert("Sueldo mínimo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_97__1.xml"]);
        m.insert("Carnavales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_99__1.xml"]);
        m.insert("Carnavales Culturales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_17__1.xml"]);
        m.insert("Cómic", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_100__1.xml"]);
        m.insert("Cine", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_4__1.xml"]);
        m.insert("Espectáculos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_101__1.xml"]);
        m.insert("Festival de Viña", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_103__1.xml"]);
        m.insert("Festivales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_275__1.xml"]);
        m.insert("Humor", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_105__1.xml"]);
        m.insert("Listas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_106__1.xml"]);
        m.insert("Música", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11__1.xml"]);
        m.insert("Panoramas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_111__1.xml"]);
        m.insert("Personajes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_107__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_864_1.xml"]);
        m.insert("Predicciones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_108__1.xml"]);
        m.insert("Radio", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_109__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_234_1770_1.xml"]);
        m.insert("Streaming", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_303__1.xml"]);
        m.insert("Sucesos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_110__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_219__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_115_918_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_70_927_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_80_976_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_130_982_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1041_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_131_1175_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_149_1786_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1429_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1657_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1663_1.xml"]);
        m.insert("Televisión", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_234_1773_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_246_1855_1.xml"]);
        m.insert("Tendencias", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_104__1.xml"]);
        m.insert("Accidentes aéreos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_112__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1426_1.xml"]);
        m.insert("Afganistán", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_113__1.xml"]);
        m.insert("Africa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_114__1.xml"]);
        m.insert("Alemania", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_115__1.xml"]);
        m.insert("América Latina", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_92__1.xml"]);
        m.insert("Argentina", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_70__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1587_1.xml"]);
        m.insert("Asia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_126__1.xml"]);
        m.insert("Bolivia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_129__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1460_1.xml"]);
        m.insert("Brasil", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_80__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1590_1.xml"]);
        m.insert("Canadá", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_130__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1591_1.xml"]);
        m.insert("Caribe", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_154__1.xml"]);
        m.insert("China", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_73__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1593_1.xml"]);
        m.insert("Colombia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_132__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1467_1.xml"]);
        m.insert("Cuba", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_134__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1597_1.xml"]);
        m.insert("Desastres naturales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_127__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_179__1.xml"]);
        m.insert("Ecuador", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_135__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1601_1.xml"]);
        m.insert("EE.UU.", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1602_1.xml"]);
        m.insert("España", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_140__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1588_1.xml"]);
        m.insert("Europa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79__1.xml"]);
        m.insert("Ex Yugoslavia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_259__1.xml"]);
        m.insert("Francia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_142__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1604_1.xml"]);
        m.insert("Haití", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_143__1.xml"]);
        m.insert("India", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_124__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1606_1.xml"]);
        m.insert("India-Pakistán", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_261__1.xml"]);
        m.insert("Irak", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_141__1.xml"]);
        m.insert("Irán", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_125__1.xml"]);
        m.insert("Italia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_144__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1607_1.xml"]);
        m.insert("Japón", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_77__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1608_1.xml"]);
        m.insert("México", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_147__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1610_1.xml"]);
        m.insert("Medio Oriente", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_118__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1603_1.xml"]);
        m.insert("Organismos Internacionales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_123__1.xml"]);
        m.insert("Pacífico Sur", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_116__1.xml"]);
        m.insert("Paraguay", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_148__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1614_1.xml"]);
        m.insert("Península de Corea", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_122__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1595_1.xml"]);
        m.insert("Perú", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_78__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1585_1.xml"]);
        m.insert("Reino Unido", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_131__1.xml"]);
        m.insert("Rusia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_149__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1616_1.xml"]);
        m.insert("Sudáfrica", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_150__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1619_1.xml"]);
        m.insert("Sudán", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_114_1203_1.xml"]);
        m.insert("Terrorismo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_258__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_144_1104_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_131_1182_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_149_1196_1.xml"]);
        m.insert("Uruguay", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_151__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1624_1.xml"]);
        m.insert("Vaticano", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_145__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1625_1.xml"]);
        m.insert("Venezuela", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_152__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1474_1.xml"]);
        m.insert("Augusto Pinochet", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_183__1.xml"]);
        m.insert("Ciudades", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_164__1.xml"]);
        m.insert("DD.HH.", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_168__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_70_931_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_73_987_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_134_1002_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1027_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_152_1223_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_183_1422_1.xml"]);
        m.insert("Educación", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_171__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_227__1.xml"]);
        m.insert("Empresas del Estado", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_167__1.xml"]);
        m.insert("Energía", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_162__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_249__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_70_929_1.xml"]);
        m.insert("Festivos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_7__1.xml"]);
        m.insert("FF.AA. y de Orden", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_173__1.xml"]);
        m.insert("Iglesia Católica", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_178__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_206_1879_1.xml"]);
        m.insert("Infancia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_180__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_233__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1799_1.xml"]);
        m.insert("Judicial", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_159__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_168_1260_1.xml"]);
        m.insert("Juegos de azar", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_184__1.xml"]);
        m.insert("Juventud", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_185__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_181_2462_1.xml"]);
        m.insert("Manifestaciones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_155__1.xml"]);
        m.insert("Medioambiente", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_166__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_187__1.xml"]);
        m.insert("Michelle Bachelet", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_190__1.xml"]);
        m.insert("Mujer", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_188__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_209__1.xml"]);
        m.insert("Obras Públicas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_189__1.xml"]);
        m.insert("Organismos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_161__1.xml"]);
        m.insert("Organismos del Estado", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_5__1.xml"]);
        m.insert("Personalidades", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_304__1.xml"]);
        m.insert("Población", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_181__1.xml"]);
        m.insert("Política", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_113_912_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_70_935_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_129_963_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_73_985_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_135_1012_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1023_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_142_1069_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_143_1078_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_152_1226_1.xml"]);
        m.insert("Políticas sociales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_163__1.xml"]);
        m.insert("Policial", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_110_2393_1.xml"]);
        m.insert("Presidente Boric", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_313__1.xml"]);
        m.insert("Pueblos originarios", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_165__1.xml"]);
        m.insert("Rapa Nui", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_182__1.xml"]);
        m.insert("Región de Antofagasta", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_191__1.xml"]);
        m.insert("Región de Arica", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_192__1.xml"]);
        m.insert("Región de Atacama", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_193__1.xml"]);
        m.insert("Región de Aysén", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_194__1.xml"]);
        m.insert("Región de Ñuble", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_305__1.xml"]);
        m.insert("Región de Coquimbo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_195__1.xml"]);
        m.insert("Región de La Araucanía", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_196__1.xml"]);
        m.insert("Región de Los Lagos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_197__1.xml"]);
        m.insert("Región de Los Ríos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_198__1.xml"]);
        m.insert("Región de Magallanes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_199__1.xml"]);
        m.insert("Región de OHiggins", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_200__1.xml"]);
        m.insert("Región de Tarapacá", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_201__1.xml"]);
        m.insert("Región de Valparaíso", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_202__1.xml"]);
        m.insert("Región del Biobío", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_203__1.xml"]);
        m.insert("Región del Maule", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_204__1.xml"]);
        m.insert("Regiones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_310__1.xml"]);
        m.insert("Relaciones Exteriores", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_70_941_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_129_966_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_73_988_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_134_1001_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1022_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_125_1092_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_144_1099_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_131_1174_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_149_1193_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_152_1222_1.xml"]);
        m.insert("Religiones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_206__1.xml"]);
        m.insert("Ricardo Lagos Escobar", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_207__1.xml"]);
        m.insert("Salud", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120__1.xml"]);
        m.insert("Salvador Allende", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_208__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1672_1.xml"]);
        m.insert("Sebastián Piñera", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_217__1.xml"]);
        m.insert("Seguridad ciudadana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_174__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_80_972_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_132_993_1.xml"]);
        m.insert("Servicios básicos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_98__1.xml"]);
        m.insert("Servicios públicos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_205__1.xml"]);
        m.insert("Sismos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_211__1.xml"]);
        m.insert("Tiempo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_213__1.xml"]);
        m.insert("Trabajo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_87__1.xml"]);
        m.insert("Transportes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_214__1.xml"]);
        m.insert("Turismo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_215__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_104_911_1.xml"]);
        m.insert("Vivienda", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_216__1.xml"]);
        m.insert("Adulto mayor", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_218__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_181_2461_1.xml"]);
        m.insert("Astronomía", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_250__1.xml"]);
        m.insert("Celebraciones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_220__1.xml"]);
        m.insert("Ciencia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_221__1.xml"]);
        m.insert("Ciencias sociales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_8__1.xml"]);
        m.insert("Derechos animales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_224__1.xml"]);
        m.insert("Derechos humanos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_225__1.xml"]);
        m.insert("Desarrollo humano", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_226__1.xml"]);
        m.insert("Familia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_244__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_190_1506_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_313_2471_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_217_2450_1.xml"]);
        m.insert("Fauna", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_230__1.xml"]);
        m.insert("Historia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_139__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_258_1.xml"]);
        m.insert("Medios", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_234__1.xml"]);
        m.insert("Minorías sexuales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_177__1.xml"]);
        m.insert("Pedofilia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_136__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1420_1.xml"]);
        m.insert("Pornografía", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_238__1.xml"]);
        m.insert("Premio Príncipe de Asturias", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_239__1.xml"]);
        m.insert("Premios Nobel", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_240__1.xml"]);
        m.insert("Racismo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_241__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_334_1.xml"]);
        m.insert("Religión", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_146__1.xml"]);
        m.insert("Sexualidad", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_245__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1806_1.xml"]);
        m.insert("Estudios", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_251__1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_171_1746_1.xml"]);
        m.insert("Internet", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_248__1.xml"]);
        m.insert("Inventos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_253__1.xml"]);
        m.insert("Redes Sociales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_252__1.xml"]);
        m.insert("Efecto China noticias", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_16_312_2465_1.xml"]);
        m.insert("Efecto China opinión", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_16_312_2466_1.xml"]);
        m.insert("Contenido de servicio", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_16_267_2475_1.xml"]);
        m.insert("Contenido patrocinado", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_16_256_1895_1.xml"]);
        m.insert("Fact checking", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_16_256_2459_1.xml"]);
        m.insert("Arquitectura", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_2_1_1.xml"]);
        m.insert("Arte contemporáneo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_2_2_1.xml"]);
        m.insert("Danza", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_2_8_1.xml"]);
        m.insert("Exhibiciones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_2_3_1.xml"]);
        m.insert("Fotografía", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_2_11_1.xml"]);
        m.insert("Pintura", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_2_66_1.xml"]);
        m.insert("Feria del Libro", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_9_17_1.xml"]);
        m.insert("Gabriela Mistral", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_9_20_1.xml"]);
        m.insert("Isabel Allende", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_9_25_1.xml"]);
        m.insert("Mario Vargas Llosa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_9_33_1.xml"]);
        m.insert("Miguel de Cervantes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_9_35_1.xml"]);
        m.insert("Pablo Neruda", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_9_37_1.xml"]);
        m.insert("Premio Nobel", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_9_42_1.xml"]);
        m.insert("Chile", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_13_65_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_23_84_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_302_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_89_643_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_139_1760_1.xml"]);
        m.insert("Premios Nacionales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_14_69_1.xml"]);
        m.insert("Santiago a Mil", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_5_15_73_1.xml"]);
        m.insert("Galería de Goles", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_66_613_1.xml"]);
        m.insert("Lo Peor de lo Nuestro", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_66_612_1.xml"]);
        m.insert("Opinión", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_66_1975_1.xml"]);
        m.insert("Judo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_43_437_1.xml"]);
        m.insert("Karate", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_43_1863_1.xml"]);
        m.insert("Tae kwon do", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_43_522_1.xml"]);
        m.insert("UFC", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_43_2067_1.xml"]);
        m.insert("Chilenos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_23_78_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_24_94_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_25_121_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_28_134_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_38_423_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_22_471_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_524_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_59_588_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_62_598_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_63_1965_1.xml"]);
        m.insert("Corridas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_23_2320_1.xml"]);
        m.insert("Dopaje", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_23_81_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_237_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_539_1.xml"]);
        m.insert("Internacional", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_23_77_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_28_136_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_194_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_38_422_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_57_515_1.xml"]);
        m.insert("Maratón", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_23_83_1.xml"]);
        m.insert("Mundial", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_23_85_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_25_109_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_57_517_1.xml"]);
        m.insert("Natalia Ducó", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_23_86_1.xml"]);
        m.insert("Records", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_23_88_1.xml"]);
        m.insert("Eliseo Salazar", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_24_95_1.xml"]);
        m.insert("Fórmula 1", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_24_96_1.xml"]);
        m.insert("Fórmula 3", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_24_98_1.xml"]);
        m.insert("FIA", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_24_97_1.xml"]);
        m.insert("IndyCar", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_24_100_1.xml"]);
        m.insert("Mundial de Rally", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_24_101_1.xml"]);
        m.insert("Nascar", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_24_99_1.xml"]);
        m.insert("Pilotos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_24_93_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_48_482_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_49_477_1.xml"]);
        m.insert("Jugadores", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_25_104_1.xml"]);
        m.insert("Liga nacional", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_25_119_1.xml"]);
        m.insert("Liga sudamericana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_25_106_1.xml"]);
        m.insert("Michael Jordan", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_25_108_1.xml"]);
        m.insert("NBA", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_25_105_1.xml"]);
        m.insert("Selección", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_25_113_1.xml"]);
        m.insert("Sudamericano femenino", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_25_115_1.xml"]);
        m.insert("Torneo de las Américas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_25_117_1.xml"]);
        m.insert("Boxeo chileno", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_27_128_1.xml"]);
        m.insert("Boxeo femenino", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_27_131_1.xml"]);
        m.insert("Boxeo internacional", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_27_129_1.xml"]);
        m.insert("Mike Tyson", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_27_130_1.xml"]);
        m.insert("Copa del Mundo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_28_135_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_36_419_1.xml"]);
        m.insert("Giro de Italia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_28_137_1.xml"]);
        m.insert("Mountainbike", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_28_139_1.xml"]);
        m.insert("Mundial de pista", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_28_140_1.xml"]);
        m.insert("Panamericano ciclismo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_28_141_1.xml"]);
        m.insert("Panamericano junior", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_28_142_1.xml"]);
        m.insert("Tour de Francia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_28_143_1.xml"]);
        m.insert("Vuelta Ciclista Femenina", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_28_146_1.xml"]);
        m.insert("Chile 2015", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_31_2075_1.xml"]);
        m.insert("EE.UU. 2016", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_31_2077_1.xml"]);
        m.insert("Cobreloa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_210_1.xml"]);
        m.insert("Cobresal", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_211_1.xml"]);
        m.insert("Colo Colo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_213_1.xml"]);
        m.insert("Deportes Iquique", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_266_1.xml"]);
        m.insert("Everton", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_243_1.xml"]);
        m.insert("Huachipato", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_259_1.xml"]);
        m.insert("OHiggins", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_319_1.xml"]);
        m.insert("Palestino", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_324_1.xml"]);
        m.insert("Alexis Sánchez", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_195_1.xml"]);
        m.insert("ANFP", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_196_1.xml"]);
        m.insert("Antofagasta", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_197_1.xml"]);
        m.insert("Arbitros chilenos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_198_1.xml"]);
        m.insert("Arturo Salah", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_415_1.xml"]);
        m.insert("Arturo Vidal", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_200_1.xml"]);
        m.insert("Audax Italiano", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_201_1.xml"]);
        m.insert("Ñublense", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_318_1.xml"]);
        m.insert("Balón de Oro", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_202_1.xml"]);
        m.insert("Barnechea", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_413_1.xml"]);
        m.insert("Barras bravas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_293_1.xml"]);
        m.insert("Boca Juniors", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_203_1.xml"]);
        m.insert("Campeonato Nacional", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_2394_1.xml"]);
        m.insert("Carlos Caszely", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_392_1.xml"]);
        m.insert("Carlos Villanueva", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_204_1.xml"]);
        m.insert("CDF", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_1989_1.xml"]);
        m.insert("Chilenos en el exterior", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_206_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1056_1.xml"]);
        m.insert("Claudio Borghi", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_208_1.xml"]);
        m.insert("Claudio Bravo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_390_1.xml"]);
        m.insert("Colegio de Técnicos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_212_1.xml"]);
        m.insert("Conmebol", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_214_1.xml"]);
        m.insert("Copa Asia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_216_1.xml"]);
        m.insert("Copa Chile", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_217_1.xml"]);
        m.insert("Copa de Oro Concacaf", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_219_1.xml"]);
        m.insert("Copa del Rey", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_221_1.xml"]);
        m.insert("Copa Italia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_222_1.xml"]);
        m.insert("Copa Naciones de Africa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_224_1.xml"]);
        m.insert("Copa Sudamericana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_225_1.xml"]);
        m.insert("Copiapó", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_402_1.xml"]);
        m.insert("Coquimbo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_228_1.xml"]);
        m.insert("Corrupción", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_229_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_80_970_1.xml"]);
        m.insert("Cristiano Ronaldo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_230_1.xml"]);
        m.insert("Curicó Unido", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_232_1.xml"]);
        m.insert("David Beckham", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_234_1.xml"]);
        m.insert("David Pizarro", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_235_1.xml"]);
        m.insert("Deportes Concepción", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_236_1.xml"]);
        m.insert("Dirigentes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_271_1.xml"]);
        m.insert("Eduardo Vargas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_408_1.xml"]);
        m.insert("El mejor de América", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_238_1.xml"]);
        m.insert("Elías Figueroa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_239_1.xml"]);
        m.insert("Estadios", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_241_1.xml"]);
        m.insert("Esteban Paredes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_394_1.xml"]);
        m.insert("Europa League", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_226_1.xml"]);
        m.insert("Ex futbolistas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_323_1.xml"]);
        m.insert("Expo F11", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_1967_1.xml"]);
        m.insert("Fútbol Joven", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_389_1.xml"]);
        m.insert("Fútbol sala", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_251_1.xml"]);
        m.insert("FC Barcelona", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_244_1.xml"]);
        m.insert("FIFA", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_246_1.xml"]);
        m.insert("Francisco Valdés", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_393_1.xml"]);
        m.insert("Futbol 7", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_247_1.xml"]);
        m.insert("Futbol de exhibición", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_248_1.xml"]);
        m.insert("Futbol femenino", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_249_1.xml"]);
        m.insert("Futbol playa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_250_1.xml"]);
        m.insert("Futbolistas chilenos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_199_1.xml"]);
        m.insert("Gary Medel", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_254_1.xml"]);
        m.insert("Héctor Mancilla", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_256_1.xml"]);
        m.insert("Humberto Suazo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_260_1.xml"]);
        m.insert("Iberia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_1983_1.xml"]);
        m.insert("INAF", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_262_1.xml"]);
        m.insert("Inter de Milán", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_263_1.xml"]);
        m.insert("Intercontinental", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_264_1.xml"]);
        m.insert("Interliga México", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_265_1.xml"]);
        m.insert("Iván Zamorano", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_267_1.xml"]);
        m.insert("Johnny Herrera", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_411_1.xml"]);
        m.insert("Jorge Acuña", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_268_1.xml"]);
        m.insert("Jorge Sampaoli", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_401_1.xml"]);
        m.insert("Jorge Valdivia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_269_1.xml"]);
        m.insert("José Rojas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_409_1.xml"]);
        m.insert("La Serena", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_273_1.xml"]);
        m.insert("Ley SAD", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_274_1.xml"]);
        m.insert("Liga alemana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_275_1.xml"]);
        m.insert("Liga argentina", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_276_1.xml"]);
        m.insert("Liga brasileña", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_231_1.xml"]);
        m.insert("Liga colombiana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_277_1.xml"]);
        m.insert("Liga de Campeones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_278_1.xml"]);
        m.insert("Liga de Estados Unidos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_279_1.xml"]);
        m.insert("Liga escocesa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_280_1.xml"]);
        m.insert("Liga española", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_281_1.xml"]);
        m.insert("Liga eSport", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_2441_1.xml"]);
        m.insert("Liga francesa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_282_1.xml"]);
        m.insert("Liga holandesa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_283_1.xml"]);
        m.insert("Liga inglesa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_205_1.xml"]);
        m.insert("Liga israelí", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_284_1.xml"]);
        m.insert("Liga italiana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_240_1.xml"]);
        m.insert("Liga mexicana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_285_1.xml"]);
        m.insert("Liga peruana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_286_1.xml"]);
        m.insert("Liga portuguesa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_287_1.xml"]);
        m.insert("Liga turca", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_290_1.xml"]);
        m.insert("Liguilla de Promoción", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_291_1.xml"]);
        m.insert("Lionel Messi", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_292_1.xml"]);
        m.insert("Lota Schwager", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_294_1.xml"]);
        m.insert("Luis Jiménez", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_295_1.xml"]);
        m.insert("Magallanes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_400_1.xml"]);
        m.insert("Manchester United", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_297_1.xml"]);
        m.insert("Manuel Pellegrini", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_326_1.xml"]);
        m.insert("Maradona", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_298_1.xml"]);
        m.insert("Marcelo Barticciotto", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_299_1.xml"]);
        m.insert("Marcelo Bielsa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_387_1.xml"]);
        m.insert("Marcelo Díaz", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_410_1.xml"]);
        m.insert("Marcelo Espina", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_300_1.xml"]);
        m.insert("Marcelo Salas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_301_1.xml"]);
        m.insert("Marco Antonio Figueroa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_398_1.xml"]);
        m.insert("Mark González", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_303_1.xml"]);
        m.insert("Matías Fernández", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_305_1.xml"]);
        m.insert("Mathias Vidangossy", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_304_1.xml"]);
        m.insert("Mauricio Isla", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_306_1.xml"]);
        m.insert("Mauricio Pinilla", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_307_1.xml"]);
        m.insert("Melipilla", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_308_1.xml"]);
        m.insert("Movidas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_310_1.xml"]);
        m.insert("Mundial de Clubes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_311_1.xml"]);
        m.insert("Mundial Femenino", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_399_1.xml"]);
        m.insert("Mundial sub 17", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_315_1.xml"]);
        m.insert("Mundial sub 20", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_316_1.xml"]);
        m.insert("Naval", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_405_1.xml"]);
        m.insert("Nelson Acosta", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_317_1.xml"]);
        m.insert("Neymar", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_1979_1.xml"]);
        m.insert("Osorno", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_322_1.xml"]);
        m.insert("Pelé", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_325_1.xml"]);
        m.insert("Preolímpico", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_327_1.xml"]);
        m.insert("Primera B", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_328_1.xml"]);
        m.insert("Programación", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_329_1.xml"]);
        m.insert("Puerto Montt", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_331_1.xml"]);
        m.insert("Racing Club", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_333_1.xml"]);
        m.insert("Rangers", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_335_1.xml"]);
        m.insert("Rankings", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_261_1.xml"]);
        m.insert("Real Madrid", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_336_1.xml"]);
        m.insert("Recopa Sudamericana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_337_1.xml"]);
        m.insert("Record", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_338_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_560_1.xml"]);
        m.insert("Reglas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_339_1.xml"]);
        m.insert("Reinaldo Rueda", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_2398_1.xml"]);
        m.insert("River Plate", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_340_1.xml"]);
        m.insert("Roberto Rojas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_341_1.xml"]);
        m.insert("Ronaldinho", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_344_1.xml"]);
        m.insert("Ronaldo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_345_1.xml"]);
        m.insert("San Felipe", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_346_1.xml"]);
        m.insert("San Luis", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_347_1.xml"]);
        m.insert("San Marcos de Arica", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_412_1.xml"]);
        m.insert("Santiago Morning", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_348_1.xml"]);
        m.insert("Santiago Wanderers", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_349_1.xml"]);
        m.insert("Segunda División", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_414_1.xml"]);
        m.insert("Selección alemana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_351_1.xml"]);
        m.insert("Selección argentina", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_352_1.xml"]);
        m.insert("Selección boliviana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_353_1.xml"]);
        m.insert("Selección brasileña", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_354_1.xml"]);
        m.insert("Selección chilena", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_150_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_39_429_1.xml"]);
        m.insert("Selección colombiana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_355_1.xml"]);
        m.insert("Selección de Palestina", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_356_1.xml"]);
        m.insert("Selección ecuatoriana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_357_1.xml"]);
        m.insert("Selección española", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_358_1.xml"]);
        m.insert("Selección francesa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_359_1.xml"]);
        m.insert("Selección inglesa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_360_1.xml"]);
        m.insert("Selección italiana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_361_1.xml"]);
        m.insert("Selección mexicana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_362_1.xml"]);
        m.insert("Selección paraguaya", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_406_1.xml"]);
        m.insert("Selección peruana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_363_1.xml"]);
        m.insert("Selección portuguesa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_364_1.xml"]);
        m.insert("Selección uruguaya", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_365_1.xml"]);
        m.insert("Selección venezolana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_407_1.xml"]);
        m.insert("Sifup", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_369_1.xml"]);
        m.insert("Sub 17", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_1993_1.xml"]);
        m.insert("Sub 20", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_370_1.xml"]);
        m.insert("Sudamericano femenino sub 17", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_373_1.xml"]);
        m.insert("Sudamericano sub 17", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_375_1.xml"]);
        m.insert("Sudamericano sub 20", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_376_1.xml"]);
        m.insert("Superclásico", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_377_1.xml"]);
        m.insert("Supercopa de Europa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_378_1.xml"]);
        m.insert("Supercopa italiana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_379_1.xml"]);
        m.insert("Temuco", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_380_1.xml"]);
        m.insert("Tercera División", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_381_1.xml"]);
        m.insert("Torneo Transición", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_2389_1.xml"]);
        m.insert("UEFA", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_385_1.xml"]);
        m.insert("Unión Española", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_384_1.xml"]);
        m.insert("Unión La Calera", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_403_1.xml"]);
        m.insert("Universidad Católica", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_382_1.xml"]);
        m.insert("Universidad de Chile", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_332_1.xml"]);
        m.insert("Universidad de Concepción", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_383_1.xml"]);
        m.insert("Zidane", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_30_386_1.xml"]);
        m.insert("Tomas González", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_36_420_1.xml"]);
        m.insert("Estadio Seguro", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_37_1945_1.xml"]);
        m.insert("IND", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_37_421_1.xml"]);
        m.insert("Ministerio del Deporte", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_37_1911_1.xml"]);
        m.insert("Tiger Woods", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_38_425_1.xml"]);
        m.insert("Mundial juvenil", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_39_428_1.xml"]);
        m.insert("Selección adulta", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_40_433_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_64_607_1.xml"]);
        m.insert("Beijing 2008", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_22_125_1.xml"]);
        m.insert("Maria Sharapova", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_549_1.xml"]);
        m.insert("Novak Djokovic", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_553_1.xml"]);
        m.insert("Enduro", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_48_483_1.xml"]);
        m.insert("Motocross", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_48_484_1.xml"]);
        m.insert("Motos GP", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_48_481_1.xml"]);
        m.insert("Six days", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_48_479_1.xml"]);
        m.insert("Kristel Kobrich", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_50_487_1.xml"]);
        m.insert("Mundial de Natación", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_50_490_1.xml"]);
        m.insert("Récords", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_50_491_1.xml"]);
        m.insert("COCH", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_45_493_1.xml"]);
        m.insert("Comité olímpico internacional", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_45_494_1.xml"]);
        m.insert("Juegos de la Juventud", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_45_1987_1.xml"]);
        m.insert("Juegos Panamericanos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_45_472_1.xml"]);
        m.insert("Juegos Sudamericanos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_45_498_1.xml"]);
        m.insert("Ajedrez", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_74_1.xml"]);
        m.insert("Béisbol", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_126_1.xml"]);
        m.insert("Canotaje", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_132_1.xml"]);
        m.insert("Cricket", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_611_1.xml"]);
        m.insert("Deporte aventura", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_187_1.xml"]);
        m.insert("Deportes extremos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_188_1.xml"]);
        m.insert("Esgrima", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_190_1.xml"]);
        m.insert("Esquí", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_191_1.xml"]);
        m.insert("Esquí náutico", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_192_1.xml"]);
        m.insert("Fútbol americano", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_418_1.xml"]);
        m.insert("Hípica", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_426_1.xml"]);
        m.insert("Hockey hielo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_430_1.xml"]);
        m.insert("Levantamiento de pesas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_474_1.xml"]);
        m.insert("Lucha", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_475_1.xml"]);
        m.insert("Montañismo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_476_1.xml"]);
        m.insert("Patín carrera", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_500_1.xml"]);
        m.insert("Patinaje artístico", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_501_1.xml"]);
        m.insert("Remo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_511_1.xml"]);
        m.insert("Surf", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_1991_1.xml"]);
        m.insert("Team Chile", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_1969_1.xml"]);
        m.insert("Tiro al blanco", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_591_1.xml"]);
        m.insert("Tiro al vuelo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_19_592_1.xml"]);
        m.insert("Dakar", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_49_505_1.xml"]);
        m.insert("Motos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_49_478_1.xml"]);
        m.insert("Patagonia-Atacama", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_49_508_1.xml"]);
        m.insert("Rally Mobil", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_49_509_1.xml"]);
        m.insert("Túnez", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_114_916_1.xml"]);
        m.insert("Champion de Chile", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_56_513_1.xml"]);
        m.insert("Clasificatorios", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_56_512_1.xml"]);
        m.insert("Federaciones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_56_514_1.xml"]);
        m.insert("Clasificatorias", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_57_516_1.xml"]);
        m.insert("Ligas chilenas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_57_520_1.xml"]);
        m.insert("Los Cóndores", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_57_2396_1.xml"]);
        m.insert("Seven a Side", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_57_2397_1.xml"]);
        m.insert("Arabia Saudita", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_118_923_1.xml"]);
        m.insert("Australia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_116_955_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1589_1.xml"]);
        m.insert("Bélgica", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_959_1.xml"]);
        m.insert("Corea del Sur", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_122_998_1.xml"]);
        m.insert("Costa Rica", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_92_1000_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1596_1.xml"]);
        m.insert("Dinamarca", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1011_1.xml"]);
        m.insert("Egipto", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_118_1048_1.xml"]);
        m.insert("Islandia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1232_1.xml"]);
        m.insert("Marruecos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_114_1129_1.xml"]);
        m.insert("Nigeria", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_114_1149_1.xml"]);
        m.insert("Panamá", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_92_1157_1.xml"]);
        m.insert("Polonia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1169_1.xml"]);
        m.insert("Portugal", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1170_1.xml"]);
        m.insert("Suecia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1204_1.xml"]);
        m.insert("Suiza", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1205_1.xml"]);
        m.insert("Abierto de Australia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_527_1.xml"]);
        m.insert("ATP de Chile", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_582_1.xml"]);
        m.insert("Challengers", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_533_1.xml"]);
        m.insert("Christian Garín", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_1907_1.xml"]);
        m.insert("Copa Hopman", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_537_1.xml"]);
        m.insert("Equipo olímpico", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_540_1.xml"]);
        m.insert("Ex tenistas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_532_1.xml"]);
        m.insert("Federación de Tenis de Chile", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_542_1.xml"]);
        m.insert("Fernando González", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_541_1.xml"]);
        m.insert("Horacio de la Peña", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_544_1.xml"]);
        m.insert("Liga Internacional", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_2073_1.xml"]);
        m.insert("Marcelo Ríos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_548_1.xml"]);
        m.insert("Master de Londres", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_578_1.xml"]);
        m.insert("N. Massú", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_552_1.xml"]);
        m.insert("Nicolás Jarry", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_2129_1.xml"]);
        m.insert("P. Capdeville", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_555_1.xml"]);
        m.insert("R. Federer", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_558_1.xml"]);
        m.insert("Rafael Nadal", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_559_1.xml"]);
        m.insert("Roland Garros", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_561_1.xml"]);
        m.insert("S. Williams", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_563_1.xml"]);
        m.insert("Tenis menores", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_564_1.xml"]);
        m.insert("Tenistas chilenas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_565_1.xml"]);
        m.insert("Torneos ATP", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_528_1.xml"]);
        m.insert("US Open", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_566_1.xml"]);
        m.insert("V. Williams", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_567_1.xml"]);
        m.insert("Wimbledon", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_568_1.xml"]);
        m.insert("WTA", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_58_550_1.xml"]);
        m.insert("Pucon", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_62_600_1.xml"]);
        m.insert("Regatas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_63_605_1.xml"]);
        m.insert("Torneos nacionales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_64_606_1.xml"]);
        m.insert("Voleibol playa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_1_64_610_1.xml"]);
        m.insert("Tasa de interés", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_82_658_1.xml"]);
        m.insert("IPSA", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_84_637_1.xml"]);
        m.insert("Imagen país", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_85_661_1.xml"]);
        m.insert("Libre competencia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_85_660_1.xml"]);
        m.insert("Endeudamiento", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_68_645_1.xml"]);
        m.insert("Finanzas Personales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_68_617_1.xml"]);
        m.insert("Dólar", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_75_623_1.xml"]);
        m.insert("Aeronáuticas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_71_620_1.xml"]);
        m.insert("Automotoras", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_71_632_1.xml"]);
        m.insert("Grupos económicos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_71_664_1.xml"]);
        m.insert("Latam", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_71_936_1.xml"]);
        m.insert("APEC", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_72_1873_1.xml"]);
        m.insert("OMC", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_72_627_1.xml"]);
        m.insert("Operación Renta", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_93_653_1.xml"]);
        m.insert("Panama Papers", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_93_2377_1.xml"]);
        m.insert("Alimentos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_69_618_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1669_1.xml"]);
        m.insert("Cobre", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_69_641_1.xml"]);
        m.insert("Litio", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_69_1409_1.xml"]);
        m.insert("Petróleo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_69_630_1.xml"]);
        m.insert("Comercio", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_86_639_1.xml"]);
        m.insert("Supermercados", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_86_644_1.xml"]);
        m.insert("Agricultura", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_91_1242_1.xml"]);
        m.insert("Construcción", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_91_662_1.xml"]);
        m.insert("Minería", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_91_2386_1.xml"]);
        m.insert("Pesca", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_91_654_1.xml"]);
        m.insert("Bancos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_83_636_1.xml"]);
        m.insert("AFP", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_212_1684_1.xml"]);
        m.insert("Jubilación", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_6_212_1685_1.xml"]);
        m.insert("Batman", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_100_707_1.xml"]);
        m.insert("Animación", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_4_673_1.xml"]);
        m.insert("Censura", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_4_677_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_73_1730_1.xml"]);
        m.insert("Cine Chileno", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_4_669_1.xml"]);
        m.insert("Documentales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_4_4_1.xml"]);
        m.insert("Festivales de Cine", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_4_674_1.xml"]);
        m.insert("Hollywood", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_4_672_1.xml"]);
        m.insert("Marlon Brando", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_4_690_1.xml"]);
        m.insert("Premios Globos de Oro", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_4_695_1.xml"]);
        m.insert("Premios Goya", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_4_696_1.xml"]);
        m.insert("Premios Oscar", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_4_697_1.xml"]);
        m.insert("Quentin Tarantino", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_4_698_1.xml"]);
        m.insert("Star Wars", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_4_682_1.xml"]);
        m.insert("Concursos de belleza", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_101_709_1.xml"]);
        m.insert("Animadores", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_103_712_1.xml"]);
        m.insert("Jurados", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_103_715_1.xml"]);
        m.insert("Show", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_103_722_1.xml"]);
        m.insert("Lollapalooza", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_275_1937_1.xml"]);
        m.insert("Olmué", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_275_1939_1.xml"]);
        m.insert("The Metal Fest", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_275_1961_1.xml"]);
        m.insert("Che Copete", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_105_733_1.xml"]);
        m.insert("Dinamita Show", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_105_738_1.xml"]);
        m.insert("Stefan Kramer", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_105_736_1.xml"]);
        m.insert("Bob Marley", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_757_1.xml"]);
        m.insert("Charly García", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_766_1.xml"]);
        m.insert("Electrónica", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_774_1.xml"]);
        m.insert("Elvis Presley", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_776_1.xml"]);
        m.insert("Fito Páez", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_783_1.xml"]);
        m.insert("Folclor", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_53_1.xml"]);
        m.insert("Grammy", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_832_1.xml"]);
        m.insert("Grammy Latino", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_833_1.xml"]);
        m.insert("Gustavo Cerati", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_788_1.xml"]);
        m.insert("Hip Hop", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_777_1.xml"]);
        m.insert("Illapu", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_789_1.xml"]);
        m.insert("Inti Illimani", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_790_1.xml"]);
        m.insert("Jazz", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_792_1.xml"]);
        m.insert("Jorge González", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_796_1.xml"]);
        m.insert("José Luis Rodríguez", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_797_1.xml"]);
        m.insert("Juan Gabriel", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_798_1.xml"]);
        m.insert("Julio Iglesias", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_801_1.xml"]);
        m.insert("Los Bunkers", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_805_1.xml"]);
        m.insert("Los Jaivas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_806_1.xml"]);
        m.insert("Los Prisioneros", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_807_1.xml"]);
        m.insert("Los Tres", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_809_1.xml"]);
        m.insert("Lucybell", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_813_1.xml"]);
        m.insert("Luis Miguel", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_815_1.xml"]);
        m.insert("Madonna", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_816_1.xml"]);
        m.insert("Música brasileña", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_759_1.xml"]);
        m.insert("Música chilena", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_779_1.xml"]);
        m.insert("Música tropical", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_786_1.xml"]);
        m.insert("Metal", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_772_1.xml"]);
        m.insert("Michael Jackson", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_819_1.xml"]);
        m.insert("Miguel Bosé", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_820_1.xml"]);
        m.insert("MTV", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_822_1.xml"]);
        m.insert("Orquestas Juveniles", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_57_1.xml"]);
        m.insert("Paul McCartney", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_828_1.xml"]);
        m.insert("Pink Floyd", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_830_1.xml"]);
        m.insert("Pop en español", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_749_1.xml"]);
        m.insert("Pop en inglés", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_746_1.xml"]);
        m.insert("Raphael", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_835_1.xml"]);
        m.insert("Reggae", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_787_1.xml"]);
        m.insert("Reggaetón", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_837_1.xml"]);
        m.insert("Ricky Martin", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_839_1.xml"]);
        m.insert("Rock", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_754_1.xml"]);
        m.insert("Rock en español", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_752_1.xml"]);
        m.insert("Románticos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_747_1.xml"]);
        m.insert("Shakira", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_845_1.xml"]);
        m.insert("Shows en vivo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_859_1.xml"]);
        m.insert("Silvio Rodríguez", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_846_1.xml"]);
        m.insert("Sting", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_847_1.xml"]);
        m.insert("The Beatles", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_849_1.xml"]);
        m.insert("The Rolling Stones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_850_1.xml"]);
        m.insert("U2", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_852_1.xml"]);
        m.insert("Víctor Jara", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_62_1.xml"]);
        m.insert("Violeta Parra", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_11_63_1.xml"]);
        m.insert("Jennifer Lopez", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_107_871_1.xml"]);
        m.insert("Radio Cooperativa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_109_880_1.xml"]);
        m.insert("Sergio Campos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_109_882_1.xml"]);
        m.insert("31 minutos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_885_1.xml"]);
        m.insert("Canal 13", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_888_1.xml"]);
        m.insert("Chespirito", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_889_1.xml"]);
        m.insert("Chilevisión", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_890_1.xml"]);
        m.insert("Consejo Nacional de Televisión", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_891_1.xml"]);
        m.insert("Dibujos animados", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_710_1.xml"]);
        m.insert("Don Francisco", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_892_1.xml"]);
        m.insert("Felipe Camiroaga", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_910_1.xml"]);
        m.insert("MEGA", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_898_1.xml"]);
        m.insert("Premios Emmy", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_902_1.xml"]);
        m.insert("Reality show", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_904_1.xml"]);
        m.insert("Series", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_893_1.xml"]);
        m.insert("Telenovelas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_906_1.xml"]);
        m.insert("Teletón", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_907_1.xml"]);
        m.insert("Televisión Nacional", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_908_1.xml"]);
        m.insert("TV de pago", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_102_909_1.xml"]);
        m.insert("Gastronomía", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_104_731_1.xml"]);
        m.insert("Juguetes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_104_743_1.xml"]);
        m.insert("Moda", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_104_745_1.xml"]);
        m.insert("Parques de diversiones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_4_104_863_1.xml"]);
        m.insert("Conflicto", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_113_913_1.xml"]);
        m.insert("Argelia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_114_924_1.xml"]);
        m.insert("Costa de Marfil", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_114_999_1.xml"]);
        m.insert("Kenia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_114_1112_1.xml"]);
        m.insert("Libia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_114_951_1.xml"]);
        m.insert("R.D. Congo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_114_1172_1.xml"]);
        m.insert("Ruanda", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_114_1187_1.xml"]);
        m.insert("Somalía", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_114_1199_1.xml"]);
        m.insert("Elecciones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_70_933_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_129_965_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_80_971_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_130_980_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_132_994_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1029_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_142_1071_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_78_1164_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_152_1224_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1462_1.xml"]);
        m.insert("El Salvador", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_92_1049_1.xml"]);
        m.insert("Guatemala", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_92_1077_1.xml"]);
        m.insert("Honduras", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_92_1081_1.xml"]);
        m.insert("Mercosur", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_92_652_1.xml"]);
        m.insert("Nicaragua", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_92_1148_1.xml"]);
        m.insert("Puerto Rico", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_92_1171_1.xml"]);
        m.insert("República Dominicana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_92_1186_1.xml"]);
        m.insert("Alberto Fernández", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_70_2456_1.xml"]);
        m.insert("Atentado AMIA", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_70_925_1.xml"]);
        m.insert("Buenos Aires", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_70_926_1.xml"]);
        m.insert("Carlos Menem", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_70_937_1.xml"]);
        m.insert("Conflictos sociales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_70_940_1.xml"]);
        m.insert("Cristina Fernández", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_70_930_1.xml"]);
        m.insert("Mauricio Macri", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_70_2323_1.xml"]);
        m.insert("Perón", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_70_939_1.xml"]);
        m.insert("Bangladesh", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_126_958_1.xml"]);
        m.insert("Birmania", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_126_962_1.xml"]);
        m.insert("Camboya", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_126_979_1.xml"]);
        m.insert("Filipinas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_126_1067_1.xml"]);
        m.insert("Indonesia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_126_1085_1.xml"]);
        m.insert("Kazajistán", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_126_1111_1.xml"]);
        m.insert("Malasia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_126_1128_1.xml"]);
        m.insert("Nepal", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_126_1147_1.xml"]);
        m.insert("Pakistán", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_126_952_1.xml"]);
        m.insert("Sri Lanka", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_126_1200_1.xml"]);
        m.insert("Tailandia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_126_1206_1.xml"]);
        m.insert("Taiwán", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_126_1207_1.xml"]);
        m.insert("Vietnam", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_126_1228_1.xml"]);
        m.insert("Evo Morales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_129_967_1.xml"]);
        m.insert("Gas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_129_968_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1678_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_98_2065_1.xml"]);
        m.insert("Jair Bolsonaro", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_80_2454_1.xml"]);
        m.insert("Lula da Silva", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_80_973_1.xml"]);
        m.insert("Jamaica", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_154_1236_1.xml"]);
        m.insert("Natalidad", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_73_1731_1.xml"]);
        m.insert("Tíbet", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_73_991_1.xml"]);
        m.insert("Guerrilla", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_132_995_1.xml"]);
        m.insert("Narcotráfico", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_132_997_1.xml"]);
        m.insert("Oposición", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_134_1004_1.xml"]);
        m.insert("Erupciones volcánicas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_127_1233_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_179_2199_1.xml"]);
        m.insert("Huracanes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_127_1036_1.xml"]);
        m.insert("Incendios forestales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_127_956_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_179_1328_1.xml"]);
        m.insert("Terremotos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_127_990_1.xml"]);
        m.insert("Tifones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_127_953_1.xml"]);
        m.insert("Tsunamis", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_127_954_1.xml"]);
        m.insert("Alzamiento", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_135_1016_1.xml"]);
        m.insert("Protestas indígenas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_135_1014_1.xml"]);
        m.insert("Rafael Correa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_135_1015_1.xml"]);
        m.insert("11-S", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1017_1.xml"]);
        m.insert("Barack Obama", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1020_1.xml"]);
        m.insert("Bill Clinton", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1021_1.xml"]);
        m.insert("CIA", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1024_1.xml"]);
        m.insert("Donald Trump", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_2384_1.xml"]);
        m.insert("Ejecuciones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1028_1.xml"]);
        m.insert("Familia Kennedy", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1031_1.xml"]);
        m.insert("George W. Bush", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1033_1.xml"]);
        m.insert("Inmigración", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1038_1.xml"]);
        m.insert("Joe Biden", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_2469_1.xml"]);
        m.insert("Nueva York", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1040_1.xml"]);
        m.insert("Partido Demócrata", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1039_1.xml"]);
        m.insert("Partido Republicano", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1043_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_2476_1.xml"]);
        m.insert("Ronald Reagan", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1042_1.xml"]);
        m.insert("Seguridad", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1026_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_248_1841_1.xml"]);
        m.insert("Tiroteos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_76_1032_1.xml"]);
        m.insert("Corridas de toros", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_140_1751_1.xml"]);
        m.insert("Familia real española", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_140_1779_1.xml"]);
        m.insert("Inmigrantes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_140_1060_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_144_1098_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_181_1331_1.xml"]);
        m.insert("Austria", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_957_1.xml"]);
        m.insert("Balcanes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_969_1.xml"]);
        m.insert("Bielorrusia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_960_1.xml"]);
        m.insert("Bulgaria", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_977_1.xml"]);
        m.insert("Chipre", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_992_1.xml"]);
        m.insert("Eslovaquia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1051_1.xml"]);
        m.insert("Finlandia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1068_1.xml"]);
        m.insert("Georgia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1076_1.xml"]);
        m.insert("Grecia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_633_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1629_1.xml"]);
        m.insert("Hungría", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1082_1.xml"]);
        m.insert("Irlanda", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1093_1.xml"]);
        m.insert("Lituania", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1126_1.xml"]);
        m.insert("Noruega", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1150_1.xml"]);
        m.insert("Países Bajos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1080_1.xml"]);
        m.insert("Rumania", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1188_1.xml"]);
        m.insert("Turquía", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1209_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1622_1.xml"]);
        m.insert("Ucrania", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1213_1.xml"]);
        m.insert("Unión Europea", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_79_1214_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1623_1.xml"]);
        m.insert("Disturbios", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_142_1070_1.xml"]);
        m.insert("Nicolas Sarkozy", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_142_1074_1.xml"]);
        m.insert("Relaciones exteriores", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_142_1073_1.xml"]);
        m.insert("Sarkozy", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_142_1781_1.xml"]);
        m.insert("Ayuda exterior", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_143_1079_1.xml"]);
        m.insert("Programa nuclear", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_125_949_1.xml"]);
        m.insert("Huelgas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_144_1097_1.xml"]);
        m.insert("Mafia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_144_1101_1.xml"]);
        m.insert("Monarquía", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_77_1107_1.xml"]);
        m.insert("López Obrador", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_147_1144_1.xml"]);
        m.insert("Narcos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_147_1146_1.xml"]);
        m.insert("Conflicto Israel-Palestina", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_118_1981_1.xml"]);
        m.insert("Emiratos Arabes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_118_1050_1.xml"]);
        m.insert("Estado Islámico", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_118_1997_1.xml"]);
        m.insert("Israel", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_118_950_1.xml"]);
        m.insert("Jordania", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_118_1109_1.xml"]);
        m.insert("Líbano", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_118_1124_1.xml"]);
        m.insert("Palestina", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_118_1134_1.xml"]);
        m.insert("Siria", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_118_945_1.xml"]);
        m.insert("Yemen", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_118_1229_1.xml"]);
        m.insert("FAO", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_123_1234_1.xml"]);
        m.insert("OCDE", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_123_1152_1.xml"]);
        m.insert("OEA", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_123_1153_1.xml"]);
        m.insert("OIEA", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_123_947_1.xml"]);
        m.insert("ONU", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_123_1154_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1594_1.xml"]);
        m.insert("OTAN", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_123_1155_1.xml"]);
        m.insert("Tribunal Penal Internacional", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_123_1211_1.xml"]);
        m.insert("Antártica", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_116_921_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_199_2322_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1586_1.xml"]);
        m.insert("Nueva Zelanda", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_116_1151_1.xml"]);
        m.insert("Corea del Norte", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_122_946_1.xml"]);
        m.insert("Alberto Fujimori", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_78_1162_1.xml"]);
        m.insert("Alejandro Toledo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_78_1163_1.xml"]);
        m.insert("Ollanta Humala", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_78_1168_1.xml"]);
        m.insert("Vladimiro Montesinos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_78_1167_1.xml"]);
        m.insert("Caso Madeleine McCann", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_131_1127_1.xml"]);
        m.insert("Escocia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_131_1177_1.xml"]);
        m.insert("Familia real británica", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_131_1778_1.xml"]);
        m.insert("Malvinas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_131_2087_1.xml"]);
        m.insert("Margaret Thatcher", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_131_1181_1.xml"]);
        m.insert("Tony Blair", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_131_1183_1.xml"]);
        m.insert("Violencia callejera", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_131_1184_1.xml"]);
        m.insert("Dmitri Medvedev", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_149_1192_1.xml"]);
        m.insert("Partido Comunista", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_149_1195_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1472_1.xml"]);
        m.insert("Vladimir Putin", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_149_1197_1.xml"]);
        m.insert("Corrupción de menores", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_151_1215_1.xml"]);
        m.insert("Marihuana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_151_1927_1.xml"]);
        m.insert("Elección", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_145_1221_1.xml"]);
        m.insert("Papa Francisco", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_145_1893_1.xml"]);
        m.insert("Nicolás Maduro", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_2_152_1929_1.xml"]);
        m.insert("Santiago", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_164_1251_1.xml"]);
        m.insert("Valparaíso", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_164_1413_1.xml"]);
        m.insert("Combustibles", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_88_642_1.xml"]);
        m.insert("Denuncias", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_88_1254_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_87_1258_1.xml"]);
        m.insert("Inflación", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_88_650_1.xml"]);
        m.insert("Sernac", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_88_656_1.xml"]);
        m.insert("Falsos DDDD", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_168_1265_1.xml"]);
        m.insert("José Tohá", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_168_1397_1.xml"]);
        m.insert("Proyectos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_168_1264_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_171_1274_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_87_646_1.xml"]);
        m.insert("Inundaciones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_179_2205_1.xml"]);
        m.insert("Sequía", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_179_2457_1.xml"]);
        m.insert("Admisión universitaria", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_171_1277_1.xml"]);
        m.insert("Beneficios", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_171_1272_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_163_1502_1.xml"]);
        m.insert("Colegios", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_171_1271_1.xml"]);
        m.insert("Mediciones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_171_1275_1.xml"]);
        m.insert("Movimiento estudiantil", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_171_1276_1.xml"]);
        m.insert("Preescolar", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_171_1919_1.xml"]);
        m.insert("Profesores", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_171_1273_1.xml"]);
        m.insert("Universidades", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_171_1270_1.xml"]);
        m.insert("Violencia escolar", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_171_1279_1.xml"]);
        m.insert("Codelco", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_167_1408_1.xml"]);
        m.insert("Correos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_167_1257_1.xml"]);
        m.insert("Biocombustibles", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_162_1280_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_249_1824_1.xml"]);
        m.insert("Cambio de hora", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_162_1248_1.xml"]);
        m.insert("Gas natural", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_162_1283_1.xml"]);
        m.insert("Generación eléctrica", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_162_1284_1.xml"]);
        m.insert("Zona Centro", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_310_2444_1.xml"]);
        m.insert("Zona Norte", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_310_2443_1.xml"]);
        m.insert("Zona Sur", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_310_2445_1.xml"]);
        m.insert("Día de Todos los Santos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_7_1717_1.xml"]);
        m.insert("Fiestas Patrias", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_7_6_1.xml"]);
        m.insert("Fin de Año", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_7_1305_1.xml"]);
        m.insert("Semana Santa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_7_1324_1.xml"]);
        m.insert("Armada", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_173_1290_1.xml"]);
        m.insert("Carabineros", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_173_1293_1.xml"]);
        m.insert("Ejército", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_173_1295_1.xml"]);
        m.insert("Estado Mayor Conjunto", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_173_1303_1.xml"]);
        m.insert("Fidae", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_173_1296_1.xml"]);
        m.insert("Fuerza Aérea", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_173_1297_1.xml"]);
        m.insert("Servicio Militar", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_173_1301_1.xml"]);
        m.insert("Gabinete", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_175_1309_1.xml"]);
        m.insert("Transparencia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_175_1313_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1495_1.xml"]);
        m.insert("Hogar de Cristo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_178_1322_1.xml"]);
        m.insert("Visita papal 2018", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_178_2391_1.xml"]);
        m.insert("Políticas públicas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_180_1915_1.xml"]);
        m.insert("Protección", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_180_1330_1.xml"]);
        m.insert("Caso Spiniak", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_159_1353_1.xml"]);
        m.insert("Casos emblemáticos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_159_2085_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_2392_1.xml"]);
        m.insert("Cárceles", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_159_1307_1.xml"]);
        m.insert("Corte Suprema", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_159_1358_1.xml"]);
        m.insert("Denuncias de corrupción", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_159_1357_1.xml"]);
        m.insert("Drogas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_159_1362_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1439_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1269_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1794_1.xml"]);
        m.insert("Frentistas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_159_1367_1.xml"]);
        m.insert("Gendarmería", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_159_1306_1.xml"]);
        m.insert("Indultos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_159_1246_1.xml"]);
        m.insert("Irregularidades", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_159_1370_1.xml"]);
        m.insert("Menores", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_159_1411_1.xml"]);
        m.insert("Ministerio Público", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_159_1377_1.xml"]);
        m.insert("Robo de cobre", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_159_1385_1.xml"]);
        m.insert("SML", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_159_1389_1.xml"]);
        m.insert("Villa Baviera", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_159_1710_1.xml"]);
        m.insert("Polla", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_184_1399_1.xml"]);
        m.insert("11 de septiembre", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_155_1238_1.xml"]);
        m.insert("29 de marzo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_155_1239_1.xml"]);
        m.insert("Animales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_166_1403_1.xml"]);
        m.insert("Contaminación", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_166_1404_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_187_1766_1.xml"]);
        m.insert("Institucionalidad", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_166_1253_1.xml"]);
        m.insert("Viajes al exterior", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_190_1508_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_313_2472_1.xml"]);
        m.insert("Plan Chiloé", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_189_1415_1.xml"]);
        m.insert("Bomberos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_161_1247_1.xml"]);
        m.insert("CDE", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_5_1249_1.xml"]);
        m.insert("Contraloría", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_5_1256_1.xml"]);
        m.insert("PDI", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_5_1716_1.xml"]);
        m.insert("Censos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_181_1423_1.xml"]);
        m.insert("5 de octubre", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1240_1.xml"]);
        m.insert("Agenda legislativa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1308_1.xml"]);
        m.insert("Caso SQM", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_2213_1.xml"]);
        m.insert("Cámara Baja", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1461_1.xml"]);
        m.insert("Chile Vamos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1488_1.xml"]);
        m.insert("Concertación", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1465_1.xml"]);
        m.insert("Constitución", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1483_1.xml"]);
        m.insert("Cuenta Pública", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1468_1.xml"]);
        m.insert("Democracia Cristiana", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1466_1.xml"]);
        m.insert("Eduardo Frei", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1469_1.xml"]);
        m.insert("Evopoli", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_2467_1.xml"]);
        m.insert("Frente Amplio", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_2388_1.xml"]);
        m.insert("Marco Enríquez-Ominami", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1499_1.xml"]);
        m.insert("Municipales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1478_1.xml"]);
        m.insert("Nueva Mayoría", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1925_1.xml"]);
        m.insert("Parlamentarias", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1480_1.xml"]);
        m.insert("Partido Radical", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1487_1.xml"]);
        m.insert("Partido Socialista", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1481_1.xml"]);
        m.insert("Patricio Aylwin", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1482_1.xml"]);
        m.insert("PPD", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1484_1.xml"]);
        m.insert("Presidenciales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1485_1.xml"]);
        m.insert("Renovación Nacional", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1489_1.xml"]);
        m.insert("Senado", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1492_1.xml"]);
        m.insert("UDI", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_156_1497_1.xml"]);
        m.insert("Adulto Mayor", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_163_1875_1.xml"]);
        m.insert("Pobreza", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_163_1424_1.xml"]);
        m.insert("Servicio País", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_163_1503_1.xml"]);
        m.insert("Accidentes de tránsito", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1427_1.xml"]);
        m.insert("Apedreos autopistas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1428_1.xml"]);
        m.insert("Ataques de perros", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1244_1.xml"]);
        m.insert("Atentados", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1243_1.xml"]);
        m.insert("Delitos sexuales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1438_1.xml"]);
        m.insert("Estafas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1436_1.xml"]);
        m.insert("Estafas telefónicas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1454_1.xml"]);
        m.insert("Femicidio", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1412_1.xml"]);
        m.insert("Homicidios", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_2474_1.xml"]);
        m.insert("Incendios", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1441_1.xml"]);
        m.insert("Pandillas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1419_1.xml"]);
        m.insert("Robo bancos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1443_1.xml"]);
        m.insert("Robo de cajeros", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1444_1.xml"]);
        m.insert("Robo de vehículos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1445_1.xml"]);
        m.insert("Robos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1679_1.xml"]);
        m.insert("Robos vitrinas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1447_1.xml"]);
        m.insert("Secuestros", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_1448_1.xml"]);
        m.insert("Violencia intrafamiliar", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_158_2449_1.xml"]);
        m.insert("Conadi", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_165_1252_1.xml"]);
        m.insert("Mapuche", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_165_1402_1.xml"]);
        m.insert("Mineros atrapados San José", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_193_1518_1.xml"]);
        m.insert("Alcalde Coquimbo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_195_1525_1.xml"]);
        m.insert("Chiloé", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_197_1897_1.xml"]);
        m.insert("Torres del Paine", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_199_1551_1.xml"]);
        m.insert("Dunas de Concón", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_202_1560_1.xml"]);
        m.insert("Juan Fernández", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_202_1565_1.xml"]);
        m.insert("Ventanas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_202_1571_1.xml"]);
        m.insert("Hospital de Talca", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_204_1580_1.xml"]);
        m.insert("Temporeros", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_87_1689_1.xml"]);
        m.insert("Zona Austral", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_310_2446_1.xml"]);
        m.insert("Acuerdos comerciales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_635_1.xml"]);
        m.insert("Cumbres", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1598_1.xml"]);
        m.insert("Pacifico Sur", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_81_1612_1.xml"]);
        m.insert("Iglesias evangélicas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_206_1630_1.xml"]);
        m.insert("Judaísmo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_206_1881_1.xml"]);
        m.insert("Aborto", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_2127_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1018_1.xml"]);
        m.insert("Alertas sanitarias", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1667_1.xml"]);
        m.insert("Atención de urgencia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1632_1.xml"]);
        m.insert("Atención privada", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1671_1.xml"]);
        m.insert("Denuncias de negligencias", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1652_1.xml"]);
        m.insert("Discapacidad", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1268_1.xml"]);
        m.insert("Enfermedades respiratorias", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1639_1.xml"]);
        m.insert("Eutanasia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1640_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1748_1.xml"]);
        m.insert("Fonasa", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1641_1.xml"]);
        m.insert("Gremios", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1317_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_87_1315_1.xml"]);
        m.insert("Hospitales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1644_1.xml"]);
        m.insert("Intoxicaciones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1646_1.xml"]);
        m.insert("Isapre", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1647_1.xml"]);
        m.insert("Licencias médicas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1649_1.xml"]);
        m.insert("Médicos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1316_1.xml"]);
        m.insert("Medicamentos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1651_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1801_1.xml"]);
        m.insert("Píldora del día después", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1655_1.xml"]);
        m.insert("Trasplantes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1664_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1810_1.xml"]);
        m.insert("Vacunación", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_169_1665_1.xml"]);
        m.insert("Armas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_174_1291_1.xml"]);
        m.insert("Planes antidelincuencia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_174_1674_1.xml"]);
        m.insert("Agua", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_98_1675_1.xml"]);
        m.insert("Electricidad", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_98_1677_1.xml"]);
        m.insert("Telefonía", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_98_659_1.xml"]);
        m.insert("Registro Civil", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_205_1584_1.xml"]);
        m.insert("SII", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_205_1891_1.xml"]);
        m.insert("Norte grande", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_211_1680_1.xml"]);
        m.insert("Simulacros", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_211_1682_1.xml"]);
        m.insert("1 de mayo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_87_1686_1.xml"]);
        m.insert("Accidentes laborales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_87_1687_1.xml"]);
        m.insert("Cesantía", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_87_640_1.xml"]);
        m.insert("Negociaciones colectivas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_87_1692_1.xml"]);
        m.insert("Alcohol", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_214_1697_1.xml"]);
        m.insert("Automovilistas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_214_2207_1.xml"]);
        m.insert("Autopistas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_214_1695_1.xml"]);
        m.insert("Aviación", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_214_1694_1.xml"]);
        m.insert("Buses interprovinciales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_214_1696_1.xml"]);
        m.insert("Ciclistas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_214_2209_1.xml"]);
        m.insert("Ferrocarriles", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_214_1698_1.xml"]);
        m.insert("Merval", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_214_1699_1.xml"]);
        m.insert("Metro", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_214_1700_1.xml"]);
        m.insert("Transantiago", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_214_1693_1.xml"]);
        m.insert("Tránsito", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_214_1705_1.xml"]);
        m.insert("Acceso a playas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_215_1706_1.xml"]);
        m.insert("Vacaciones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_215_1708_1.xml"]);
        m.insert("Tomas de terrenos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_216_1714_1.xml"]);
        m.insert("Viviendas sociales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_3_216_1712_1.xml"]);
        m.insert("Eclipses", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_250_2452_1.xml"]);
        m.insert("EEI", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_250_1826_1.xml"]);
        m.insert("Espacio", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_250_1825_1.xml"]);
        m.insert("Estaciones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_250_1867_1.xml"]);
        m.insert("Luna", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_250_1828_1.xml"]);
        m.insert("Marte", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_250_1827_1.xml"]);
        m.insert("Programas espaciales", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_250_1829_1.xml"]);
        m.insert("Vida extraterrestre", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_250_1832_1.xml"]);
        m.insert("Año Nuevo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_220_1725_1.xml"]);
        m.insert("Halloween", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_220_1727_1.xml"]);
        m.insert("Navidad", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_220_1728_1.xml"]);
        m.insert("Congreso Futuro", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_221_2451_1.xml"]);
        m.insert("Física", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_221_1734_1.xml"]);
        m.insert("Genética", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_221_1735_1.xml"]);
        m.insert("Geología", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_221_1756_1.xml"]);
        m.insert("Paleontología", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_221_1775_1.xml"]);
        m.insert("Química", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_221_1737_1.xml"]);
        m.insert("Vulcanología", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_221_1739_1.xml"]);
        m.insert("Antropología", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_8_1722_1.xml"]);
        m.insert("Arqueología", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_8_1723_1.xml"]);
        m.insert("Demografía", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_8_1740_1.xml"]);
        m.insert("Filosofía", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_8_10_1.xml"]);
        m.insert("Lenguaje", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_8_12_1.xml"]);
        m.insert("Sicología", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_8_1877_1.xml",
        "https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1808_1.xml"]);
        m.insert("Amnistía Internacional", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_225_1741_1.xml"]);
        m.insert("Discriminación", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_225_1745_1.xml"]);
        m.insert("Migración", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_225_1743_1.xml"]);
        m.insert("Refugiados", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_225_1744_1.xml"]);
        m.insert("Trata de personas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_225_1869_1.xml"]);
        m.insert("Matrimonio", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_244_1816_1.xml"]);
        m.insert("Ballenas", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_230_1749_1.xml"]);
        m.insert("Extinción", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_230_1752_1.xml"]);
        m.insert("Osos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_230_1753_1.xml"]);
        m.insert("Simios", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_230_1754_1.xml"]);
        m.insert("Tiburones", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_230_1755_1.xml"]);
        m.insert("Cristóbal Colón", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_139_1759_1.xml"]);
        m.insert("Holocausto", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_139_1761_1.xml"]);
        m.insert("II Guerra Mundial", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_139_1065_1.xml"]);
        m.insert("Nazismo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_139_1774_1.xml"]);
        m.insert("Derechos del niño", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_233_1765_1.xml"]);
        m.insert("Clima", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_187_1736_1.xml"]);
        m.insert("COP25", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_187_2455_1.xml"]);
        m.insert("Greenpeace", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_187_1405_1.xml"]);
        m.insert("Reciclaje", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_187_1768_1.xml"]);
        m.insert("Diarios", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_234_1769_1.xml"]);
        m.insert("Religiosos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_136_1019_1.xml"]);
        m.insert("Cristianismo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_146_1783_1.xml"]);
        m.insert("Islam", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_146_1133_1.xml"]);
        m.insert("Alcoholismo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1787_1.xml"]);
        m.insert("Alimentación", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1803_1.xml"]);
        m.insert("Alzheimer", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1788_1.xml"]);
        m.insert("Cáncer", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1789_1.xml"]);
        m.insert("Corazón", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1791_1.xml"]);
        m.insert("Coronavirus", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_2458_1.xml"]);
        m.insert("Dengue", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_943_1.xml"]);
        m.insert("Depresión", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1792_1.xml"]);
        m.insert("Diabetes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1793_1.xml"]);
        m.insert("Gripes", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1798_1.xml"]);
        m.insert("Meningitis", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1802_1.xml"]);
        m.insert("Oftalmología", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1804_1.xml"]);
        m.insert("Parkinson", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1812_1.xml"]);
        m.insert("Sida", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1661_1.xml"]);
        m.insert("Tabaco", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_120_1662_1.xml"]);
        m.insert("Canibalismo", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_7_219_1724_1.xml"]);
        m.insert("Aeronáutica", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_246_1817_1.xml"]);
        m.insert("Automóviles", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_246_1818_1.xml"]);
        m.insert("Informática", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_246_1821_1.xml"]);
        m.insert("Mobile World Congress", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_246_1943_1.xml"]);
        m.insert("Robótica", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_246_1852_1.xml"]);
        m.insert("Telefonía móvil", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_246_1854_1.xml"]);
        m.insert("Videojuegos", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_246_1822_1.xml"]);
        m.insert("Conectividad", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_248_1820_1.xml"]);
        m.insert("Google", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_248_1840_1.xml"]);
        m.insert("Wikileaks", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_248_1849_1.xml"]);
        m.insert("Wikipedia", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_248_1845_1.xml"]);
        m.insert("Facebook", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_252_1838_1.xml"]);
        m.insert("Twitter", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_252_1848_1.xml"]);
        m.insert("Youtube", &["https://www.cooperativa.cl/noticias/site/tax/port/all/rss_8_252_1847_1.xml"]);
        m
    })
}
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
            title,
            description: item.description().unwrap_or(item.extensions().get("descent").and_then(|d| d.values().next()?.first()?.value.as_deref()).unwrap_or(title)).to_string(),
            description: item.description().unwrap_or("").to_string(),
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