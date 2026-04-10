"use client";

import { useEffect, useState } from "react";

export default function SudokuGame() {
  const [sudokuGameBoard, setSudokuGameBoard] = useState([]);
  const [sudokuSolvedBoard, setSudokuSolvedBoard] = useState([]);
  const [gameInitialized, setGameInitialized] = useState(false);

  const getCandidates = (board, row, col) => {
    const used = new Set();
    for (let i = 0; i < 9; i += 1) {
      used.add(board[row][i]);
      used.add(board[i][col]);
    }
    const startRow = row - (row % 3);
    const startCol = col - (col % 3);
    for (let i = 0; i < 3; i += 1) {
      for (let j = 0; j < 3; j += 1) {
        used.add(board[startRow + i][startCol + j]);
      }
    }
    const candidates = [];
    for (let n = 1; n <= 9; n += 1) {
      if (!used.has(n)) candidates.push(n);
    }
    return candidates.sort(() => Math.random() - 0.5);
  };

  const getBestCell = (board) => {
    let minCandidates = 10;
    let bestCell = null;
    for (let r = 0; r < 9; r += 1) {
      for (let c = 0; c < 9; c += 1) {
        if (board[r][c] === 0) {
          const candidates = getCandidates(board, r, c);
          if (candidates.length < minCandidates) {
            minCandidates = candidates.length;
            bestCell = { r, c, candidates };
          }
        }
      }
    }
    return bestCell;
  };

  const solve = (board) => {
    const cell = getBestCell(board);
    if (!cell) return true;
    for (const num of cell.candidates) {
      board[cell.r][cell.c] = num;
      if (solve(board)) return true;
      board[cell.r][cell.c] = 0;
    }
    return false;
  };

  const validateInput = (value, r, c) => {
    const val = parseInt(value, 10);
    if (!val) return { color: "inherit", backgroundColor: "white" };

    if (val === sudokuSolvedBoard[r][c]) {
      return { color: "#2ecc71", backgroundColor: "#e8f8f0" };
    } else {
      return { color: "#e74c3c", backgroundColor: "#fdedec" };
    }
  };

  const handleInputChange = (r, c, value) => {
    const newBoard = sudokuGameBoard.map(row => [...row]);
    newBoard[r][c] = value === "" ? 0 : parseInt(value, 10) || 0;
    setSudokuGameBoard(newBoard);
  };

  useEffect(() => {
    if (!gameInitialized) {
      let board = Array.from({ length: 9 }, () => Array(9).fill(0));
      solve(board);

      const solvedBoard = board.map(row => [...row]);
      const gameBoard = board.map(row => [...row]);

      let cellsToHide = 45;
      while (cellsToHide > 0) {
        const r = Math.floor(Math.random() * 9);
        const c = Math.floor(Math.random() * 9);
        if (gameBoard[r][c] !== 0) {
          gameBoard[r][c] = 0;
          cellsToHide -= 1;
        }
      }

      setSudokuSolvedBoard(solvedBoard);
      setSudokuGameBoard(gameBoard);
      setGameInitialized(true);
    }
  }, [gameInitialized]);

  if (!gameInitialized) {
    return <div className="sudoku-loading">Generando Sudoku...</div>;
  }

  return (
    <div className="sudoku-game">
      <h1>Sudoku</h1>
      <div className="sudoku-grid">
        {sudokuGameBoard.map((row, r) =>
          row.map((val, c) => {
            const isFixed = sudokuSolvedBoard[r][c] !== val && val !== 0;
            const inputStyle = isFixed ? validateInput(val, r, c) : {};

            return (
              <input
                key={`cell-${r}-${c}`}
                type="number"
                inputMode="numeric"
                className={`sudoku-input ${isFixed ? "" : "fixed-value"}`}
                value={val === 0 ? "" : val}
                readOnly={sudokuSolvedBoard[r][c] !== val && val !== 0}
                onChange={(e) => handleInputChange(r, c, e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "ArrowUp" || e.key === "ArrowDown") {
                    e.preventDefault();
                  }
                }}
                onWheel={(e) => e.preventDefault()}
                style={inputStyle}
                min="1"
                max="9"
              />
            );
          })
        )}
      </div>
    </div>
  );
}