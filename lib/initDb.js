#!/usr/bin/env node

/**
 * Script para inicializar la base de datos manualmente
 * Uso: node lib/initDb.js
 */

import dotenv from "dotenv";
dotenv.config();

import { initializeDatabase } from "./db.js";

async function main() {
  try {
    console.log("Iniciando base de datos");
    await initializeDatabase();
    
    console.log("\nBase de datos inicializada exitosamente");
    
    process.exit(0);
  } catch (error) {
    console.error("❌ Error:", error.message);
    process.exit(1);
  }
}

main();
