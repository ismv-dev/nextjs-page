"use client";

import { useEffect, useRef, useState } from "react";

const DIFFICULTIES = {
  easy: { rows: 8, cols: 8, mines: 10 },
  medium: { rows: 16, cols: 16, mines: 40 },
  hard: { rows: 32, cols: 32, mines: 99 },
};

export default function MinesweeperGame() {
  const [config, setConfig] = useState(DIFFICULTIES.easy);
  const [currentDiff, setCurrentDiff] = useState("easy");
  const [board, setBoard] = useState([]);
  const [revealed, setRevealed] = useState([]);
  const [flagged, setFlagged] = useState([]);
  const [gameState, setGameState] = useState("idle");
  const [firstClick, setFirstClick] = useState(true);
  const [mineSet, setMineSet] = useState(new Set());
  const [timer, setTimer] = useState(0);
  const timerRef = useRef(null);
  const boardRef = useRef(null);

  const fmt = (n) => String(n).padStart(3, "0");

  const initGame = () => {
    if (timerRef.current) clearInterval(timerRef.current);
    setTimer(0);
    setGameState("idle");
    setFirstClick(true);
    setMineSet(new Set());

    const { rows, cols } = config;
    setBoard(Array.from({ length: rows }, () => Array(cols).fill(0)));
    setRevealed(Array.from({ length: rows }, () => Array(cols).fill(false)));
    setFlagged(Array.from({ length: rows }, () => Array(cols).fill(false)));
  };

  const placeMines = (safeR, safeC, currentBoard) => {
    const { rows, cols, mines } = config;
    const safe = new Set();
    for (let dr = -1; dr <= 1; dr += 1) {
      for (let dc = -1; dc <= 1; dc += 1) {
        const nr = safeR + dr;
        const nc = safeC + dc;
        if (nr >= 0 && nr < rows && nc >= 0 && nc < cols) {
          safe.add(nr * cols + nc);
        }
      }
    }

    const all = [];
    for (let i = 0; i < rows * cols; i += 1) {
      if (!safe.has(i)) all.push(i);
    }

    for (let i = all.length - 1; i > 0; i -= 1) {
      const j = Math.floor(Math.random() * (i + 1));
      [all[i], all[j]] = [all[j], all[i]];
    }

    const hasAdjacentMine = (idx, mineIndices) => {
      const r = Math.floor(idx / cols);
      const c = idx % cols;
      for (let dr = -1; dr <= 1; dr += 1) {
        for (let dc = -1; dc <= 1; dc += 1) {
          if (dr === 0 && dc === 0) continue;
          const nr = r + dr;
          const nc = c + dc;
          if (nr >= 0 && nr < rows && nc >= 0 && nc < cols) {
            if (mineIndices.has(nr * cols + nc)) return true;
          }
        }
      }
      return false;
    };

    const selectedMines = new Set();
    for (const idx of all) {
      if (selectedMines.size >= mines) break;
      if (!hasAdjacentMine(idx, selectedMines)) {
        selectedMines.add(idx);
      }
    }

    if (selectedMines.size < mines) {
      for (const idx of all) {
        if (selectedMines.size >= mines) break;
        selectedMines.add(idx);
      }
    }

    const newMineSet = new Set(selectedMines);
    const newBoard = currentBoard.map(row => [...row]);

    newMineSet.forEach((idx) => {
      const r = Math.floor(idx / cols);
      const c = idx % cols;
      newBoard[r][c] = -1;
      for (let dr = -1; dr <= 1; dr += 1) {
        for (let dc = -1; dc <= 1; dc += 1) {
          const nr = r + dr;
          const nc = c + dc;
          if (nr >= 0 && nr < rows && nc >= 0 && nc < cols && newBoard[nr][nc] !== -1) {
            newBoard[nr][nc] += 1;
          }
        }
      }
    });

    return { newBoard, newMineSet };
  };

  const flood = (r, c, currentRevealed, currentBoard) => {
    if (r < 0 || r >= config.rows || c < 0 || c >= config.cols) return currentRevealed;
    if (currentRevealed[r][c] || flagged[r][c]) return currentRevealed;

    const newRevealed = currentRevealed.map(row => [...row]);
    newRevealed[r][c] = true;

    if (currentBoard[r][c] === 0) {
      for (let dr = -1; dr <= 1; dr += 1) {
        for (let dc = -1; dc <= 1; dc += 1) {
          if (dr !== 0 || dc !== 0) {
            const result = flood(r + dr, c + dc, newRevealed, currentBoard);
            newRevealed.splice(0, newRevealed.length, ...result);
          }
        }
      }
    }

    return newRevealed;
  };

  const revealMine = (r, c) => {
    if (timerRef.current) clearInterval(timerRef.current);
    setGameState("lost");

    setRevealed(prev => {
      const newRevealed = prev.map(row => [...row]);
      newRevealed[r][c] = true;
      return newRevealed;
    });

    const selectedIdx = r * config.cols + c;
    const otherMines = Array.from(mineSet).filter(idx => idx !== selectedIdx);

    const shuffle = (array) => {
      const result = [...array];
      for (let i = result.length - 1; i > 0; i -= 1) {
        const j = Math.floor(Math.random() * (i + 1));
        [result[i], result[j]] = [result[j], result[i]];
      }
      return result;
    };

    const revealOrder = shuffle(otherMines);
    revealOrder.forEach((idx, index) => {
      const mr = Math.floor(idx / config.cols);
      const mc = idx % config.cols;
      setTimeout(() => {
        setRevealed(prev => {
          const newRevealed = prev.map(row => [...row]);
          if (!newRevealed[mr][mc]) {
            newRevealed[mr][mc] = true;
          }
          return newRevealed;
        });
      }, 25 * (index + 1));
    });
  };

  const checkWin = (currentRevealed) => {
    const { rows, cols, mines } = config;
    let revealedCount = 0;
    for (let r = 0; r < rows; r += 1) {
      for (let c = 0; c < cols; c += 1) {
        if (currentRevealed[r][c]) revealedCount += 1;
      }
    }

    if (revealedCount === rows * cols - mines) {
      if (timerRef.current) clearInterval(timerRef.current);
      setGameState("won");
    }
  };

  const handleCellClick = (r, c) => {
    if (gameState === "won" || gameState === "lost") return;
    if (flagged[r][c] || revealed[r][c]) return;

    let currentBoard = board;

    if (firstClick) {
      setFirstClick(false);
      const { newBoard, newMineSet } = placeMines(r, c, board);
      setMineSet(newMineSet);
      setBoard(newBoard);
      currentBoard = newBoard;
      setGameState("playing");
      timerRef.current = setInterval(() => {
        setTimer(prev => {
          const newTimer = prev + 1;
          if (newTimer >= 999) {
            if (timerRef.current) clearInterval(timerRef.current);
            return 999;
          }
          return newTimer;
        });
      }, 1000);
    }

    if (currentBoard[r][c] === -1) {
      revealMine(r, c);
      return;
    }

    const newRevealed = flood(r, c, revealed, currentBoard);
    setRevealed(newRevealed);
    checkWin(newRevealed);
  };

  const handleCellRightClick = (e, r, c) => {
    e.preventDefault();
    if (gameState === "won" || gameState === "lost") return;
    if (revealed[r][c]) return;

    setFlagged(prev => {
      const newFlagged = prev.map(row => [...row]);
      newFlagged[r][c] = !newFlagged[r][c];
      return newFlagged;
    });
  };

  const handleDifficultyChange = (diff) => {
    setCurrentDiff(diff);
    setConfig(DIFFICULTIES[diff]);
  };

  useEffect(() => {
    initGame();
  }, [config]);

  useEffect(() => {
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, []);

  const renderCell = (r, c) => {
    const cellClasses = ["cell"];
    let cellContent = "";
    let cellStyle = {};

    if (revealed[r] && revealed[r][c]) {
      cellClasses.push("revealed");
      const v = board[r] && board[r][c];
      if (v === -1) {
        cellContent = "💥";
        cellClasses.push("mine-hit");
      } else if (v > 0) {
        cellContent = String(v);
        cellClasses.push(`n${v}`);
      }
    } else if (flagged[r] && flagged[r][c]) {
      cellClasses.push("flagged");
      cellContent = "⚑";
      cellStyle = { color: "var(--warn)", textShadow: "0 0 8px rgba(255,204,0,0.5)" };
    }

    return (
      <div
        key={`${r}-${c}`}
        className={cellClasses.join(" ")}
        style={cellStyle}
        onClick={() => handleCellClick(r, c)}
        onContextMenu={(e) => handleCellRightClick(e, r, c)}
      >
        {cellContent}
      </div>
    );
  };

  return (
    <div className="minesweeper-game">
      <div className="game-header">
        <div className="indicator">
          <span className="indicator-label">Time</span>
          <span className="timer">{fmt(timer)}</span>
        </div>
        <div className="difficulty-buttons">
          {Object.keys(DIFFICULTIES).map((diff) => (
            <button
              key={diff}
              className={`diff-btn ${currentDiff === diff ? "active" : ""}`}
              onClick={() => handleDifficultyChange(diff)}
            >
              {diff.charAt(0).toUpperCase() + diff.slice(1)}
            </button>
          ))}
        </div>
        <div className="indicator">
          <span className="indicator-label">Mines</span>
          <span className="mine-count">
            {fmt(config.mines - flagged.flat().filter(f => f).length)}
          </span>
        </div>
      </div>

      <div
        className="board"
        ref={boardRef}
        style={{
          gridTemplateColumns: `repeat(${config.cols}, 1fr)`,
          display: "grid"
        }}
      >
        {board.map((row, r) =>
          row.map((_, c) => renderCell(r, c))
        )}
      </div>

      {(gameState === "won" || gameState === "lost") && (
        <div className="game-overlay visible">
          <div className="overlay-content">
            <div className={`overlay-title ${gameState}`}>
              {gameState === "won" ? "CLEARED" : "DETONATED"}
            </div>
            <div className="overlay-sub">
              {gameState === "won"
                ? `COMPLETED IN ${timer}s`
                : `SURVIVED ${timer}s — TRY AGAIN?`
              }
            </div>
            <button
              className={`overlay-btn ${gameState}`}
              onClick={initGame}
            >
              {gameState === "won" ? "PLAY AGAIN" : "RETRY"}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}