import React, { useState } from "react";

export function Dashboard(): React.ReactElement {
  const [items, set_items] = useState(["System active", "Data synchronized"]);
  return (
    <div className={"dashboard"}>
      <h1>
        {"Dashboard"}
      </h1>
      <ul>
        {items.map((item) => ((() => {
        <li
    >
      {(() => {
        item;
      })()}
    </li>;
      })()))}
      </ul>
    </div>
  );
}
