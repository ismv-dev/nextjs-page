"use client";

import { useEffect, useState } from "react";
import Sidebar from "./components/Sidebar";
import TriviaSection from "./components/TriviaSection";
import OperationSection from "./components/OperationSection";
import GamesSection from "./components/GamesSection";
import QuestionSummary from "./components/QuestionSummary";
import { buildArithmeticExpression, getBinaryConversion } from "./lib/operations";
import NewsSection from "./components/NewsSection";
import { useRef } from "react";

let sudokuGameBoard = [];
let sudokuSolvedBoard = [];

const NIVELES_PREGUNTA = ["Bajo", "Normal", "Avanzado", "Experto"];

const randomQuestionType = () => {
  const types = ["Verdadero o falso", "Abierta", "Alternativas", "Escenario"];
  return types[Math.floor(Math.random() * types.length)];
};

function getRandomInt(min, max) {
  const minCeil = Math.ceil(min);
  const maxFloor = Math.floor(max);
  return Math.floor(Math.random() * (maxFloor - minCeil + 1)) + minCeil;
}

function renderMinesweeper() {
  const wrapper = document.getElementById('minesweeper-wrapper');
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

function iniciarMinesweeper() {
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
  let gameState = 'idle';
  let timer = 0;
  let timerInterval = null;
  let firstClick = true;
  let currentDiff = 'easy';

  const boardEl = document.getElementById('board');
  const mineCountEl = document.getElementById('mine-count');
  const timerEl = document.getElementById('timer');
  const overlayEl = document.getElementById('overlay');
  const overlayTitle = document.getElementById('overlay-title');
  const overlaySub = document.getElementById('overlay-sub');
  const overlayBtn = document.getElementById('overlay-btn');

  const fmt = (n) => String(n).padStart(3, '0');

  function initGame() {
    clearInterval(timerInterval);
    timer = 0;
    timerEl.textContent = fmt(0);
    gameState = 'idle';
    firstClick = true;
    mineSet = new Set();
    overlayEl.classList.remove('visible');

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
    boardEl.innerHTML = '';

    for (let r = 0; r < rows; r += 1) {
      for (let c = 0; c < cols; c += 1) {
        const cell = document.createElement('div');
        cell.className = 'cell';
        cell.dataset.r = String(r);
        cell.dataset.c = String(c);
        cell.addEventListener('click', onCellClick);
        cell.addEventListener('contextmenu', onCellRightClick);
        boardEl.appendChild(cell);
      }
    }
  }

  function updateCell(r, c) {
    const cell = boardEl.children[r * config.cols + c];
    if (!cell) return;
    cell.className = 'cell';
    cell.textContent = '';

    if (revealed[r][c]) {
      cell.classList.add('revealed');
      const v = board[r][c];
      if (v === -1) {
        cell.textContent = '💥';
        cell.classList.add('mine-hit');
      } else if (v > 0) {
        cell.textContent = String(v);
        cell.classList.add(`n${v}`);
      }
    } else if (flagged[r][c]) {
      cell.classList.add('flagged');
      cell.textContent = '⚑';
      cell.style.color = 'var(--warn)';
      cell.style.textShadow = '0 0 8px rgba(255,204,0,0.5)';
    }
  }

  function onCellClick(e) {
    if (gameState === 'won' || gameState === 'lost') return;
    const r = Number(e.currentTarget.dataset.r);
    const c = Number(e.currentTarget.dataset.c);
    if (flagged[r][c] || revealed[r][c]) return;

    if (firstClick) {
      firstClick = false;
      placeMines(r, c);
      gameState = 'playing';
      timerInterval = window.setInterval(() => {
        timer += 1;
        timerEl.textContent = fmt(timer);
        if (timer >= 999) {
          timerEl.textContent = '999';
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

  function onCellRightClick(e) {
    e.preventDefault();
    if (gameState === 'won' || gameState === 'lost') return;
    const r = Number(e.currentTarget.dataset.r);
    const c = Number(e.currentTarget.dataset.c);
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
    gameState = 'lost';
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
            cell.className = 'cell mine-shown';
            cell.textContent = '💣';
          }
        }
      }, delay);
      delay += 30;
    });

    window.setTimeout(() => {
      overlayTitle.textContent = 'DETONATED';
      overlayTitle.className = 'overlay-title lose';
      overlaySub.textContent = `SURVIVED ${timer}s — TRY AGAIN?`;
      overlayBtn.textContent = 'RETRY';
      overlayBtn.className = 'overlay-btn lose';
      overlayEl.classList.add('visible');
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
      gameState = 'won';
      overlayTitle.textContent = 'CLEARED';
      overlayTitle.className = 'overlay-title win';
      overlaySub.textContent = `COMPLETED IN ${timer}s`;
      overlayBtn.textContent = 'PLAY AGAIN';
      overlayBtn.className = 'overlay-btn win';
      overlayEl.classList.add('visible');
    }
  }

  function updateMineCount() {
    let flags = 0;
    flagged.forEach((row) => row.forEach((f) => {
      if (f) flags += 1;
    }));
    const remaining = config.mines - flags;
    mineCountEl.textContent = fmt(Math.max(0, remaining));
  }

  function updateStats() {
    // No visible stats required for the migrated version.
  }

  overlayBtn.addEventListener('click', initGame);
  document.querySelectorAll('.diff-btn').forEach((btn) => {
    btn.addEventListener('click', () => {
      document.querySelectorAll('.diff-btn').forEach((b) => b.classList.remove('active'));
      btn.classList.add('active');
      currentDiff = btn.dataset.diff;
      config = DIFFICULTIES[currentDiff];
      initGame();
    });
  });
  boardEl.addEventListener('contextmenu', (e) => e.preventDefault());
  initGame();
}

function renderSudoku() {
  const wrapper = document.getElementById('minesweeper-wrapper');
  if (!wrapper) return;

  wrapper.innerHTML = `
        <h1>Sudoku</h1>
        <div id="sudoku-grid"></div>
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

function renderUI() {
  const grid = document.getElementById('sudoku-grid');
  if (!grid) return;
  grid.innerHTML = '';

  sudokuGameBoard.forEach((row, r) => {
    row.forEach((val, c) => {
      const input = document.createElement('input');
      input.type = 'number';
      input.id = `cell-${r}-${c}`;
      input.className = 'sudoku-input';

      if (val !== 0) {
        input.value = val;
        input.readOnly = true;
        input.classList.add('fixed');
      } else {
        input.addEventListener('input', () => validateInput(input, r, c));
      }
      grid.appendChild(input);
    });
  });
}

function validateInput(input, r, c) {
  const val = parseInt(input.value, 10);
  if (!val) {
    input.style.backgroundColor = 'white';
    input.style.color = 'inherit';
    return;
  }

  if (val === sudokuSolvedBoard[r][c]) {
    input.style.color = '#2ecc71';
    input.style.backgroundColor = '#e8f8f0';
  } else {
    input.style.color = '#e74c3c';
    input.style.backgroundColor = '#fdedec';
  }
}

function iniciarSudoku() {
  renderSudoku();
  let board = Array.from({ length: 9 }, () => Array(9).fill(0));
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

export default function Home() {
  const [view, setView] = useState("trivias");
  const [theme, setTheme] = useState("light");
  const [topic, setTopic] = useState("");
  const [questionLevel, setQuestionLevel] = useState(1);
  const [questions, setQuestions] = useState([]);
  const [activeIndex, setActiveIndex] = useState(0);
  const [isLoading, setIsLoading] = useState(false);
  const [feedback, setFeedback] = useState("");
  const [summaryOpen, setSummaryOpen] = useState(false);
  const [operationMode, setOperationMode] = useState(0);
  const [operationDisplay, setOperationDisplay] = useState("2+2");
  const [operationAnswer, setOperationAnswer] = useState("");
  const [operationFeedback, setOperationFeedback] = useState("");
  const [selectedGame, setSelectedGame] = useState(null);

  // --- News State ---
  const [newsArticles, setNewsArticles] = useState([]);
  const [newsLoading, setNewsLoading] = useState(false);
  const [newsSyncing, setNewsSyncing] = useState(false);
  const [newsError, setNewsError] = useState("");
  const [newsLastUpdate, setNewsLastUpdate] = useState(null);
  const [newsOffset, setNewsOffset] = useState(0);
  const [newsHasMore, setNewsHasMore] = useState(true);
  const [newsCategories, setNewsCategories] = useState([]);
  const [newsFilters, setNewsFilters] = useState({
    selectedCategories: [],
    startDate: "",
    endDate: "",
  });
  const newsFetchingRef = useRef(false);
  const LIMIT = 10;

  useEffect(() => {
    const date = new Date();
    const start = new Date();
    start.setMonth(start.getMonth() - 3);
    
    setNewsFilters({
      selectedCategories: [],
      startDate: start.toISOString().split('T')[0],
      endDate: date.toISOString().split('T')[0],
    });
  }, []);

  const fetchNews = async (currentOffset, isInitial, signal) => {
    if (newsFetchingRef.current) return;
    newsFetchingRef.current = true;

    if (isInitial) setNewsLoading(true);
    if (isInitial) {
      setNewsSyncing(false);
      setNewsError("");
    }

    try {
      const { selectedCategories, startDate, endDate } = newsFilters;
      const categoryQuery = selectedCategories.length > 0 
        ? `categories=${encodeURIComponent(selectedCategories.join(','))}` 
        : "";
      const dateQuery = `startDate=${startDate}&endDate=${endDate}`;
      const query = [categoryQuery, dateQuery].filter(Boolean).join('&');
      
      const response = await fetch(`/api/news?${query}&limit=${LIMIT}&offset=${currentOffset}`, {
        signal: signal,
      });

      if (!response.ok) {
        const body = await response.json().catch(() => ({}));
        if (body.requiresSync) {
          setNewsSyncing(true);
          return;
        }
        throw new Error(body.error || "No se pudo cargar las noticias");
      }

      const data = await response.json();
      const newArticles = data.items || [];
      
      setNewsArticles(prev => isInitial ? newArticles : [...prev, ...newArticles]);
      setNewsHasMore(newArticles.length === LIMIT);
      setNewsLastUpdate(new Date(data.timestamp).toLocaleTimeString("es-CL", {
                      hour: "2-digit",
                      minute: "2-digit",
                      hour12: false
                    }));
      setNewsCategories(data.allCategories);
      setNewsSyncing(false);
    } catch (error) {
      if (error.name !== "AbortError") {
        setNewsError(error.message || "Error al obtener noticias");
        if (isInitial) setNewsArticles([]);
      }
    } finally {
      setNewsLoading(false);
      newsFetchingRef.current = false;
    }
  };

  useEffect(() => {
    if (!newsFilters.startDate) return;
    const controller = new AbortController();
    setNewsOffset(0);
    setNewsHasMore(true);
    fetchNews(0, true, controller.signal);
    return () => controller.abort();
  }, [newsFilters]);

  const handleFetchNextPage = () => {
    const nextOffset = newsOffset + LIMIT;
    setNewsOffset(nextOffset);
    fetchNews(nextOffset, false, undefined);
  };

  useEffect(() => {
    const saved = window.localStorage.getItem("theme");
    if (saved) {
      setTheme(saved);
    } else if (window.matchMedia("(prefers-color-scheme: dark)").matches) {
      setTheme("dark");
    }
  }, []);

  useEffect(() => {
    document.body.classList.toggle("dark-mode", theme === "dark");
    window.localStorage.setItem("theme", theme);
  }, [theme]);

  useEffect(() => {
    if (view === "operaciones") {
      updateOperationDisplay(operationMode);
      setOperationFeedback("");
    }
    if (view !== "juegos") {
      setSelectedGame(null);
    }
  }, [view, operationMode]);

  useEffect(() => {
    updateOperationDisplay(operationMode);
  }, [operationMode]);

  const activeQuestion = questions[activeIndex] || null;

  const updateQuestion = (index, patch) => {
    setQuestions((prev) => prev.map((item, idx) => (idx === index ? { ...item, ...patch } : item)));
  };

  const handleGenerateQuestion = async () => {
    if (!topic.trim()) {
      setFeedback("Ingresa un tema para la trivia.");
      return;
    }

    setFeedback("");
    setIsLoading(true);

    try {
      const selectedType = randomQuestionType();
      const response = await fetch("/api/generate", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          theme: topic.trim(),
          type: selectedType,
          level: NIVELES_PREGUNTA[questionLevel],
        }),
      });
      const data = await response.json();
      if (!response.ok) {
        setFeedback(data.error || "No se pudo generar la pregunta.");
        return;
      }

      setQuestions((prev) => [
        ...prev,
        {
          pregunta: data.pregunta || "",
          alternativas: data.alternativas || [],
          correcta: data.correcta ?? -1,
          type: selectedType,
          answer: null,
          validation: undefined,
        },
      ]);
      setActiveIndex(questions.length);
      setSummaryOpen(false);
    } catch (error) {
      setFeedback("Error generando la pregunta.");
    } finally {
      setIsLoading(false);
    }
  };

  const handleAnswerChange = (value) => {
    if (!activeQuestion) return;
    updateQuestion(activeIndex, { answer: value });
  };

  const handleSelectQuestion = (index) => {
    if (index < 0 || index >= questions.length) return;
    setActiveIndex(index);
    setFeedback("");
    setSummaryOpen(false);
  };

  const handleNextQuestion = async () => {
    if (!activeQuestion) {
      setFeedback("Genera una pregunta primero.");
      return;
    }

    if (activeQuestion.alternativas.length) {
      if (activeQuestion.answer === null || activeQuestion.answer === undefined) {
        setFeedback("Selecciona una alternativa.");
        return;
      }
      updateQuestion(activeIndex, { answered: true, answer: activeQuestion.answer });
    } else {
      const answerText = (activeQuestion.answer || "").toString().trim();
      if (!answerText) {
        setFeedback("Escribe una respuesta.");
        return;
      }

      if (activeQuestion.validation === undefined) {
        setIsLoading(true);
        try {
          const response = await fetch("/api/validate", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ pregunta: activeQuestion.pregunta, respuesta: answerText }),
          });
          const data = await response.json();
          updateQuestion(activeIndex, { validation: data.validation });
        } catch (error) {
          setFeedback("Error validando la respuesta.");
        } finally {
          setIsLoading(false);
        }
      }
    }

    const nextIndex = activeIndex + 1;
    if (nextIndex < questions.length) {
      setActiveIndex(nextIndex);
    } else {
      await handleGenerateQuestion();
    }
  };

  const handleFinish = () => {
    if (!questions.length) {
      setFeedback("Genera al menos una pregunta antes de terminar.");
      return;
    }
    setSummaryOpen(true);
  };

  const updateOperationDisplay = (mode) => {
    if (mode === 0) {
      setOperationDisplay(buildArithmeticExpression());
    } else {
      setOperationDisplay(getBinaryConversion());
    }
  };

  const handleVerifyOperation = () => {
    try {
      if (operationMode === 0) {
        const expected = parseFloat(eval(operationDisplay));
        const userValue = parseFloat(operationAnswer || "");
        if (Number.isNaN(userValue)) {
          setOperationFeedback("Ingresa un número válido.");
          return;
        }
        if (Math.abs(userValue - expected) < 1e-9) {
          setOperationFeedback("Correcto!");
        } else {
          setOperationFeedback("Incorrecto");
        }
      } else {
        const userValue = parseInt(operationAnswer || "", 10);
        if (Number.isNaN(userValue)) {
          setOperationFeedback("Ingresa un número válido.");
          return;
        }
        if (userValue.toString(2).padStart(8, "0") === operationDisplay) {
          setOperationFeedback("Correcto!");
        } else {
          setOperationFeedback("Incorrecto");
        }
      }
      setOperationAnswer("");
      updateOperationDisplay(operationMode);
    } catch (error) {
      setOperationFeedback("Error evaluando la operación.");
    }
  };

  return (
    <div className="app-container">
      <Sidebar
        view={view}
        theme={theme}
        onViewChange={setView}
        onThemeChange={setTheme}
      />
      <main className="main-content">
        {view === "trivias" && (
          <TriviaSection
            topic={topic}
            questionLevel={questionLevel}
            isLoading={isLoading}
            feedback={feedback}
            summaryOpen={summaryOpen}
            questions={questions}
            activeQuestion={activeQuestion}
            onTopicChange={setTopic}
            onLevelChange={setQuestionLevel}
            onGenerateQuestion={handleGenerateQuestion}
            onAnswerChange={handleAnswerChange}
            onSelectQuestion={handleSelectQuestion}
            onNextQuestion={handleNextQuestion}
            onFinish={handleFinish}
          />
        )}
        {view === "operaciones" && (
          <OperationSection
            operationDisplay={operationDisplay}
            operationAnswer={operationAnswer}
            operationMode={operationMode}
            operationFeedback={operationFeedback}
            onAnswerChange={setOperationAnswer}
            onModeChange={setOperationMode}
            onRefresh={() => updateOperationDisplay(operationMode)}
            onVerify={handleVerifyOperation}
          />
        )}
        {view === "juegos" && (
          <GamesSection
            selectedGame={selectedGame}
            onSelectGame={setSelectedGame}
          />
        )}
        { view === "noticias" && (
          <NewsSection 
            articles={newsArticles}
            allCategories={newsCategories}
            loading={newsLoading}
            syncing={newsSyncing}
            error={newsError}
            lastUpdate={newsLastUpdate}
            hasMore={newsHasMore}
            fetchNextPage={handleFetchNextPage}
            setFilters={setNewsFilters}
          />
        )}
        {summaryOpen && (
          <QuestionSummary
            questions={questions}
            onClose={() => setSummaryOpen(false)}
          />
        )}
      </main>
    </div>
  );
}
