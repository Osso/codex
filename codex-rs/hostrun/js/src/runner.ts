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

function validateString(value: unknown, field: string): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`Hostrun runner request must include ${field}`);
  }

  return value;
}

function buildCapabilities(
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

function approvalDecisionFor(
  request: HostrunRunnerRequest,
  tool: string,
): HostrunApprovalDecision {
  const fixture = request.capabilities?.[tool];
  const decision = fixture?.decision ?? "approve";

  if (decision === "deny") {
    return {
      type: "deny",
      reason: fixture?.denyReason ?? `Hostrun denied ${tool}`,
    };
  }

  return { type: decision };
}
