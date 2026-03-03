import React, { useState } from "react";

import "./Chat.css";

export function Chat(): React.ReactElement {
  const [messages, set_messages] = useState([]);
  const [input, set_input] = useState("");
  const send = (_e) => [set_messages([...messages, { role: "user", text: input }]), new ChatClientActor().send(input), set_input("")];
  return (
    <div className={"chat_container"}>
      <div className={"messages"}>
        {messages.map((msg, _i) => (
          <div className={`message ${msg.role}`}>
            {msg.text}
          </div>
        ))}
      </div>
      <div className={"input_area"}>
        <input className={"chat_input"} value={input} onChange={(e) => set_input(e.target.value)} onKeyUp={(e) => e.key === "Enter" && send(e)} placeholder={"Type a message..."} />
        <button className={"send_btn"} onClick={send}>
          Send
        </button>
      </div>
    </div>
  );
}
