#!/usr/bin/env node
import { runHostrunRequest, type HostrunRunnerRequest } from "./runner.js";

const input = await readStdin();

try {
  const request = JSON.parse(input) as HostrunRunnerRequest;
  const result = await runHostrunRequest(request);
  process.stdout.write(`${JSON.stringify(result)}\n`);
} catch (error) {
  process.stderr.write(`${formatError(error)}\n`);
  process.exitCode = 1;
}

async function readStdin(): Promise<string> {
  const chunks: string[] = [];
  for await (const chunk of process.stdin) {
    chunks.push(String(chunk));
  }

  return chunks.join("");
}

function formatError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}
