# Maple/OpenSecret API Guide

This guide documents how to query the Maple/OpenSecret AI API for use with Aman.
The Aman server currently integrates via the Rust `opensecret` SDK (MapleBrain),
but the browser SDK examples below are kept for reference.

## Overview

Maple/OpenSecret provides an OpenAI-compatible API with end-to-end encryption. Requests are encrypted client-side,
processed in a secure enclave, and responses are encrypted before returning. This allows privacy-preserving AI integration.

**Key features:**
- OpenAI-compatible chat completions API (streaming required)
- End-to-end encryption for prompts and responses
- Streaming support
- Authentication varies by SDK (browser SDK uses user auth; MapleBrain uses an API key)

## Aman integration notes

- Aman integrates via `maple-brain`, which wraps this API and handles the attestation handshake.
- MapleBrain can optionally use a `ToolExecutor` (e.g., Grok search) for real-time lookups. Tool calls are separate
  from the OpenSecret API path. See `crates/maple-brain/README.md` and `crates/grok-brain/README.md`.

## Setup

### Installation

```bash
npm install @opensecret/react openai
# or
yarn add @opensecret/react openai
# or
bun add @opensecret/react openai
```

### Provider Configuration

Wrap your application with the `OpenSecretProvider`:

```javascript
import { OpenSecretProvider } from "@opensecret/react";

function App() {
  return (
    <OpenSecretProvider
      apiUrl="https://enclave.trymaple.ai"
      clientId="your-project-uuid"
    >
      <YourApp />
    </OpenSecretProvider>
  );
}
```

### Accessing the SDK

```javascript
import { useOpenSecret } from "@opensecret/react";

function YourComponent() {
  const os = useOpenSecret();
  // os.aiCustomFetch - custom fetch for AI requests
  // os.auth - authentication state
  // os.apiUrl - base API URL
}
```

## Authentication

Users must be authenticated before making AI requests:

```javascript
// Sign in
await os.signIn(email, password);

// Check auth status
if (os.auth.user) {
  // User is authenticated, can make AI requests
}

// Sign out
await os.signOut();
```

## Making AI Requests

### OpenAI Client Setup

Create an OpenAI client configured to use OpenSecret's encrypted endpoint:

```javascript
import OpenAI from "openai";
import { useOpenSecret } from "@opensecret/react";

const os = useOpenSecret();

const client = new OpenAI({
  baseURL: `${os.apiUrl}/v1/`,
  dangerouslyAllowBrowser: true,
  apiKey: "api-key-doesnt-matter",  // Auth handled by aiCustomFetch
  defaultHeaders: {
    "Accept-Encoding": "identity",
    "Content-Type": "application/json"
  },
  fetch: os.aiCustomFetch  // Handles encryption + token refresh
});
```

### Non-Streaming Request

Note: some Maple/OpenSecret deployments require streaming responses. Aman uses streaming in MapleBrain.

```javascript
const response = await client.chat.completions.create({
  model: "llama-3.3-70b",
  messages: [
    { role: "system", content: "You are a helpful assistant." },
    { role: "user", content: "Hello, how are you?" }
  ]
});

const reply = response.choices[0].message.content;
```

### Streaming Request (recommended)

```javascript
const stream = await client.beta.chat.completions.stream({
  model: "llama-3.3-70b",
  messages: [
    { role: "system", content: "You are a helpful assistant." },
    { role: "user", content: "Tell me a story." }
  ],
  stream: true
});

for await (const chunk of stream) {
  const content = chunk.choices[0]?.delta?.content;
  if (content) {
    process.stdout.write(content);
  }
}
```

### Request Parameters

Standard OpenAI chat completion parameters are supported:

| Parameter | Type | Description |
|-----------|------|-------------|
| `model` | string | Model identifier (required) |
| `messages` | array | Conversation history (required) |
| `stream` | boolean | Enable streaming responses |
| `temperature` | number | Sampling temperature (0-2) |
| `max_tokens` | number | Maximum tokens to generate |
| `top_p` | number | Nucleus sampling parameter |

## Available Models

| Model | Description |
|-------|-------------|
| `llama-3.3-70b` | Llama 3.3 70B (text) |
| `gemma-3-27b` | Gemma 3 27B (text) |
| `deepseek-r1-0528` | DeepSeek R1 (text) |
| `gpt-oss-120b` | GPT OSS 120B (text) |
| `qwen3-coder-480b` | Qwen3 Coder 480B (text) |
| `qwen3-vl-30b` | Qwen3 VL 30B (vision) |

Check with OpenSecret for current model availability.

## Direct Fetch Usage

For advanced control, use `aiCustomFetch` directly:

```javascript
const response = await os.aiCustomFetch(
  `${os.apiUrl}/v1/chat/completions`,
  {
    method: "POST",
    headers: {
      "Content-Type": "application/json"
    },
    body: JSON.stringify({
      model: "llama-3.3-70b",
      messages: [
        { role: "user", content: "Hello" }
      ]
    })
  }
);

const data = await response.json();
```

## Error Handling

```javascript
try {
  const response = await client.chat.completions.create({
    model: "llama-3.3-70b",
    messages: [{ role: "user", content: "Hello" }]
  });
  return response.choices[0].message.content;
} catch (error) {
  const message = error instanceof Error
    ? error.message
    : "Failed to get AI response";
  console.error("AI request failed:", message);
  throw error;
}
```

## Encryption Flow

Data flows through these stages:

1. **Client-side encryption** - Request encrypted before transmission
2. **Secure transmission** - Encrypted data sent to OpenSecret
3. **Enclave decryption** - Data decrypted in secure enclave
4. **AI processing** - Model processes request
5. **Response encryption** - Response encrypted in enclave
6. **Encrypted return** - Encrypted response sent to client
7. **Client-side decryption** - Response decrypted for use

This provides true end-to-end encryption where prompts and responses are never visible in plaintext outside the secure enclave.

## Integration with Aman

For server-side usage in Aman, MapleBrain uses the Rust `opensecret` SDK and an API key:

- Set `MAPLE_API_KEY` (required) and optional `MAPLE_API_URL`, `MAPLE_MODEL`, `MAPLE_VISION_MODEL`,
  `MAPLE_SYSTEM_PROMPT`, and `MAPLE_PROMPT_FILE` (default: `SYSTEM_PROMPT.md`).
- MapleBrain performs an attestation handshake on startup and uses streaming responses.

### Vision Support (MapleBrain)

When incoming Signal messages include image attachments, MapleBrain switches to
`MAPLE_VISION_MODEL` and sends a multimodal request with image data URLs.
Attachment files are read from the local signal-cli attachments directory.

If you want to build your own non-React integration, you'll need to:

1. Authenticate and obtain tokens via the OpenSecret REST API
2. Use the tokens to make requests to `/v1/chat/completions`
3. Handle the encryption/decryption flow manually or via a non-React SDK

The base endpoint pattern is:
```
POST https://enclave.trymaple.ai/v1/chat/completions
```

Request body follows the standard OpenAI chat completions format.

## References

- [OpenSecret Documentation](https://docs.opensecret.cloud/docs)
- [OpenSecret API Reference](https://docs.opensecret.cloud/docs/api)
- [AI Integration Guide](https://docs.opensecret.cloud/docs/guides/ai-integration)
