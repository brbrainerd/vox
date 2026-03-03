import express, { Request, Response } from "express";
import cors from "cors";

const app = express();
app.use(cors());
app.use(express.json());

// Mock LLM actor (replace with real API key for production)
class OpenRouterActor {
  async send(message: string): Promise<string> {
    const apiKey = process.env.OPENROUTER_API_KEY;
    if (apiKey) {
      const response = await fetch("https://openrouter.ai/api/v1/chat/completions", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "Authorization": `Bearer ${apiKey}`,
        },
        body: JSON.stringify({
          model: "openrouter/auto",
          messages: [{ role: "user", content: message }],
        }),
      });
      const data = await response.json() as any;
      return data.choices?.[0]?.message?.content || "No response from LLM";
    }
    // Mock response when no API key is set
    return `Vox AI Echo: ${message}`;
  }
}

app.post("/api/chat", async (req: Request, res: Response) => {
  try {
    const request = req;
    const body = request.json();
    const prompt = str(body.message);
    const response = await new OpenRouterActor().send(prompt);
    response;
  } catch (err) {
    res.status(500).json({ error: String(err) });
  }
});

const PORT = process.env.PORT || 3001;
app.listen(PORT, () => {
  console.log(`Vox server running on port ${PORT}`);
});
