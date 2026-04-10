"use client";

import QuestionSummary from "./QuestionSummary";

export default function TriviaSection({
  topic,
  questionLevel,
  isLoading,
  feedback,
  summaryOpen,
  questions,
  activeQuestion,
  onTopicChange,
  onLevelChange,
  onGenerateQuestion,
  onAnswerChange,
  onSelectQuestion,
  onNextQuestion,
  onFinish,
}) {
  return (
    <div className="trivia-card">
    <h1 className="page-title">Trivias personalizadas</h1>
      <div className="question-panel">
        <div className="question-row">
          <textarea
            name="tema"
            id="tema"
            placeholder="Tema"
            className="input"
            value={topic}
            onChange={(event) => onTopicChange(event.target.value)}
          />
        </div>
        <br/>
        <div className="question-row">
          <button className="btn" type="button" onClick={onGenerateQuestion} disabled={isLoading}>
            {isLoading ? "Generando..." : "Generar pregunta"}
          </button>
        </div>
        <br/>
        <div className="question-row">
          <select
            value={questionLevel}
            onChange={(event) => onLevelChange(Number(event.target.value))}
            className="input"
          >
            <option value={0}>Bajo</option>
            <option value={1}>Normal</option>
            <option value={2}>Avanzado</option>
            <option value={3}>Experto</option>
          </select>
        </div>
      </div>

      {activeQuestion && (
        <div className="question-panel" style={{ marginTop: 20 }}>
          <h2>{activeQuestion.pregunta}</h2>

          {activeQuestion.alternativas.length ? (
            <div id="alternativas">
              {activeQuestion.alternativas.map((option, idx) => (
                <label className="radio-label" key={`${option}-${idx}`}>
                  <input
                    type="radio"
                    name="alternativa"
                    value={idx}
                    checked={activeQuestion.answer === idx}
                    onChange={() => onAnswerChange(idx)}
                  />
                  {option}
                </label>
              ))}
            </div>
          ) : (
            <textarea
              id="respuesta"
              className="input"
              placeholder="Ingresa tu respuesta..."
              value={activeQuestion.answer || ""}
              onChange={(event) => onAnswerChange(event.target.value)}
            />
          )}

          {activeQuestion.validation !== undefined && (
            <p className="validation-text">
              {activeQuestion.validation === false ? "Respuesta correcta" : activeQuestion.validation}
            </p>
          )}

          <div className="question-actions">
            <button className="btn" type="button" onClick={onNextQuestion} disabled={isLoading}>
              Siguiente
            </button>
            <button className="btn" type="button" onClick={onFinish}>
              Terminar
            </button>
          </div>

          <div id="preguntas">
            {questions.map((_, idx) => (
              <button
                type="button"
                key={`q-${idx}`}
                className="btnPregunta"
                onClick={() => onSelectQuestion(idx)}
              >
                {idx + 1}
              </button>
            ))}
          </div>
        </div>
      )}

      {feedback && <p className="feedback-text">{feedback}</p>}

      {summaryOpen && <QuestionSummary questions={questions} />}
    </div>
  );
}
