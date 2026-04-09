"use client";

export default function TriviaSection({
    operationDisplay,
    operationMode,
    handleVerifyOperation,
    operationFeedback,
    updateOperationDisplay,
    setOperationAnswer,
    setOperationMode,
    operationAnswer
}) {
  return (
    <div className="trivia-card">
      <h1 className="page-title">Aritmética y conversión</h1>
      <div className="question-panel">
        <div className="question-row">
          <h2 id="h1Operaciones">{operationDisplay}</h2>
          <button className="btn" type="button" onClick={() => updateOperationDisplay(operationMode)}>
            ↻
          </button>
        </div>
        <div className="question-row">
          <input
            id="inputOperaciones"
            type="text"
            className="input"
            value={operationAnswer}
            onChange={(event) => setOperationAnswer(event.target.value)}
            placeholder={operationMode === 0 ? "Resultado" : "Decimal"}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                handleVerifyOperation();
              }
            }}
          />
        </div>
        <div className="question-row">
          <select
            value={operationMode}
            onChange={(event) => setOperationMode(Number(event.target.value))}
            className="input"
          >
            <option value={0}>Aritmética</option>
            <option value={1}>Conversión</option>
          </select>
          <button className="btn" onClick={handleVerifyOperation}>
            Verificar
          </button>
        </div>
        {operationFeedback && <p className="feedback-text">{operationFeedback}</p>}
      </div>
    </div>
  );
}
