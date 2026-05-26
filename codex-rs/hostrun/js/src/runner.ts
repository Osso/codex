import {
  HostrunSession,
  type HostrunApprovalDecision,
  type HostrunCapability,
  type HostrunEvalResult,
} from "./hostrun-session.js";

export interface HostrunRunnerRequest {
  session_id?: unknown;
  code?: unknown;
  capabilities?: Record<string, HostrunRunnerCapability>;
  interruptCycles?: number;
  memoryLimitBytes?: number;
}

export interface HostrunRunnerCapability {
  approval: {
    id: string;
    summary: string;
  };
  decision?: "approve" | "deny" | "pending";
  denyReason?: string;
  result?: unknown;
}

export async function runHostrunRequest(
  request: HostrunRunnerRequest,
): Promise<HostrunEvalResult> {
  const sessionId = validateString(request.session_id, "session_id");
  const code = validateString(request.code, "code");
  void sessionId;

  const session = await HostrunSession.create({
    approve: (approval) => approvalDecisionFor(request, approval.tool),
    capabilities: buildCapabilities(request.capabilities ?? {}),
    interruptCycles: request.interruptCycles,
    memoryLimitBytes: request.memoryLimitBytes,
  });

  try {
    return await session.eval(code);
  } finally {
    session.dispose();
  }
}

export class HostrunRunnerServer {
  private readonly sessions = new Map<string, HostrunSession>();
  private currentRequest: HostrunRunnerRequest | null = null;

  async run(request: HostrunRunnerRequest): Promise<HostrunEvalResult> {
    const sessionId = validateString(request.session_id, "session_id");
    const code = validateString(request.code, "code");
    this.currentRequest = request;

    try {
      const session = await this.sessionFor(sessionId);
      return await session.eval(code);
    } finally {
      this.currentRequest = null;
    }
  }

  dispose(): void {
    for (const session of this.sessions.values()) {
      session.dispose();
    }
    this.sessions.clear();
  }

  private async sessionFor(sessionId: string): Promise<HostrunSession> {
    const existing = this.sessions.get(sessionId);
    if (existing) {
      return existing;
    }

    const session = await HostrunSession.create({
      approve: (approval) => approvalDecisionFor(this.request(), approval.tool),
      capabilities: this.capabilities(),
    });
    this.sessions.set(sessionId, session);
    return session;
  }

  private capabilities(): Record<string, HostrunCapability> {
    return new Proxy(Object.create(null) as Record<string, HostrunCapability>, {
      get: (_target, tool) => {
        if (typeof tool !== "string") {
          return undefined;
        }

        return this.capabilityFor(tool);
      },
    });
  }

  private capabilityFor(tool: string): HostrunCapability | undefined {
    return buildCapabilities(this.request().capabilities ?? {})[tool];
  }

  private request(): HostrunRunnerRequest {
    if (!this.currentRequest) {
      throw new Error("Hostrun runner request is unavailable");
    }

    return this.currentRequest;
  }
}

function validateString(value: unknown, field: string): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`Hostrun runner request must include ${field}`);
  }

  return value;
}

function buildCapabilities(
  fixtures: Record<string, HostrunRunnerCapability>,
): Record<string, HostrunCapability> {
  return {
    ...builtInCapabilities(),
    ...fixtureCapabilities(fixtures),
  };
}

function approvalDecisionFor(
  request: HostrunRunnerRequest,
  tool: string,
): HostrunApprovalDecision {
  const fixture = request.capabilities?.[tool];
  const decision = fixture?.decision ?? defaultApprovalDecision(tool);

  if (decision === "deny") {
    return {
      type: "deny",
      reason: fixture?.denyReason ?? `Hostrun denied ${tool}`,
    };
  }

  return { type: decision };
}

function fixtureCapabilities(
  fixtures: Record<string, HostrunRunnerCapability>,
): Record<string, HostrunCapability> {
  return Object.fromEntries(
    Object.entries(fixtures).map(([tool, fixture]) => [
      tool,
      {
        describe: (args) => ({
          id: fixture.approval.id,
          tool,
          summary: fixture.approval.summary,
          args,
        }),
        invoke: () => fixture.result,
      },
    ]),
  );
}

function builtInCapabilities(): Record<string, HostrunCapability> {
  return {
    "fs.write": {
      describe: (args) => {
        const path = validateCapabilityString(args.path, "path");
        const content = validateCapabilityString(args.content, "content");
        return {
          id: `fs.write:${path}`,
          tool: "fs.write",
          summary: `Write ${content.length} bytes to ${path}`,
          args,
        };
      },
      invoke: () => {
        throw new Error("fs.write requires approval");
      },
    },
  };
}

function defaultApprovalDecision(tool: string): "approve" | "pending" {
  return builtInCapabilities()[tool] ? "pending" : "approve";
}

function validateCapabilityString(value: unknown, field: string): string {
  if (typeof value !== "string") {
    throw new Error(`Hostrun capability argument ${field} must be a string`);
  }

  return value;
}
