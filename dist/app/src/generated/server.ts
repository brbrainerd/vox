import express, { Request, Response } from "express";
import cors from "cors";

const app = express();
app.use(cors());
app.use(express.json());

// Mock LLM actor (replace with real API key for production)
class ClaudeActor {
  async send(message: string): Promise<string> {
    const apiKey = process.env.ANTHROPIC_API_KEY;
    if (apiKey) {
      const response = await fetch("https://api.anthropic.com/v1/messages", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "x-api-key": apiKey,
          "anthropic-version": "2023-06-01",
        },
        body: JSON.stringify({
          model: "claude-sonnet-4-20250514",
          max_tokens: 256,
          messages: [{ role: "user", content: message }],
        }),
      });
      const data = await response.json() as any;
      return data.content?.[0]?.text || "No response from Claude";
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
    const response = await new ClaudeActor().send(prompt);
    const result = response;
    res.json({ text: result });
  } catch (err) {
    res.status(500).json({ error: String(err) });
  }
});

const PORT = process.env.PORT || 3001;
app.listen(PORT, () => {
  console.log(`Vox server running on port ${PORT}`);
});
