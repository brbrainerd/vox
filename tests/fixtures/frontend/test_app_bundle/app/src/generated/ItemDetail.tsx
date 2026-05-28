import React from "react";

export interface ItemDetailProps {
  id: string;
}

export function ItemDetail({ id }: ItemDetailProps): React.ReactElement {
  return (
    <div>
      <h2>
        {"Item: {id}"}
      </h2>
    </div>
  );
}
