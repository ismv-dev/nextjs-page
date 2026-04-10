let sudokuSolvedBoard = [];
let sudokuGameBoard = [];

function renderMinesweeper() {
  const wrapper = document.getElementById("minesweeper-wrapper");
  if (!wrapper) return;

  wrapper.innerHTML = `
        <header>
            <h1>MINESWEEPER</h1>
            <p>TACTICAL GRID CLEARANCE SYSTEM</p>
        </header>
        
        <div class="difficulty-bar">
            <button class="diff-btn active" data-diff="easy">EASY</button>
            <button class="diff-btn" data-diff="medium">MEDIUM</button>
            <button class="diff-btn" data-diff="hard">HARD</button>
        </div>
        
        <div class="hud">
            <div>
                <div class="hud-display mines" id="mine-count">000</div>
                <span class="hud-label">MINES</span>
            </div>
            <button class="reset-btn" id="reset-btn" title="New Game">
                <img src="/img/smile.webp" alt="" style="width: 100%; height: 100%; object-fit: cover; object-position: center center;">
            </button>
            <div>
                <div class="hud-display timer" id="timer">000</div>
                <span class="hud-label">SECONDS</span>
            </div>
        </div>
        
        <div class="board-container">
            <div id="board"></div>
            <div class="overlay" id="overlay">
                <div class="overlay-title" id="overlay-title"></div>
                <div class="overlay-sub" id="overlay-sub"></div>
                <button class="overlay-btn" id="overlay-btn"></button>
            </div>
        </div>
    `;
}

function getCandidates(board, row, col) {
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
}

function getBestCell(board) {
  let minCandidates = 10;
  let bestCell = null;

  for (let r = 0; r < 9; r += 1) {
    for (let c = 0; c < 9; c += 1) {
      if (board[r][c] !== 0) continue;
      const candidates = getCandidates(board, r, c);
      if (candidates.length < minCandidates) {
        minCandidates = candidates.length;
        bestCell = { r, c, candidates };
      }
    }
  }

  return bestCell;
}

function solve(board) {
  const cell = getBestCell(board);
  if (!cell) return true;

  for (const num of cell.candidates) {
    board[cell.r][cell.c] = num;
    if (solve(board)) return true;
    board[cell.r][cell.c] = 0;
  }

  return false;
}

function renderSudoku() {
  const wrapper = document.getElementById("minesweeper-wrapper");
  if (!wrapper) return;

  wrapper.innerHTML = `
        <h1>Sudoku</h1>
        <div id="sudoku-grid"></div>
    `;
}

function renderUI() {
  const grid = document.getElementById("sudoku-grid");
  if (!grid) return;

  grid.innerHTML = "";

  sudokuGameBoard.forEach((row, r) => {
    row.forEach((val, c) => {
      const input = document.createElement("input");
      input.type = "number";
      input.id = `cell-${r}-${c}`;
      input.className = "sudoku-input";

      if (val !== 0) {
        input.value = val;
        input.readOnly = true;
      } else {
        input.addEventListener("input", () => validateInput(input, r, c));
      }

      grid.appendChild(input);
    });
  });
}

function validateInput(input, r, c) {
  const val = parseInt(input.value, 10);
  if (!val) {
    input.style.backgroundColor = "white";
    input.style.color = "inherit";
    return;
  }

  if (val === sudokuSolvedBoard[r][c]) {
    input.style.color = "#2ecc71";
    input.style.backgroundColor = "#e8f8f0";
  } else {
    input.style.color = "#e74c3c";
    input.style.backgroundColor = "#fdedec";
  }
}

export function iniciarMinesweeper() {
  renderMinesweeper();

  const DIFFICULTIES = {
    easy: { rows: 9, cols: 9, mines: 10 },
    medium: { rows: 16, cols: 16, mines: 40 },
    hard: { rows: 16, cols: 30, mines: 99 },
  };

  let config = DIFFICULTIES.easy;
  let board = [];
  let revealed = [];
  let flagged = [];
  let mineSet = new Set();
  let gameState = "idle";
  let timer = 0;
  let timerInterval = null;
  let firstClick = true;
  let currentDiff = "easy";

  const boardEl = document.getElementById("board");
  const mineCountEl = document.getElementById("mine-count");
  const timerEl = document.getElementById("timer");
  const overlayEl = document.getElementById("overlay");
  const overlayTitle = document.getElementById("overlay-title");
  const overlaySub = document.getElementById("overlay-sub");
  const overlayBtn = document.getElementById("overlay-btn");

  const fmt = (value) => String(value).padStart(3, "0");

  function initGame() {
    clearInterval(timerInterval);
    timer = 0;
    timerEl.textContent = fmt(0);
    gameState = "idle";
    firstClick = true;
    mineSet = new Set();
    overlayEl.classList.remove("visible");

    const { rows, cols } = config;
    board = Array.from({ length: rows }, () => Array(cols).fill(0));
    revealed = Array.from({ length: rows }, () => Array(cols).fill(false));
    flagged = Array.from({ length: rows }, () => Array(cols).fill(false));

    updateMineCount();
    renderBoard();
  }

  function placeMines(safeR, safeC) {
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

    for (let i = 0; i < mines; i += 1) {
      mineSet.add(all[i]);
    }

    mineSet.forEach((idx) => {
      const r = Math.floor(idx / cols);
      const c = idx % cols;
      board[r][c] = -1;
      for (let dr = -1; dr <= 1; dr += 1) {
        for (let dc = -1; dc <= 1; dc += 1) {
          const nr = r + dr;
          const nc = c + dc;
          if (nr >= 0 && nr < rows && nc >= 0 && nc < cols && board[nr][nc] !== -1) {
            board[nr][nc] += 1;
          }
        }
      }
    });
  }

  function renderBoard() {
    const { rows, cols } = config;
    boardEl.style.gridTemplateColumns = `repeat(${cols}, 1fr)`;
    boardEl.innerHTML = "";

    for (let r = 0; r < rows; r += 1) {
      for (let c = 0; c < cols; c += 1) {
        const cell = document.createElement("div");
        cell.className = "cell";
        cell.dataset.r = String(r);
        cell.dataset.c = String(c);
        cell.addEventListener("click", onCellClick);
        cell.addEventListener("contextmenu", onCellRightClick);
        boardEl.appendChild(cell);
      }
    }
  }

  function updateCell(r, c) {
    const cell = boardEl.children[r * config.cols + c];
    if (!cell) return;

    cell.className = "cell";
    cell.textContent = "";

    if (revealed[r][c]) {
      cell.classList.add("revealed");
      const value = board[r][c];
      if (value === -1) {
        cell.textContent = "💥";
        cell.classList.add("mine-hit");
      } else if (value > 0) {
        cell.textContent = String(value);
        cell.classList.add(`n${value}`);
      }
    } else if (flagged[r][c]) {
      cell.classList.add("flagged");
      cell.textContent = "⚑";
      cell.style.color = "var(--warn)";
    }
  }

  function onCellClick(event) {
    if (gameState === "won" || gameState === "lost") return;
    const r = Number(event.currentTarget.dataset.r);
    const c = Number(event.currentTarget.dataset.c);
    if (flagged[r][c] || revealed[r][c]) return;

    if (firstClick) {
      firstClick = false;
      placeMines(r, c);
      gameState = "playing";
      timerInterval = window.setInterval(() => {
        timer += 1;
        timerEl.textContent = fmt(timer);
        if (timer >= 999) {
          timerEl.textContent = "999";
          clearInterval(timerInterval);
        }
      }, 1000);
    }

    if (board[r][c] === -1) {
      revealMine(r, c);
      return;
    }

    flood(r, c);
    checkWin();
    updateStats();
  }

  function onCellRightClick(event) {
    event.preventDefault();
    if (gameState === "won" || gameState === "lost") return;
    const r = Number(event.currentTarget.dataset.r);
    const c = Number(event.currentTarget.dataset.c);
    if (revealed[r][c]) return;

    flagged[r][c] = !flagged[r][c];
    updateCell(r, c);
    updateMineCount();
    updateStats();
  }

  function flood(r, c) {
    if (r < 0 || r >= config.rows || c < 0 || c >= config.cols) return;
    if (revealed[r][c] || flagged[r][c]) return;

    revealed[r][c] = true;
    updateCell(r, c);

    if (board[r][c] === 0) {
      for (let dr = -1; dr <= 1; dr += 1) {
        for (let dc = -1; dc <= 1; dc += 1) {
          if (dr !== 0 || dc !== 0) {
            flood(r + dr, c + dc);
          }
        }
      }
    }
  }

  function revealMine(r, c) {
    clearInterval(timerInterval);
    gameState = "lost";
    revealed[r][c] = true;
    updateCell(r, c);

    let delay = 60;
    mineSet.forEach((idx) => {
      const mr = Math.floor(idx / config.cols);
      const mc = idx % config.cols;
      if (mr === r && mc === c) return;
      window.setTimeout(() => {
        if (!revealed[mr][mc]) {
          const cell = boardEl.children[mr * config.cols + mc];
          if (cell) {
            cell.className = "cell mine-shown";
            cell.textContent = "💣";
          }
        }
      }, delay);
      delay += 30;
    });

    window.setTimeout(() => {
      overlayTitle.textContent = "DETONATED";
      overlayTitle.className = "overlay-title lose";
      overlaySub.textContent = `SURVIVED ${timer}s — TRY AGAIN?`;
      overlayBtn.textContent = "RETRY";
      overlayBtn.className = "overlay-btn lose";
      overlayEl.classList.add("visible");
    }, delay + 100);
  }

  function checkWin() {
    const { rows, cols, mines } = config;
    let revealedCount = 0;

    for (let r = 0; r < rows; r += 1) {
      for (let c = 0; c < cols; c += 1) {
        if (revealed[r][c]) revealedCount += 1;
      }
    }

    if (revealedCount === rows * cols - mines) {
      clearInterval(timerInterval);
      gameState = "won";
      overlayTitle.textContent = "CLEARED";
      overlayTitle.className = "overlay-title win";
      overlaySub.textContent = `COMPLETED IN ${timer}s`;
      overlayBtn.textContent = "PLAY AGAIN";
      overlayBtn.className = "overlay-btn win";
      overlayEl.classList.add("visible");
    }
  }

  function updateMineCount() {
    let flags = 0;
    flagged.forEach((row) => row.forEach((flag) => {
      if (flag) flags += 1;
    }));
    mineCountEl.textContent = fmt(Math.max(0, config.mines - flags));
  }

  function updateStats() {
    // Deferred to the original implementation.
  }

  overlayBtn.addEventListener("click", initGame);
  document.querySelectorAll(".diff-btn").forEach((btn) => {
    btn.addEventListener("click", () => {
      document.querySelectorAll(".diff-btn").forEach((button) => button.classList.remove("active"));
      btn.classList.add("active");
      currentDiff = btn.dataset.diff;
      config = DIFFICULTIES[currentDiff];
      initGame();
    });
  });

  boardEl.addEventListener("contextmenu", (event) => event.preventDefault());
  initGame();
}

export function iniciarSudoku() {
  renderSudoku();

  const board = Array.from({ length: 9 }, () => Array(9).fill(0));
  solve(board);

  sudokuSolvedBoard = board.map((row) => [...row]);
  sudokuGameBoard = board.map((row) => [...row]);

  let cellsToHide = 45;
  while (cellsToHide > 0) {
    const r = Math.floor(Math.random() * 9);
    const c = Math.floor(Math.random() * 9);
    if (sudokuGameBoard[r][c] !== 0) {
      sudokuGameBoard[r][c] = 0;
      cellsToHide -= 1;
    }
  }

  renderUI();
}
