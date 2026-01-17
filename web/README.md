This is the [assistant-ui](https://github.com/Yonom/assistant-ui) starter project.

## Getting Started

First, add your OpenAI-compatible API key to `.env.local` file (if using a hosted provider):

```
OPENAI_API_KEY=sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

To use the local Aman API instead of a hosted provider, set:

```
AMAN_API_BASE_URL=http://127.0.0.1:8787
AMAN_API_KEY=aman-local
AMAN_API_MODEL=aman-chat
```

`AMAN_API_KEY` should match `AMAN_API_TOKEN` used by the local API (or can be omitted if the API has no token).

Then, run the development server:

```bash
npm run dev
# or
yarn dev
# or
pnpm dev
# or
bun dev
```

Open [http://localhost:3000](http://localhost:3000) with your browser to see the result.

You can start editing the page by modifying `app/page.tsx`. The page auto-updates as you edit the file.
