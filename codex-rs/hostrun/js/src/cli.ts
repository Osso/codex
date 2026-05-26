#!/usr/bin/env node
import {
  HostrunRunnerServer,
  runHostrunRequest,
  type HostrunRunnerRequest,
} from "./runner.js";

if (process.argv.includes("--serve")) {
  await serveJsonLines();
} else {
  await runOneShot();
}

async function runOneShot(): Promise<void> {
  const input = await readStdin();

  try {
    const request = JSON.parse(input) as HostrunRunnerRequest;
    const result = await runHostrunRequest(request);
    process.stdout.write(`${JSON.stringify(result)}\n`);
  } catch (error) {
    process.stderr.write(`${formatError(error)}\n`);
    process.exitCode = 1;
  }
}

async function serveJsonLines(): Promise<void> {
  const server = new HostrunRunnerServer();
  let buffer = "";

  try {
    for await (const chunk of process.stdin) {
      buffer += String(chunk);
      const lines = buffer.split("\n");
      buffer = lines.pop() ?? "";
      for (const line of lines) {
        await runJsonLine(server, line);
      }
    }

    if (buffer.trim().length > 0) {
      await runJsonLine(server, buffer);
    }
  } finally {
    server.dispose();
  }
}

async function runJsonLine(
  server: HostrunRunnerServer,
  line: string,
): Promise<void> {
  if (line.trim().length === 0) {
    return;
  }

  try {
    const request = JSON.parse(line) as HostrunRunnerRequest;
    const result = await server.run(request);
    process.stdout.write(`${JSON.stringify(result)}\n`);
  } catch (error) {
    process.stdout.write(`${JSON.stringify({ type: "error", error: formatError(error) })}\n`);
  }
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
