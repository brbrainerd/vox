import React, { useState } from "react";

export function Chat(): React.ReactElement {
  const [messages, set_messages] = useState([]);
  const [input, set_input] = useState("");
  const send = (_e) => set_messages([...messages, { role: "user", text: input }]);
  return (
    <div className={"chat-container"}>
      <h1>
        Vox
        Chatbot
      </h1>
      <div className={"messages"}>
        {messages.map((msg, _i) => (
          <div className={`message ${msg.role}`}>
            {msg.text}
          </div>
        ))}
      </div>
      <div className={"input-area"}>
        <input className={"chat-input"} onChange={(e) => set_input(e.target.value)} value={input} />
        <button className={"send-btn"} onClick={send}>
          Send
        </button>
      </div>
    </div>
  );
}
