"use client";

import MinesweeperGame from "./MinesweeperGame";
import SudokuGame from "./SudokuGame";

export default function GamesSection({ selectedGame, onSelectGame }) {
  return (
    <div id="juegos-screen">
      <h1 className="page-title">Juegos</h1>
      <div className="game-grid">
        <button
          type="button"
          className="game-card"
          onClick={() => onSelectGame("minesweeper")}
        >
          <img src="/img/minesweeper.png" alt="Minesweeper" />
          <p>Minesweeper</p>
        </button>
        <button
          type="button"
          className="game-card"
          onClick={() => onSelectGame("sudoku")}
        >
          <img src="/img/sudoku.png" alt="Sudoku" />
          <p>Sudoku</p>
        </button>
      </div>
      <div id="game-wrapper" style={{ display: selectedGame ? "block" : "none" }}>
        {selectedGame === "minesweeper" && <MinesweeperGame />}
        {selectedGame === "sudoku" && <SudokuGame />}
      </div>
    </div>
  );
}
