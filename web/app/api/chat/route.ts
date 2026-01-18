import { createOpenAI, openai } from "@ai-sdk/openai";
import { streamText, convertToModelMessages, type UIMessage } from "ai";

export async function POST(req: Request) {
  const { messages }: { messages: UIMessage[] } = await req.json();

  const amanBaseUrl =
    process.env.AMAN_API_BASE_URL ??
    "https://aman-gateway.openagents.workers.dev/v1";
  const amanApiKey = process.env.AMAN_API_KEY ?? "aman-local";
  const amanModel = process.env.AMAN_API_MODEL ?? "x-ai/grok-4.1-fast";
  const openaiModel = process.env.OPENAI_MODEL ?? "gpt-5-nano";

  const useAman = amanBaseUrl.trim().length > 0;
  const client = useAman
    ? createOpenAI({ baseURL: amanBaseUrl, apiKey: amanApiKey })
    : openai;

  const model = useAman ? client.chat(amanModel) : client.responses(openaiModel);

  const result = streamText({
    model,
    messages: convertToModelMessages(messages),
    ...(amanBaseUrl
      ? {}
      : {
          providerOptions: {
            openai: {
              reasoningEffort: "low",
              reasoningSummary: "auto",
            },
          },
        }),
  });

  return result.toUIMessageStreamResponse({
    sendReasoning: true,
  });
}
