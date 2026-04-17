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

const NIVELES_PREGUNTA = ["Bajo", "Normal", "Avanzado", "Experto"];

const randomQuestionType = () => {
  const types = ["Verdadero o falso", "Abierta", "Alternativas", "Escenario"];
  return types[Math.floor(Math.random() * types.length)];
};

export default function Home() {
  const [view, setView] = useState("noticias");
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
  const [newsError, setNewsError] = useState("");
  const [newsLastUpdate, setNewsLastUpdate] = useState(null);
  const [newsOffset, setNewsOffset] = useState(0);
  const [newsHasMore, setNewsHasMore] = useState(true);
  const [newsAvailableCategories, setNewsAvailableCategories] = useState({categories: [], specific_categories: []});
  const [newsFilters, setNewsFilters] = useState({
    selectedCategories: [],
    selectedSpecificCategories: [],
    startDate: "",
    endDate: "",
  });
  const [newsInitialLoaded, setNewsInitialLoaded] = useState(false);
  const newsFetchingRef = useRef(false);
  const LIMIT = 10;

  useEffect(() => {
    const date = new Date().toISOString().split('T')[0];
    const start = new Date();
    start.setDate(start.getDate() - 7);
    const startDate = start.toISOString().split('T')[0];
    
    setNewsFilters({
      selectedCategories: [],
      selectedSpecificCategories: [],
      startDate: startDate,
      endDate: date,
    });
  }, []);

  const fetchNews = async (currentOffset, isInitial, signal) => {
    if (newsFetchingRef.current) return;
    newsFetchingRef.current = true;

    if (isInitial) setNewsLoading(true);
    if (isInitial) {
      setNewsError("");
    }

    try {
      const { selectedCategories, selectedSpecificCategories, startDate, endDate } = newsFilters;
      const categoryQuery = selectedCategories.length > 0 
        ? `categories=${encodeURIComponent(selectedCategories.join(','))}` 
        : "";
      const specificCategoryQuery = selectedSpecificCategories.length > 0 
        ? `specific_categories=${encodeURIComponent(selectedSpecificCategories.join(','))}` 
        : "";
      const dateQuery = `startDate=${new Date(startDate).toISOString().split('T')[0]}&endDate=${new Date(endDate).toISOString().split('T')[0]}`;
      const query = [categoryQuery, specificCategoryQuery, dateQuery].filter(Boolean).join('&');
      
      const response = await fetch(`/api/news?${query}&limit=${LIMIT}&offset=${currentOffset}`, {
        signal: signal,
      });

      if (!response.ok) {
        const body = await response.json().catch(() => ({}));
        throw new Error(body.error || "No se pudo cargar las noticias");
      }

      const data = await response.json();
      const newArticles = data.items || [];
      
      setNewsArticles(prev => isInitial ? newArticles : [...prev, ...newArticles]);
      setNewsHasMore(newArticles.length === LIMIT);
      setNewsLastUpdate(new Date(data.timestamp).toLocaleTimeString(undefined, {
                      hour: "2-digit",
                      minute: "2-digit",
                      hour12: false
                    }));
      setNewsAvailableCategories(data.availableCategories);
      if (isInitial) setNewsInitialLoaded(true);
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
    if (newsFetchingRef.current) return;
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
        { view === "noticias" && (
          <NewsSection 
            articles={newsArticles}
            availableCategories={newsAvailableCategories}
            loading={newsLoading}
            error={newsError}
            lastUpdate={newsLastUpdate}
            hasMore={newsHasMore}
            fetchNextPage={handleFetchNextPage}
            setFilters={setNewsFilters}
            filters={newsFilters}
          />
        )}
        {view === "juegos" && (
          <GamesSection
            selectedGame={selectedGame}
            onSelectGame={setSelectedGame}
          />
        )}
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
