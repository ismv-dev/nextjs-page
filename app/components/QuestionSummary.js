"use client";

export default function QuestionSummary({ questions, onClose }) {
  return (
    <div className="summary-card">
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <h2>Resumen de sesión</h2>
        <button className="btn" onClick={onClose}>Cerrar</button>
      </div>
      {questions.map((item, idx) => (
        <div key={idx} style={{ marginBottom: 16 }}>
          <strong>{idx + 1}. {item.pregunta}</strong>
          {item.alternativas.length ? (
            <p>Alternativas: {item.alternativas.join(", ")}</p>
          ) : (
            <p>Respuesta: {item.answer || "No respondida"}</p>
          )}
          {item.alternativas.length ? (
            <p>Correcta: {item.alternativas[item.correcta] || "-"}</p>
          ) : (
            <p>Validación: {item.validation === false ? "Correcto" : item.validation || "Pendiente"}</p>
          )}
        </div>
      ))}
    </div>
  );
}
