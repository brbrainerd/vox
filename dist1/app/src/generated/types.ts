export type ChatResult =
  | { readonly _tag: "Success"; readonly text: string }
  | { readonly _tag: "Error"; readonly message: string };

export const Success = (text: string): ChatResult => ({ _tag: "Success", text });
export const Error = (message: string): ChatResult => ({ _tag: "Error", message });

