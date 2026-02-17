export type Greeting =
  | { readonly _tag: "Hello"; readonly message: string };

export const Hello = (message: string): Greeting => ({ _tag: "Hello", message });

