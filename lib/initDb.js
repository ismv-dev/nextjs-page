#!/usr/bin/env node

/**
 * Script para inicializar la base de datos manualmente
 * Uso: node lib/initDb.js
 */

import dotenv from "dotenv";
dotenv.config();

import { initializeDatabase, syncAllNews } from "./newsSync.js";

async function main() {
  try {
    console.log("🔧 Inicializando base de datos...");
    await initializeDatabase();
    
    console.log("\n📡 Sincronizando noticias...");
    const results = await syncAllNews();
    
    console.log("\n✅ ¡Base de datos inicializada exitosamente!");
    console.log("Resultados de sincronización:", results);
    
    process.exit(0);
  } catch (error) {
    console.error("❌ Error:", error.message);
    process.exit(1);
  }
}

main();
